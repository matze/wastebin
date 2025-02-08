use crate::crypto::Password;
use crate::errors::Error;
use crate::id::Id;
use parking_lot::Mutex;
use rusqlite::{params, Connection, Transaction};
use rusqlite_migration::{HookError, Migrations, M};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use tokio::task::spawn_blocking;

static MIGRATIONS: LazyLock<Migrations> = LazyLock::new(|| {
    Migrations::new(vec![
        M::up(include_str!("migrations/0001-initial.sql")),
        M::up(include_str!("migrations/0002-add-created-column.sql")),
        M::up(include_str!(
            "migrations/0003-drop-created-add-uid-column.sql"
        )),
        M::up_with_hook(
            include_str!("migrations/0004-add-compressed-column.sql"),
            |tx: &Transaction| {
                let mut stmt = tx.prepare("SELECT id, text FROM entries")?;

                let rows = stmt
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<Vec<(u32, String)>, _>>()?;

                tracing::debug!("compressing {} rows", rows.len());

                for (id, text) in rows {
                    let cursor = Cursor::new(text);
                    let data = zstd::stream::encode_all(cursor, zstd::DEFAULT_COMPRESSION_LEVEL)
                        .map_err(|e| HookError::Hook(e.to_string()))?;

                    tx.execute(
                        "UPDATE entries SET data = ?1 WHERE id = ?2",
                        params![data, id],
                    )?;
                }

                Ok(())
            },
        ),
        M::up(include_str!("migrations/0005-drop-text-column.sql")),
        M::up(include_str!("migrations/0006-add-nonce-column.sql")),
        M::up(include_str!("migrations/0007-add-title-column.sql")),
    ])
});

/// Our main database and integrated cache.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

/// Database opening modes
#[derive(Debug)]
pub enum Open {
    /// Open in-memory database that is wiped after reload
    Memory,
    /// Open database from given path
    Path(PathBuf),
}

/// Module with types for insertion.
pub mod write {
    use crate::crypto::{Encrypted, Password, Plaintext};
    use crate::errors::Error;
    use async_compression::tokio::bufread::ZstdEncoder;
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;
    use std::num::NonZeroU32;
    use tokio::io::{AsyncReadExt, BufReader};

    /// An uncompressed entry to be inserted into the database.
    #[derive(Default, Debug, Serialize, Deserialize)]
    pub struct Entry {
        /// Content
        pub text: String,
        /// File extension
        pub extension: Option<String>,
        /// Expiration in seconds from now
        pub expires: Option<NonZeroU32>,
        /// Delete if read
        pub burn_after_reading: Option<bool>,
        /// User identifier that inserted the entry
        pub uid: Option<i64>,
        /// Optional password to encrypt the entry
        pub password: Option<String>,
        /// Title
        pub title: Option<String>,
    }

    /// A compressed entry to be inserted.
    pub struct CompressedEntry {
        /// Original data
        entry: Entry,
        /// Compressed data
        data: Vec<u8>,
    }

    /// An entry that might be encrypted.
    pub struct DatabaseEntry {
        /// Original data
        pub entry: Entry,
        /// Compressed and potentially encrypted data
        pub data: Vec<u8>,
        /// Nonce for this entry
        pub nonce: Option<Vec<u8>>,
    }

    impl Entry {
        /// Compress the entry for insertion.
        pub async fn compress(self) -> Result<CompressedEntry, Error> {
            let reader = BufReader::new(Cursor::new(&self.text));
            let mut encoder = ZstdEncoder::new(reader);
            let mut data = Vec::new();

            encoder
                .read_to_end(&mut data)
                .await
                .map_err(|e| Error::Compression(e.to_string()))?;

            Ok(CompressedEntry { entry: self, data })
        }
    }

    impl CompressedEntry {
        /// Encrypt if password is set.
        pub async fn encrypt(self) -> Result<DatabaseEntry, Error> {
            let (data, nonce) = if let Some(password) = &self.entry.password {
                let password = Password::from(password.as_bytes().to_vec());
                let plaintext = Plaintext::from(self.data);
                let Encrypted { ciphertext, nonce } = plaintext.encrypt(password).await?;
                (ciphertext, Some(nonce))
            } else {
                (self.data, None)
            };

            Ok(DatabaseEntry {
                entry: self.entry,
                data,
                nonce,
            })
        }
    }
}

/// Module with types for reading from the database.
pub mod read {
    use crate::crypto::{Encrypted, Password};
    use crate::errors::Error;
    use async_compression::tokio::bufread::ZstdDecoder;
    use std::io::Cursor;
    use tokio::io::{AsyncReadExt, BufReader};

    /// A raw entry as read from the database.
    pub struct DatabaseEntry {
        /// Compressed and potentially encrypted data
        pub data: Vec<u8>,
        /// Entry is expired
        pub expired: bool,
        /// Entry must be deleted
        pub must_be_deleted: bool,
        /// User identifier that inserted the entry
        pub uid: Option<i64>,
        /// Nonce for this entry
        pub nonce: Option<Vec<u8>>,
        /// Title
        pub title: Option<String>,
    }

    /// Potentially decrypted but still compressed entry
    pub struct CompressedReadEntry {
        /// Compressed data
        data: Vec<u8>,
        /// Entry must be deleted
        must_be_deleted: bool,
        /// User identifier that inserted the entry
        uid: Option<i64>,
        /// Title
        title: Option<String>,
    }

    /// An entry read from the database.
    pub struct Entry {
        /// Content
        pub text: String,
        /// Delete if read
        pub must_be_deleted: bool,
        /// User identifier that inserted the entry
        pub uid: Option<i64>,
        /// Title
        pub title: Option<String>,
    }

    impl DatabaseEntry {
        pub async fn decrypt(
            self,
            password: Option<Password>,
        ) -> Result<CompressedReadEntry, Error> {
            match (self.nonce, password) {
                (Some(_), None) => Err(Error::NoPassword),
                (None, None | Some(_)) => Ok(CompressedReadEntry {
                    data: self.data,
                    must_be_deleted: self.must_be_deleted,
                    uid: self.uid,
                    title: self.title,
                }),
                (Some(nonce), Some(password)) => {
                    let encrypted = Encrypted::new(self.data, nonce);
                    let decrypted = encrypted.decrypt(password).await?;
                    Ok(CompressedReadEntry {
                        data: decrypted,
                        must_be_deleted: self.must_be_deleted,
                        uid: self.uid,
                        title: self.title,
                    })
                }
            }
        }
    }

    impl CompressedReadEntry {
        pub async fn decompress(self) -> Result<Entry, Error> {
            let reader = BufReader::new(Cursor::new(self.data));
            let mut decoder = ZstdDecoder::new(reader);
            let mut text = String::new();

            decoder
                .read_to_string(&mut text)
                .await
                .map_err(|e| Error::Compression(e.to_string()))?;

            Ok(Entry {
                text,
                uid: self.uid,
                must_be_deleted: self.must_be_deleted,
                title: self.title,
            })
        }
    }
}

impl Database {
    /// Create new database with the given `method`.
    pub fn new(method: Open) -> Result<Self, Error> {
        tracing::debug!("opening {method:?}");

        let mut conn = match method {
            Open::Memory => Connection::open_in_memory()?,
            Open::Path(path) => Connection::open(path)?,
        };

        MIGRATIONS.to_latest(&mut conn)?;

        let conn = Arc::new(Mutex::new(conn));

        Ok(Self { conn })
    }

    /// Insert `entry` under `id` into the database and optionally set owner to `uid`.
    pub async fn insert(&self, id: Id, entry: write::Entry) -> Result<(), Error> {
        let conn = self.conn.clone();
        let id = id.as_u32();
        let write::DatabaseEntry { entry, data, nonce } = entry.compress().await?.encrypt().await?;

        spawn_blocking(move || match entry.expires {
            None => conn.lock().execute(
                "INSERT INTO entries (id, uid, data, burn_after_reading, nonce, title) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, entry.uid, data, entry.burn_after_reading, nonce, entry.title],
            ),
            Some(expires) => conn.lock().execute(
                "INSERT INTO entries (id, uid, data, burn_after_reading, nonce, expires, title) VALUES (?1, ?2, ?3, ?4, ?5, datetime('now', ?6), ?7)",
                params![
                    id,
                    entry.uid,
                    data,
                    entry.burn_after_reading,
                    nonce,
                    format!("{expires} seconds"),
                    entry.title,
                ],
            ),
        })
        .await??;

        Ok(())
    }

    /// Get entire entry for `id`.
    pub async fn get(&self, id: Id, password: Option<Password>) -> Result<read::Entry, Error> {
        let conn = self.conn.clone();
        let id_as_u32 = id.as_u32();

        let entry = spawn_blocking(move || {
            conn.lock().query_row(
                "SELECT data, burn_after_reading, uid, nonce, expires < datetime('now'), title FROM entries WHERE id=?1",
                params![id_as_u32],
                |row| {
                    Ok(read::DatabaseEntry {
                        data: row.get(0)?,
                        must_be_deleted: row.get::<_, Option<bool>>(1)?.unwrap_or(false),
                        uid: row.get(2)?,
                        nonce: row.get(3)?,
                        expired: row.get::<_, Option<bool>>(4)?.unwrap_or(false),
                        title: row.get::<_, Option<String>>(5)?,
                    })
                },
            )
        })
        .await??;

        if entry.expired {
            self.delete(id).await?;
            return Err(Error::NotFound);
        }

        entry.decrypt(password).await?.decompress().await
    }

    /// Get optional `uid` for `id` if it exists but error with `Error::NotFound` if `id` is
    /// expired or does not exist.
    pub async fn get_uid(&self, id: Id) -> Result<Option<i64>, Error> {
        let conn = self.conn.clone();
        let id_as_u32 = id.as_u32();

        let (uid, expired) = spawn_blocking(move || {
            conn.lock().query_row(
                "SELECT uid, expires < datetime('now') FROM entries WHERE id=?1",
                params![id_as_u32],
                |row| {
                    let uid: Option<i64> = row.get(0)?;
                    let expired: Option<bool> = row.get(1)?;
                    Ok((uid, expired))
                },
            )
        })
        .await??;

        if expired.unwrap_or(false) {
            self.delete(id).await?;
            return Err(Error::NotFound);
        }

        Ok(uid)
    }

    /// Get title of a paste.
    pub async fn get_title(&self, id: Id) -> Result<String, Error> {
        let conn = self.conn.clone();
        let id = id.as_u32();

        let title = spawn_blocking(move || {
            conn.lock().query_row(
                "SELECT title FROM entries WHERE id=?1",
                params![id],
                |row| row.get(0),
            )
        })
        .await??;

        Ok(title)
    }

    /// Delete `id`.
    pub async fn delete(&self, id: Id) -> Result<(), Error> {
        let conn = self.conn.clone();
        let id = id.as_u32();

        spawn_blocking(move || {
            conn.lock()
                .execute("DELETE FROM entries WHERE id=?1", params![id])
        })
        .await??;

        Ok(())
    }

    /// Retrieve next monotonically increasing uid.
    pub async fn next_uid(&self) -> Result<i64, Error> {
        let conn = self.conn.clone();

        let uid = spawn_blocking(move || {
            conn.lock().query_row(
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
    use std::num::NonZero;

    use super::*;

    fn new_db() -> Result<Database, Box<dyn std::error::Error>> {
        Ok(Database::new(Open::Memory)?)
    }

    #[tokio::test]
    async fn insert() -> Result<(), Box<dyn std::error::Error>> {
        let db = new_db()?;

        let entry = write::Entry {
            text: "hello world".to_string(),
            uid: Some(10),
            ..Default::default()
        };

        let id = Id::from(1234);
        db.insert(id, entry).await?;

        let entry = db.get(id, None).await?;
        assert_eq!(entry.text, "hello world");
        assert!(entry.uid.is_some());
        assert_eq!(entry.uid.unwrap(), 10);

        let result = db.get(Id::from(5678), None).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn expired_does_not_exist() -> Result<(), Box<dyn std::error::Error>> {
        let db = new_db()?;

        let entry = write::Entry {
            expires: Some(NonZero::new(1).unwrap()),
            ..Default::default()
        };

        let id = Id::from(1234);
        db.insert(id, entry).await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let result = db.get(id, None).await;
        assert!(result.is_err());
        assert!(matches!(result.err().unwrap(), Error::NotFound));

        Ok(())
    }

    #[tokio::test]
    async fn delete() -> Result<(), Box<dyn std::error::Error>> {
        let db = new_db()?;

        let id = Id::from(1234);
        db.insert(id, write::Entry::default()).await?;

        assert!(db.get(id, None).await.is_ok());
        assert!(db.delete(id).await.is_ok());
        assert!(db.get(id, None).await.is_err());

        Ok(())
    }
}
