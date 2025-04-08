use anyhow::Result;
use clap::{Parser, Subcommand};
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
    /// Path to the database file
    #[arg(long, env = vars::DATABASE_PATH)]
    database: PathBuf,

    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List and filter database entries
    List,
    /// Purge all expired entries and show their identifiers
    Purge,
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
            expired: entry.is_expired.into(),
        }
    }
}

#[allow(clippy::print_stdout)]
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let db = Database::new(Open::Path(cli.database))?;

    match &cli.commands {
        Commands::List => {
            let mut table = Table::new(db.list().await?.into_iter().map(Entry::from));
            table.with(Style::psql()).with(Alignment::left());

            println!("{}", table);
        }
        Commands::Purge => {
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
