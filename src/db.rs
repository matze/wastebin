use crate::id::Id;
use crate::Error;
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::task::spawn_blocking;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

/// A database entry corresponding to a paste.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Entry {
    /// Content
    pub text: String,
    /// File extension
    pub extension: Option<String>,
    /// Expiration in seconds from now
    pub expires: Option<u32>,
    /// Delete if read
    pub burn_after_reading: Option<bool>,
    /// Seconds since creation
    pub seconds_since_creation: u32,
}

#[derive(Debug)]
pub enum Open {
    Memory,
    Path(PathBuf),
}

static MIGRATIONS: Lazy<Migrations> = Lazy::new(|| {
    Migrations::new(vec![
        M::up(include_str!("migrations/0001-up-initial.sql"))
            .down(include_str!("migrations/0001-down-initial.sql")),
        M::up(include_str!("migrations/0002-up-add-created-column.sql"))
            .down(include_str!("migrations/0002-down-add-created-column.sql")),
    ])
});

impl Database {
    pub fn new(method: Open) -> Result<Self, Error> {
        tracing::debug!("opening {method:?}");

        let mut conn = match method {
            Open::Memory => Connection::open_in_memory()?,
            Open::Path(path) => Connection::open(path)?,
        };

        MIGRATIONS.to_latest(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn insert(&self, id: Id, entry: Entry) -> Result<(), Error> {
        let conn = self.conn.clone();
        let id = id.as_u32();

        spawn_blocking(move || match entry.expires {
            None => conn.lock().unwrap().execute(
                "INSERT INTO entries (id, text, burn_after_reading, created_at) VALUES (?1, ?2, ?3, datetime('now'))",
                params![id, entry.text, entry.burn_after_reading],
            ),
            Some(expires) => conn.lock().unwrap().execute(
                "INSERT INTO entries (id, text, burn_after_reading, expires, created_at) VALUES (?1, ?2, ?3, datetime('now', ?4), datetime('now'))",
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
        let id_as_u32 = id.as_u32();

        let entry = spawn_blocking(move || {
            conn.lock().unwrap().query_row(
                "SELECT text, burn_after_reading, CAST(((julianday('now') - julianday(created_at)) * 24 * 60 * 60) AS INT) FROM entries WHERE id=?1",
                params![id_as_u32],
                |row| {
                    Ok(Entry {
                        text: row.get(0)?,
                        extension: None,
                        expires: None,
                        burn_after_reading: row.get(1)?,
                        seconds_since_creation: row.get(2)?,
                    })
                },
            )
        })
        .await??;

        if entry.burn_after_reading.unwrap_or(false) {
            self.delete(id).await?;
        }

        Ok(entry)
    }

    pub async fn delete(&self, id: Id) -> Result<(), Error> {
        let conn = self.conn.clone();
        let id = id.as_u32();

        spawn_blocking(move || {
            conn.lock()
                .unwrap()
                .execute("DELETE FROM entries WHERE id=?1", params![id])
        })
        .await??;

        Ok(())
    }

    /// Remove all expired entries and return their `Id`s.
    pub async fn purge(&self) -> Result<Vec<Id>, Error> {
        tracing::debug!("purging");

        let conn = self.conn.clone();

        spawn_blocking(move || {
            let conn = conn.lock().unwrap();

            let mut stmt =
                conn.prepare("SELECT id FROM entries WHERE expires < datetime('now')")?;

            let ids = stmt
                .query_map([], |row| Ok(Id::from(row.get::<_, u32>(0)?)))?
                .collect::<Result<Vec<_>, _>>()?;

            conn.execute("DELETE FROM entries WHERE expires < datetime('now')", [])?;

            Ok(ids)
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new(Open::Memory)?;

        let entry = Entry {
            text: "hello world".to_string(),
            ..Default::default()
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
        let db = Database::new(Open::Memory)?;
        let entry = Entry {
            burn_after_reading: Some(true),
            ..Default::default()
        };
        let id = Id::from(1234);
        db.insert(id, entry).await?;
        assert!(db.get(id).await.is_ok());
        assert!(db.get(id).await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn expired_is_purged() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new(Open::Memory)?;

        let entry = Entry {
            expires: Some(1),
            ..Default::default()
        };

        let id = Id::from(1234);
        db.insert(id, entry).await?;
        assert!(db.get(id).await.is_ok());

        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        assert!(db.get(id).await.unwrap().seconds_since_creation >= 1);

        db.purge().await?;
        assert!(db.get(id).await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn delete() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new(Open::Memory)?;

        let id = Id::from(1234);
        db.insert(id, Entry::default()).await?;

        assert!(db.get(id).await.is_ok());
        assert!(db.delete(id).await.is_ok());
        assert!(db.get(id).await.is_err());

        Ok(())
    }
}
