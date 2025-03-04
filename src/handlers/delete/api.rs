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
    let can_delete = db_uid.is_some_and(|db_uid| uid == db_uid);

    if !can_delete {
        Err(Error::Delete)?;
    }

    db.delete(id).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::Client;
    use reqwest::StatusCode;

    #[tokio::test]
    async fn delete() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new().await;

        let data = crate::handlers::insert::form::Entry {
            text: "FooBarBaz".to_string(),
            extension: None,
            expires: Some("0".to_string()),
            password: "".to_string(),
            title: "".to_string(),
            burn_after_reading: None,
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;
        let id = location.replace("/", "");

        let res = client.delete(&format!("/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client.get(&format!("/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }
}
