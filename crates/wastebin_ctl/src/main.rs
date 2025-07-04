use anyhow::{Context, Result};
#[cfg(feature = "completion")]
use clap::CommandFactory;
use clap::{Args, Parser, Subcommand, ValueEnum};
#[cfg(feature = "completion")]
use clap_complete::{Shell, generate};
use std::path::PathBuf;
use std::str::FromStr;
use tabled::derive::display;
use tabled::settings::{Alignment, Style};
use tabled::{Table, Tabled};
use wastebin_core::db::read::ListEntry;
use wastebin_core::db::{Database, Open};
use wastebin_core::env::vars;
use wastebin_core::id::Id;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate shell completion.
    #[cfg(feature = "completion")]
    Completion { shell: Shell },
    /// List and filter database entries
    List {
        /// Path to the database file
        #[arg(long, env = vars::DATABASE_PATH)]
        database: PathBuf,

        /// List entry with the given identifier
        #[arg(short, long)]
        identifier: Option<String>,

        /// List entries containing the given string (case-sensitive)
        #[arg(short, long)]
        title: Option<String>,

        /// List only entries with the given encryption status
        #[arg(short, long)]
        encrypted: Option<Encrypted>,

        #[command(flatten)]
        expired_filter: ExpiredFilter,

        /// Order the results
        #[arg(short, long)]
        sort: Option<SortOrder>,
    },
    /// Delete specific entries
    Delete {
        /// Path to the database file
        #[arg(long, env = vars::DATABASE_PATH)]
        database: PathBuf,

        /// Delete entry with the given identifiers
        identifier: Vec<String>,
    },
    /// Purge all expired entries and show their identifiers
    Purge {
        /// Path to the database file
        #[arg(long, env = vars::DATABASE_PATH)]
        database: PathBuf,
    },
}

#[derive(Args)]
#[group(required = false, multiple = false)]
struct ExpiredFilter {
    /// List only expired entries
    #[arg(long)]
    expired: bool,

    /// List only non-expired entries
    #[arg(long)]
    active: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum SortOrder {
    TitleAsc,
    TitleDesc,
    ExpirationAsc,
    ExpirationDesc,
}

#[derive(Clone, Copy, ValueEnum)]
enum Encrypted {
    Encrypted,
    Plain,
}

enum Expired {
    Yes,
    No,
}

#[derive(Tabled)]
struct Entry {
    id: Id,
    #[tabled(display("display::option", ""))]
    title: Option<String>,
    encrypted: Encrypted,
    #[tabled(display("display::option", ""))]
    expiration: Option<String>,
    expired: Expired,
}

impl From<bool> for Encrypted {
    fn from(value: bool) -> Self {
        if value { Self::Encrypted } else { Self::Plain }
    }
}

impl From<bool> for Expired {
    fn from(value: bool) -> Self {
        if value { Self::Yes } else { Self::No }
    }
}

impl std::fmt::Display for Encrypted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Encrypted::Encrypted => write!(f, "🔒"),
            Encrypted::Plain => Ok(()),
        }
    }
}

impl std::fmt::Display for Expired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expired::Yes => write!(f, "☑️"),
            Expired::No => Ok(()),
        }
    }
}

impl From<ListEntry> for Entry {
    fn from(entry: ListEntry) -> Self {
        Self {
            id: entry.id,
            title: entry.title,
            encrypted: entry.is_encrypted.into(),
            expiration: entry.expiration,
            expired: entry.is_expired.into(),
        }
    }
}

#[allow(clippy::print_stdout)]
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.commands {
        #[cfg(feature = "completion")]
        Commands::Completion { shell } => {
            let mut cmd = Cli::command();
            let cmd = &mut cmd;

            generate(
                shell,
                cmd,
                cmd.get_name().to_string(),
                &mut std::io::stdout(),
            );
        }
        Commands::List {
            database,
            identifier,
            title,
            encrypted,
            expired_filter,
            sort,
        } => {
            let identifier = identifier
                .map(|id| Id::from_str(&id))
                .transpose()
                .with_context(|| "Invalid identifier")?;

            let (db, db_handler) = Database::new(Open::Path(database))?;
            tokio::task::spawn(db_handler);

            let mut db_items: Vec<_> = db
                .list()
                .await?
                .into_iter()
                .filter(|entry| identifier.is_none_or(|ident| entry.id == ident))
                .filter(|entry| {
                    title.as_ref().is_none_or(|t| {
                        entry
                            .title
                            .as_ref()
                            .is_some_and(|entry_t| entry_t.contains(t))
                    })
                })
                .filter(|entry| match encrypted {
                    Some(Encrypted::Encrypted) => entry.is_encrypted,
                    Some(Encrypted::Plain) => !entry.is_encrypted,
                    None => true,
                })
                .filter(|entry| {
                    if expired_filter.active {
                        !entry.is_expired
                    } else if expired_filter.expired {
                        entry.is_expired
                    } else {
                        true
                    }
                })
                .map(Entry::from)
                .collect();

            if let Some(sort_order) = sort {
                match sort_order {
                    SortOrder::TitleAsc => db_items.sort_unstable_by(|a, b| a.title.cmp(&b.title)),
                    SortOrder::TitleDesc => {
                        db_items.sort_unstable_by(|a, b| a.title.cmp(&b.title).reverse())
                    }
                    SortOrder::ExpirationAsc => {
                        db_items.sort_unstable_by(|a, b| a.expiration.cmp(&b.expiration))
                    }
                    SortOrder::ExpirationDesc => {
                        db_items.sort_unstable_by(|a, b| a.expiration.cmp(&b.expiration).reverse())
                    }
                }
            }

            let mut table = Table::new(&db_items);
            table.with(Style::psql()).with(Alignment::left());

            println!("{table}");
        }
        Commands::Delete {
            database,
            identifier,
        } => {
            let ids = identifier
                .iter()
                .map(|id| Id::from_str(id))
                .collect::<Result<Vec<_>, _>>()
                .with_context(|| "Invalid identifier")?;

            let (db, db_handler) = Database::new(Open::Path(database))?;
            tokio::task::spawn(db_handler);

            let affected = db.delete_many(ids).await?;
            println!(
                "Deleted {affected} {}",
                if affected > 1 { "entries" } else { "entry" }
            );
        }
        Commands::Purge { database } => {
            let (db, db_handler) = Database::new(Open::Path(database))?;
            tokio::task::spawn(db_handler);

            let ids = db.purge().await?;

            if ids.is_empty() {
                println!("no entries purged");
            } else {
                println!(
                    "purged {} expired {}",
                    ids.len(),
                    if ids.len() > 1 { "entries" } else { "entry" }
                );

                for id in ids {
                    println!("{id}");
                }
            }
        }
    }

    Ok(())
}
