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

/// An entry inserted into the database.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct InsertEntry {
    /// Content
    pub text: String,
    /// File extension
    pub extension: Option<String>,
    /// Expiration in seconds from now
    pub expires: Option<u32>,
    /// Delete if read
    pub burn_after_reading: Option<bool>,
    /// User identifier that inserted the paste
    pub uid: Option<i64>,
}

/// An entry read from the database.
pub struct ReadEntry {
    /// Content
    pub text: String,
    /// Entry is expired
    pub expired: Option<bool>,
    /// Delete if read
    pub burn_after_reading: Option<bool>,
    /// User identifier that inserted the paste
    pub uid: Option<i64>,
}

#[derive(Debug)]
pub enum Open {
    Memory,
    Path(PathBuf),
}

static MIGRATIONS: Lazy<Migrations> = Lazy::new(|| {
    Migrations::new(vec![
        M::up(include_str!("migrations/0001-initial.sql")),
        M::up(include_str!("migrations/0002-add-created-column.sql")),
        M::up(include_str!(
            "migrations/0003-drop-created-add-uid-column.sql"
        )),
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

    pub async fn insert(&self, id: Id, uid: Option<i64>, entry: InsertEntry) -> Result<(), Error> {
        let conn = self.conn.clone();
        let id = id.as_u32();

        spawn_blocking(move || match entry.expires {
            None => conn.lock().unwrap().execute(
                "INSERT INTO entries (id, uid, text, burn_after_reading) VALUES (?1, ?2, ?3, ?4)",
                params![id, uid, entry.text, entry.burn_after_reading],
            ),
            Some(expires) => conn.lock().unwrap().execute(
                "INSERT INTO entries (id, uid, text, burn_after_reading, expires) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    uid,
                    entry.text,
                    entry.burn_after_reading,
                    format!("{expires} seconds")
                ],
            ),
        })
        .await??;

        Ok(())
    }

    pub async fn get(&self, id: Id) -> Result<ReadEntry, Error> {
        let conn = self.conn.clone();
        let id_as_u32 = id.as_u32();

        let entry = spawn_blocking(move || {
            conn.lock().unwrap().query_row(
                "SELECT text, burn_after_reading, uid, expires < datetime('now') FROM entries WHERE id=?1",
                params![id_as_u32],
                |row| {
                    Ok(ReadEntry {
                        text: row.get(0)?,
                        burn_after_reading: row.get(1)?,
                        uid: row.get(2)?,
                        expired: row.get(3)?,
                    })
                },
            )
        })
        .await??;

        if entry.expired.unwrap_or(false) {
            self.delete(id).await?;
            return Err(Error::NotFound);
        }

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

    pub async fn next_uid(&self) -> Result<i64, Error> {
        let conn = self.conn.clone();

        let uid = spawn_blocking(move || {
            let conn = conn.lock().unwrap();

            conn.query_row(
                "UPDATE uids SET n = n + 1 WHERE id = 0 RETURNING n",
                [],
                |row| row.get(0),
            )
        })
        .await??;

        Ok(uid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new(Open::Memory)?;

        let entry = InsertEntry {
            text: "hello world".to_string(),
            ..Default::default()
        };

        let id = Id::from(1234);
        db.insert(id, Some(10), entry).await?;

        let entry = db.get(id).await?;
        assert_eq!(entry.text, "hello world");
        assert!(entry.uid.is_some());
        assert_eq!(entry.uid.unwrap(), 10);

        let result = db.get(Id::from(5678)).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn burn_after_reading() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new(Open::Memory)?;
        let entry = InsertEntry {
            burn_after_reading: Some(true),
            ..Default::default()
        };
        let id = Id::from(1234);
        db.insert(id, None, entry).await?;
        assert!(db.get(id).await.is_ok());
        assert!(db.get(id).await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn expired_does_not_exist() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new(Open::Memory)?;

        let entry = InsertEntry {
            expires: Some(1),
            ..Default::default()
        };

        let id = Id::from(1234);
        db.insert(id, None, entry).await?;

        let result = db.get(id).await;
        assert!(result.is_err());
        assert!(matches!(result.err().unwrap(), Error::NotFound));

        Ok(())
    }

    #[tokio::test]
    async fn delete() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new(Open::Memory)?;

        let id = Id::from(1234);
        db.insert(id, None, InsertEntry::default()).await?;

        assert!(db.get(id).await.is_ok());
        assert!(db.delete(id).await.is_ok());
        assert!(db.get(id).await.is_err());

        Ok(())
    }
}
