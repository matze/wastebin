use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use std::path::PathBuf;
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
    Completion { shell: Shell },
    /// List and filter database entries
    List {
        /// Path to the database file
        #[arg(long, env = vars::DATABASE_PATH)]
        database: PathBuf,
    },
    /// Purge all expired entries and show their identifiers
    Purge {
        /// Path to the database file
        #[arg(long, env = vars::DATABASE_PATH)]
        database: PathBuf,
    },
}

enum Encrypted {
    Yes,
    No,
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
        if value { Self::Yes } else { Self::No }
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
            Encrypted::Yes => write!(f, "🔒"),
            Encrypted::No => Ok(()),
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
        Commands::List { database } => {
            let (db, db_handler) = Database::new(Open::Path(database))?;
            tokio::task::spawn(db_handler);

            let mut table = Table::new(db.list().await?.into_iter().map(Entry::from));
            table.with(Style::psql()).with(Alignment::left());

            println!("{}", table);
        }
        Commands::Purge { database } => {
            let (db, db_handler) = Database::new(Open::Path(database))?;
            tokio::task::spawn(db_handler);

            let ids = db.purge().await?;

            if ids.is_empty() {
                println!("no entries purged");
            } else {
                println!("purged {} expired entries", ids.len());

                for id in ids {
                    println!("{id}");
                }
            }
        }
    }

    Ok(())
}
