use crate::crypto::{self, Password};
use crate::expiration::Expiration;
use crate::id::Id;
use read::{DatabaseEntry, ListEntry, Metadata};
use rusqlite::{Connection, Transaction, params};
use rusqlite_migration::{HookError, M, Migrations};
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;

/// Database related errors.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("not allowed to delete")]
    Delete,
    #[error("sqlite error: {0}")]
    Sqlite(rusqlite::Error),
    #[error("migrations error: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("failed to compress: {0}")]
    Compression(String),
    #[error("password not given")]
    NoPassword,
    #[error("entry not found")]
    NotFound,
    #[error("join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("crypto error: {0}")]
    Crypto(#[from] crypto::Error),
    #[error("failed to send command to channel")]
    SendError,
    #[error("failed to send result")]
    ResultSendError,
    #[error("failed to send result: {0}")]
    ResultRecvError(#[from] oneshot::error::RecvError),
    #[error("failed to receive command: {0}")]
    NoCommand(#[from] kanal::ReceiveError),
}

/// The programmatic database interface. However, database calls are not translated directly to
/// sqlite but moved forward to a [`Handler`] which reads database commands from a queue and
/// processes them. This is done to avoid locking the underlying non-Send database per call which
/// cuts down performance in half on certain systems.
#[derive(Clone)]
pub struct Database {
    /// Sender for database commands.
    sender: kanal::AsyncSender<Command>,
}

/// Actual database handler that owns the connection to the underlying sqlite database.
struct Handler {
    conn: Connection,
    /// Receiver for database commands.
    receiver: kanal::Receiver<Command>,
}

/// Commands issued to the database handler and corresponding to [`Database`] calls.
enum Command {
    Insert {
        entry: write::DatabaseEntry,
        result: oneshot::Sender<Result<(Id, write::Entry), Error>>,
    },
    Get {
        id: Id,
        result: oneshot::Sender<Result<DatabaseEntry, Error>>,
    },
    GetMetadata {
        id: Id,
        result: oneshot::Sender<Result<Metadata, Error>>,
    },
    Delete {
        id: Id,
        result: oneshot::Sender<Result<(), Error>>,
    },
    DeleteMany {
        ids: Vec<Id>,
        result: oneshot::Sender<Result<usize, Error>>,
    },
    DeleteFor {
        id: Id,
        uid: i64,
        result: oneshot::Sender<Result<(), Error>>,
    },
    NextUid {
        result: oneshot::Sender<Result<i64, Error>>,
    },
    List {
        result: oneshot::Sender<Result<Vec<ListEntry>, Error>>,
    },
    Purge {
        result: oneshot::Sender<Result<Vec<Id>, Error>>,
    },
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
    use crate::db::Error;
    use async_compression::tokio::bufread::ZstdEncoder;
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;
    use std::num::NonZeroU32;
    use tokio::io::AsyncReadExt;

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
            let mut encoder = ZstdEncoder::new(Cursor::new(&self.text));
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
    use crate::db::Error;
    use crate::expiration::Expiration;
    use crate::id::Id;
    use async_compression::tokio::bufread::ZstdDecoder;
    use std::io::Cursor;
    use tokio::io::AsyncReadExt;

    /// A raw entry as read from the database.
    #[derive(Debug)]
    pub struct DatabaseEntry {
        /// Compressed and potentially encrypted data
        pub data: Vec<u8>,
        /// Metadata
        pub metadata: Metadata,
        /// Entry is expired
        pub expired: bool,
        /// Entry must be deleted
        pub must_be_deleted: bool,
        /// Nonce for this entry
        pub nonce: Option<Vec<u8>>,
    }

    /// Potentially decrypted but still compressed entry
    #[derive(Debug)]
    pub struct CompressedReadEntry {
        /// Compressed data
        data: Vec<u8>,
        /// Metadata
        metadata: Metadata,
        /// Entry must be deleted
        must_be_deleted: bool,
    }

    /// Uncompressed entry
    #[derive(Debug)]
    pub struct UncompressedEntry {
        /// Content
        pub text: String,
        /// Metadata
        pub metadata: Metadata,
        /// Entry must be deleted
        pub must_be_deleted: bool,
    }

    /// Uncompressed, decrypted data read from the database.
    #[derive(Debug)]
    pub struct Data {
        /// Content
        pub text: String,
        /// Metadata
        pub metadata: Metadata,
    }

    /// Paste metadata, i.e. anything but actual content.
    #[derive(Debug)]
    pub struct Metadata {
        /// User identifier that inserted the entry
        pub uid: Option<i64>,
        /// Title
        pub title: Option<String>,
        /// Entry expiration datetime
        pub expiration: Option<Expiration>,
    }

    /// Potentially deleted or non-existent expired entry.
    #[derive(Debug)]
    pub enum Entry {
        /// Entry found and still available.
        Regular(Data),
        /// Entry burned.
        Burned(Data),
    }

    /// A simple entry as read from the database for listing purposes.
    #[derive(Debug)]
    pub struct ListEntry {
        /// Identifier
        pub id: Id,
        /// Optional title
        pub title: Option<String>,
        /// If entry is encrypted
        pub is_encrypted: bool,
        /// Expiration if set
        pub expiration: Option<String>,
        /// If entry is expired
        pub is_expired: bool,
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
                    metadata: self.metadata,
                    must_be_deleted: self.must_be_deleted,
                }),
                (Some(nonce), Some(password)) => {
                    let encrypted = Encrypted::new(self.data, nonce);
                    let decrypted = encrypted.decrypt(password).await?;
                    Ok(CompressedReadEntry {
                        data: decrypted,
                        metadata: self.metadata,
                        must_be_deleted: self.must_be_deleted,
                    })
                }
            }
        }
    }

    impl CompressedReadEntry {
        pub async fn decompress(self) -> Result<UncompressedEntry, Error> {
            let mut decoder = ZstdDecoder::new(Cursor::new(self.data));
            let mut text = String::new();

            decoder
                .read_to_string(&mut text)
                .await
                .map_err(|e| Error::Compression(e.to_string()))?;

            Ok(UncompressedEntry {
                text,
                metadata: self.metadata,
                must_be_deleted: self.must_be_deleted,
            })
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        match err {
            rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
            _ => Error::Sqlite(err),
        }
    }
}

impl Handler {
    /// Create new database with the given `method`.
    fn new(method: Open, receiver: kanal::Receiver<Command>) -> Result<Self, Error> {
        tracing::debug!("opening {method:?}");

        let mut conn = match method {
            Open::Memory => Connection::open_in_memory()?,
            Open::Path(path) => Connection::open(path)?,
        };

        let migrations = Migrations::new(vec![
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
                        let data =
                            zstd::stream::encode_all(cursor, zstd::DEFAULT_COMPRESSION_LEVEL)
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
        ]);

        migrations.to_latest(&mut conn)?;

        Ok(Self { conn, receiver })
    }

    /// Run database command loop.
    fn run(mut self) -> Result<(), Error> {
        loop {
            let command = self.receiver.recv()?;

            match command {
                Command::Insert { entry, result } => {
                    result
                        .send(self.insert(entry))
                        .map_err(|_| Error::ResultSendError)?;
                }
                Command::Get { id, result } => {
                    result
                        .send(self.get(id))
                        .map_err(|_| Error::ResultSendError)?;
                }
                Command::GetMetadata { id, result } => {
                    result
                        .send(self.get_metadata(id))
                        .map_err(|_| Error::ResultSendError)?;
                }
                Command::Delete { id, result } => {
                    result
                        .send(self.delete(id))
                        .map_err(|_| Error::ResultSendError)?;
                }
                Command::DeleteMany { ids, result } => {
                    result
                        .send(self.delete_many(ids))
                        .map_err(|_| Error::ResultSendError)?;
                }
                Command::DeleteFor { id, uid, result } => {
                    result
                        .send(self.delete_for(id, uid))
                        .map_err(|_| Error::ResultSendError)?;
                }
                Command::NextUid { result } => {
                    result
                        .send(self.next_uid())
                        .map_err(|_| Error::ResultSendError)?;
                }
                Command::List { result } => {
                    result
                        .send(self.list())
                        .map_err(|_| Error::ResultSendError)?;
                }
                Command::Purge { result } => {
                    result
                        .send(self.purge())
                        .map_err(|_| Error::ResultSendError)?;
                }
            }
        }
    }

    fn insert(
        &self,
        write::DatabaseEntry { entry, data, nonce }: write::DatabaseEntry,
    ) -> Result<(Id, write::Entry), Error> {
        let mut counter = 0;

        loop {
            let id = Id::rand();

            let result = match entry.expires {
                    None => self.conn.execute(
                        "INSERT INTO entries (id, uid, data, burn_after_reading, nonce, title) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![id.to_i64(), entry.uid, data, entry.burn_after_reading, nonce, entry.title],
                    ),
                    Some(expires) => self.conn.execute(
                        "INSERT INTO entries (id, uid, data, burn_after_reading, nonce, expires, title) VALUES (?1, ?2, ?3, ?4, ?5, datetime('now', ?6), ?7)",
                        params![
                            id.to_i64(),
                            entry.uid,
                            data,
                            entry.burn_after_reading,
                            nonce,
                            format!("{expires} seconds"),
                            entry.title,
                        ],
                    ),
                };

            match result {
                Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error {
                        code,
                        extended_code,
                    },
                    Some(ref _message),
                )) if code == rusqlite::ErrorCode::ConstraintViolation
                    && extended_code == 1555
                    && counter < 10 =>
                {
                    /* Retry if ID is already existent */
                    counter += 1;
                    continue;
                }
                Err(err) => break Err(err)?,
                Ok(rows) => {
                    debug_assert!(rows == 1);
                    return Ok((id, entry));
                }
            }
        }
    }

    fn get_metadata(&self, id: Id) -> Result<Metadata, Error> {
        let metadata = self.conn.query_row(
            "SELECT uid, title, CAST(ROUND((julianday(expires) - julianday('now')) * 86400) AS INTEGER) FROM entries WHERE id=?1",
            params![id.to_i64()],
            |row| {
                let expiration = row.get::<_, Option<i64>>(2)?
                    .filter(|secs| *secs > 0)
                    .and_then(|secs| u64::try_from(secs).ok())
                    .map(|secs| Expiration { duration: Duration::from_secs(secs), default: false });

                Ok(read::Metadata {
                    uid: row.get(0)?,
                    title: row.get::<_, Option<String>>(1)?,
                    expiration,
                })
            }
        )?;

        Ok(metadata)
    }

    fn get(&self, id: Id) -> Result<DatabaseEntry, Error> {
        let metadata = self.get_metadata(id)?;
        let entry = self.conn.query_row(
            "SELECT data, burn_after_reading, nonce, expires < datetime('now') FROM entries WHERE id=?1",
            params![id.to_i64()],
            |row| {
                Ok(read::DatabaseEntry {
                    data: row.get(0)?,
                    metadata,
                    must_be_deleted: row.get::<_, Option<bool>>(1)?.unwrap_or(false),
                    nonce: row.get(2)?,
                    expired: row.get::<_, Option<bool>>(3)?.unwrap_or(false),
                })
            },
        )?;

        Ok(entry)
    }

    fn delete(&self, id: Id) -> Result<(), Error> {
        self.conn
            .execute("DELETE FROM entries WHERE id=?1", params![id.to_i64()])?;

        Ok(())
    }

    fn delete_many(&mut self, ids: Vec<Id>) -> Result<usize, Error> {
        let tx = self.conn.transaction()?;

        let mut affected = 0;

        for id in ids {
            affected += tx.execute("DELETE FROM entries WHERE id=?1", params![id.to_i64()])?;
        }

        tx.commit()?;
        Ok(affected)
    }

    fn delete_for(&mut self, id: Id, uid: i64) -> Result<(), Error> {
        let tx = self.conn.transaction()?;

        let exists = match tx.query_row(
            "SELECT 1 FROM entries WHERE (id=?1 AND uid=?2)",
            params![id.to_i64(), uid],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(_) => true,
            Err(rusqlite::Error::QueryReturnedNoRows) => false,
            Err(e) => return Err(Error::Sqlite(e)),
        };

        if !exists {
            return Err(Error::Delete);
        }

        tx.execute(
            "DELETE FROM entries WHERE (id=?1 AND uid=?2)",
            params![id.to_i64(), uid],
        )?;

        tx.commit()?;
        Ok(())
    }

    fn next_uid(&self) -> Result<i64, Error> {
        let uid = self.conn.query_row(
            "UPDATE uids SET n = n + 1 WHERE id = 0 RETURNING n",
            [],
            |row| row.get(0),
        )?;

        Ok(uid)
    }

    fn list(&self) -> Result<Vec<ListEntry>, Error> {
        let entries = self
            .conn
            .prepare("SELECT id, title, nonce, expires, expires < datetime('now') FROM entries")?
            .query_map([], |row| {
                Ok(ListEntry {
                    id: Id::from(row.get::<_, i64>(0)?),
                    title: row.get(1)?,
                    is_encrypted: row.get::<_, Option<Vec<u8>>>(2)?.is_some(),
                    expiration: row.get(3)?,
                    is_expired: row.get::<_, Option<bool>>(4)?.unwrap_or_default(),
                })
            })?
            .collect::<Result<_, _>>()?;

        Ok(entries)
    }

    fn purge(&self) -> Result<Vec<Id>, Error> {
        let ids = self
            .conn
            .prepare("DELETE FROM entries WHERE expires < datetime('now') RETURNING id")?
            .query_map([], |row| Ok(Id::from(row.get::<_, i64>(0)?)))?
            .collect::<Result<_, _>>()?;

        Ok(ids)
    }
}

impl Database {
    /// Create new database with the given `method` as well as a [`Handler`] future that makes the
    /// actual calls.
    pub fn new(method: Open) -> Result<(Self, impl Future<Output = Result<(), Error>>), Error> {
        let (sender, receiver) = kanal::bounded(256);
        let sender = sender.to_async();
        let handler = Handler::new(method, receiver)?;
        let fut = async move { tokio::task::spawn_blocking(|| handler.run()).await? };
        Ok((Self { sender }, fut))
    }

    /// Insert `entry` under `id` into the database and optionally set owner to `uid`.
    pub async fn insert(&self, entry: write::Entry) -> Result<(Id, write::Entry), Error> {
        let entry = entry.compress().await?.encrypt().await?;

        let (result, command_result) = oneshot::channel();
        self.sender
            .send(Command::Insert { entry, result })
            .await
            .map_err(|_| Error::SendError)?;

        command_result.await?
    }

    /// Get entire entry for `id`.
    pub async fn get(&self, id: Id, password: Option<Password>) -> Result<read::Entry, Error> {
        let (result, command_result) = oneshot::channel();
        self.sender
            .send(Command::Get { id, result })
            .await
            .map_err(|_| Error::SendError)?;

        let entry = command_result.await??;

        if entry.expired {
            self.delete(id).await?;
            return Err(Error::NotFound);
        }

        let read::UncompressedEntry {
            text,
            metadata,
            must_be_deleted,
        } = entry.decrypt(password).await?.decompress().await?;

        let data = read::Data { text, metadata };

        if must_be_deleted {
            self.delete(id).await?;
            return Ok(read::Entry::Burned(data));
        }

        Ok(read::Entry::Regular(data))
    }

    /// Get metadata of a paste.
    pub async fn get_metadata(&self, id: Id) -> Result<Metadata, Error> {
        let (result, command_result) = oneshot::channel();
        self.sender
            .send(Command::GetMetadata { id, result })
            .await
            .map_err(|_| Error::SendError)?;
        command_result.await?
    }

    /// Delete paste with `id`.
    async fn delete(&self, id: Id) -> Result<(), Error> {
        let (result, command_result) = oneshot::channel();
        self.sender
            .send(Command::Delete { id, result })
            .await
            .map_err(|_| Error::SendError)?;
        command_result.await?
    }

    /// Delete pastes with `ids`.
    pub async fn delete_many(&self, ids: Vec<Id>) -> Result<usize, Error> {
        let (result, command_result) = oneshot::channel();
        self.sender
            .send(Command::DeleteMany { ids, result })
            .await
            .map_err(|_| Error::SendError)?;
        command_result.await?
    }

    /// Delete paste with `id` for user `uid`.
    pub async fn delete_for(&self, id: Id, uid: i64) -> Result<(), Error> {
        let (result, command_result) = oneshot::channel();
        self.sender
            .send(Command::DeleteFor { id, uid, result })
            .await
            .map_err(|_| Error::SendError)?;
        command_result.await?
    }

    /// Retrieve next monotonically increasing uid.
    pub async fn next_uid(&self) -> Result<i64, Error> {
        let (result, command_result) = oneshot::channel();
        self.sender
            .send(Command::NextUid { result })
            .await
            .map_err(|_| Error::SendError)?;
        command_result.await?
    }

    /// List all entries.
    pub async fn list(&self) -> Result<Vec<ListEntry>, Error> {
        let (result, command_result) = oneshot::channel();
        self.sender
            .send(Command::List { result })
            .await
            .map_err(|_| Error::SendError)?;
        command_result.await?
    }

    /// Purge all expired entries and return their [`Id`]s
    pub async fn purge(&self) -> Result<Vec<Id>, Error> {
        let (result, command_result) = oneshot::channel();
        self.sender
            .send(Command::Purge { result })
            .await
            .map_err(|_| Error::SendError)?;
        command_result.await?
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::*;

    impl read::Entry {
        /// Unwrap inner data or panic.
        #[must_use]
        pub fn unwrap_inner(self) -> read::Data {
            match self {
                read::Entry::Regular(data) | read::Entry::Burned(data) => data,
            }
        }
    }

    fn new_db() -> Result<Database, Box<dyn std::error::Error>> {
        let (db, handler) = Database::new(Open::Memory)?;
        tokio::spawn(handler);
        Ok(db)
    }

    #[tokio::test]
    async fn insert() -> Result<(), Box<dyn std::error::Error>> {
        let db = new_db()?;

        let entry = write::Entry {
            text: "hello world".to_string(),
            uid: Some(10),
            ..Default::default()
        };

        let (id, _entry) = db.insert(entry).await?;

        let entry = db.get(id, None).await?.unwrap_inner();
        assert_eq!(entry.text, "hello world");
        assert!(entry.metadata.uid.is_some());
        assert_eq!(entry.metadata.uid.unwrap(), 10);

        let result = db.get(Id::from(5678u32), None).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn next_uid() -> Result<(), Box<dyn std::error::Error>> {
        let db = new_db()?;

        let uid1 = db.next_uid().await?;
        let uid2 = db.next_uid().await?;
        let uid3 = db.next_uid().await?;

        assert!(uid1 < uid2);
        assert!(uid2 < uid3);

        Ok(())
    }

    #[tokio::test]
    async fn expired_does_not_exist() -> Result<(), Box<dyn std::error::Error>> {
        let db = new_db()?;

        let entry = write::Entry {
            expires: Some(NonZeroU32::new(1).unwrap()),
            ..Default::default()
        };

        let (id, _entry) = db.insert(entry).await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let result = db.get(id, None).await;
        assert!(matches!(result, Err(Error::NotFound)));

        Ok(())
    }

    #[tokio::test]
    async fn delete() -> Result<(), Box<dyn std::error::Error>> {
        let db = new_db()?;

        let (id, _entry) = db.insert(write::Entry::default()).await?;

        assert!(db.get(id, None).await.is_ok());
        assert!(db.delete(id).await.is_ok());
        assert!(db.get(id, None).await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn delete_for() -> Result<(), Box<dyn std::error::Error>> {
        let db = new_db()?;

        let id = Id::from(1234u32);
        let uid = 42;

        let entry = write::Entry {
            uid: Some(uid),
            ..Default::default()
        };
        db.insert(id, entry).await?;

        assert!(db.get(id, None).await.is_ok());
        assert!(db.delete_for(id, uid).await.is_ok());
        assert!(db.get(id, None).await.is_err());

        let entry = write::Entry {
            uid: Some(uid),
            ..Default::default()
        };
        db.insert(id, entry).await?;

        let incorrect_uid = 99;
        assert!(matches!(
            db.delete_for(id, incorrect_uid).await,
            Err(Error::Delete)
        ));

        assert!(db.get(id, None).await.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn purge() -> Result<(), Box<dyn std::error::Error>> {
        let db = new_db()?;

        let entry = write::Entry {
            expires: Some(NonZeroU32::new(1).unwrap()),
            ..Default::default()
        };

        let (id, _entry) = db.insert(entry).await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let ids = db.purge().await?;
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], id);

        Ok(())
    }

    #[tokio::test]
    async fn get_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let db = new_db()?;

        let entry = write::Entry {
            text: "test content".to_string(),
            uid: Some(42),
            title: Some("Test Title".to_string()),
            expires: Some(NonZeroU32::new(3600).unwrap()),
            ..Default::default()
        };

        let id = Id::from(5678u32);
        db.insert(id, entry).await?;

        let metadata = db.get_metadata(id).await?;
        assert_eq!(metadata.uid, Some(42));
        assert_eq!(metadata.title, Some("Test Title".to_string()));

        let expiration = metadata.expiration.unwrap().duration.as_secs();
        assert!(expiration <= 3600);
        // We have a problem if the test takes more than 10 seconds.
        assert!(expiration >= 3590);

        Ok(())
    }
}
