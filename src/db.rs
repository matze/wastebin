use crate::cache::Cache;
use crate::id::Id;
use crate::{Entry, Error};
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::task::spawn_blocking;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    cache: Cache,
}

#[derive(Debug)]
pub enum Open {
    Memory,
    Path(PathBuf),
}

static MIGRATIONS: Lazy<Migrations> = Lazy::new(|| {
    Migrations::new(vec![M::up(include_str!("migrations/0001-up-initial.sql"))
        .down(include_str!("migrations/0001-down-initial.sql"))])
});

impl Database {
    pub fn new(method: Open, cache: Cache) -> Result<Self, Error> {
        tracing::debug!("opening {method:?}");

        let mut conn = match method {
            Open::Memory => Connection::open_in_memory()?,
            Open::Path(path) => Connection::open(&path)?,
        };

        MIGRATIONS.to_latest(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            cache,
        })
    }

    pub async fn insert(&self, id: Id, entry: Entry) -> Result<(), Error> {
        let conn = self.conn.clone();
        let id = id.as_u32();

        spawn_blocking(move || match entry.expires {
            None => conn.lock().unwrap().execute(
                "INSERT INTO entries (id, text, burn_after_reading) VALUES (?1, ?2, ?3)",
                params![id, entry.text, entry.burn_after_reading],
            ),
            Some(expires) => conn.lock().unwrap().execute(
                "INSERT INTO entries (id, text, burn_after_reading, expires) VALUES (?1, ?2, ?3, datetime('now', ?4))",
                params![
                    id,
                    entry.text,
                    entry.burn_after_reading,
                    format!("{expires} seconds")
                ],
            ),
        })
        .await??;

        Ok(())
    }

    pub async fn get(&self, id: Id) -> Result<Entry, Error> {
        let conn = self.conn.clone();
        let id = id.as_u32();

        let entry = spawn_blocking(move || {
            conn.lock().unwrap().query_row(
                "SELECT text, burn_after_reading FROM entries WHERE id=?1",
                params![id],
                |row| {
                    Ok(Entry {
                        text: row.get(0)?,
                        extension: None,
                        expires: None,
                        burn_after_reading: row.get(1)?,
                    })
                },
            )
        })
        .await??;

        let conn = self.conn.clone();

        if entry.burn_after_reading.unwrap_or(false) {
            spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute("DELETE FROM entries WHERE id=?1", params![id])
            })
            .await??;
        }

        Ok(entry)
    }

    /// Remove all expired entries.
    pub async fn purge(&self) -> Result<(), Error> {
        tracing::debug!("purging");

        let conn = self.conn.clone();
        let cache = self.cache.clone();

        spawn_blocking(move || {
            let conn = conn.lock().unwrap();

            let mut stmt =
                conn.prepare("SELECT id FROM entries WHERE expires < datetime('now')")?;

            let mut cache = cache.lock().unwrap();

            for id in stmt.query_map([], |row| Ok(Id::from(row.get::<_, u32>(0)?)))? {
                cache.remove(id?);
            }

            conn.execute("DELETE FROM entries WHERE expires < datetime('now')", [])
        })
        .await??;
        Ok(())
    }
}

/// Purge `db` every minute.
pub async fn purge_periodically(db: Database) -> Result<(), Error> {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

    loop {
        interval.tick().await;
        db.purge().await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache;

    #[tokio::test]
    async fn insert() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new(Open::Memory, cache::new(0))?;

        let entry = Entry {
            text: "hello world".to_string(),
            extension: None,
            expires: None,
            burn_after_reading: None,
        };

        let id = Id::from(1234);
        db.insert(id, entry).await?;

        let entry = db.get(id).await?;
        assert_eq!(entry.text, "hello world");

        let result = db.get(Id::from(5678)).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn burn_after_reading() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new(Open::Memory, cache::new(0))?;
        let entry = Entry {
            text: "hello world".to_string(),
            extension: None,
            expires: None,
            burn_after_reading: Some(true),
        };
        let id = Id::from(1234);
        db.insert(id, entry).await?;
        assert!(db.get(id).await.is_ok());
        assert!(db.get(id).await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn expired_is_purged() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new(Open::Memory, cache::new(0))?;

        let entry = Entry {
            text: "hello world".to_string(),
            extension: None,
            expires: Some(1),
            burn_after_reading: None,
        };

        let id = Id::from(1234);
        db.insert(id, entry).await?;
        assert!(db.get(id).await.is_ok());

        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        db.purge().await?;
        assert!(db.get(id).await.is_err());

        Ok(())
    }
}
