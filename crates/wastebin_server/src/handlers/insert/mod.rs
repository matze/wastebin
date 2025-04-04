use std::sync::atomic::AtomicBool;

use wastebin_core::{db::write, id::Id};

use crate::AppState;
use crate::Error;
use crate::Error::RateLimit;

pub mod api;
pub mod form;

async fn common_insert(appstate: &AppState, id: Id, entry: write::Entry) -> Result<(), Error> {
    if let Some(ref ratelimiter) = appstate.ratelimit_insert {
        static RL_LOGGED: AtomicBool = AtomicBool::new(false);

        if ratelimiter.try_wait().is_err() {
            if !RL_LOGGED.fetch_or(true, std::sync::atomic::Ordering::Acquire) {
                tracing::info!("Rate limiting paste insertions");
            }

            Err(RateLimit)?;
        }

        RL_LOGGED.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    appstate.db.insert(id, entry).await?;

    Ok(())
}
