use axum::extract::{Path, State};

use crate::Database;
use crate::errors::{Error, JsonErrorResponse};
use crate::handlers::extract::Uids;

pub async fn delete(
    Path(id): Path<String>,
    State(db): State<Database>,
    Uids(uids): Uids,
) -> Result<(), JsonErrorResponse> {
    let id = id.parse().map_err(Error::Id)?;
    db.delete_for(id, &uids).await.map_err(Error::Database)?;
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
        let id = location.replace('/', "");

        let res = client.delete(&format!("/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client.get(&format!("/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn delete_without_cookie() -> Result<(), Box<dyn std::error::Error>> {
        let owner = Client::new(StoreCookies(true)).await;

        let res = owner.post_form().form(&Entry::default()).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;
        let id = location.replace('/', "");

        // A fresh client sends no `uid` cookie, so the `Uids` extractor rejects the request before
        // it can reach the database.
        let anonymous = Client::new(StoreCookies(false)).await;
        let res = anonymous.delete(&format!("/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        // The paste is untouched and still readable by its owner.
        let res = owner.get(&format!("/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        Ok(())
    }
}
