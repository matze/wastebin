use crate::Database;
use crate::errors::{Error, JsonErrorResponse};
use crate::handlers::extract::Uid;
use axum::extract::{Path, State};

pub async fn delete(
    Path(id): Path<String>,
    State(db): State<Database>,
    Uid(uid): Uid,
) -> Result<(), JsonErrorResponse> {
    let id = id.parse()?;
    let db_uid = db.get_uid(id).await?;
    let can_delete = db_uid.map(|db_uid| uid == db_uid).unwrap_or(false);

    if !can_delete {
        Err(Error::Delete)?;
    }

    db.delete(id).await?;

    Ok(())
}

#[cfg(test)]
mod tests {}
