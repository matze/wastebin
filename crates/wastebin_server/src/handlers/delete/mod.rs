use std::sync::atomic::AtomicBool;

use wastebin_core::id::Id;

use crate::AppState;
use crate::Error;
use crate::Error::RateLimit;

pub mod api;
pub mod form;

async fn common_delete(appstate: &AppState, id: Id, uid: i64) -> Result<(), Error> {
    if let Some(ref ratelimiter) = appstate.ratelimit_delete {
        static RL_LOGGED: AtomicBool = AtomicBool::new(false);

        if ratelimiter.try_wait().is_err() {
            if !RL_LOGGED.fetch_or(true, std::sync::atomic::Ordering::Acquire) {
                tracing::info!("Rate limiting paste deletions");
            }

            Err(RateLimit)?;
        }

        RL_LOGGED.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    appstate.db.delete_for(id, uid).await?;

    Ok(())
}
