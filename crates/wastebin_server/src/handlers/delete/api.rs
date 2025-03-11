use crate::Database;
use crate::errors::{Error, JsonErrorResponse};
use crate::handlers::extract::Uid;
use axum::extract::{Path, State};

pub async fn delete(
    Path(id): Path<String>,
    State(db): State<Database>,
    Uid(uid): Uid,
) -> Result<(), JsonErrorResponse> {
    let id = id.parse().map_err(Error::Id)?;
    db.delete_for(id, uid).await.map_err(Error::Database)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::handlers::insert::form::Entry;
    use crate::test_helpers::{Client, StoreCookies};
    use reqwest::StatusCode;

    #[tokio::test]
    async fn delete() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(true)).await;

        let res = client.post_form().form(&Entry::default()).send().await?;
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
