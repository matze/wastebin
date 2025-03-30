use wastebin_core::{db::write, id::Id};

use crate::AppState;
use crate::Error;
use crate::Error::TooLongExpires;

pub mod api;
pub mod form;

async fn common_insert(appstate: &AppState, id: Id, entry: write::Entry) -> Result<(), Error> {
    if let Some(max_expiration) = appstate.page.max_expiration {
        if entry.expires.is_none_or(|exp| exp > max_expiration) {
            Err(TooLongExpires)?;
        }
    }

    appstate.db.insert(id, entry).await?;

    Ok(())
}
