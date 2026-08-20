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
    async fn delete_without_owner_is_forbidden() -> Result<(), Box<dyn std::error::Error>> {
        // The client never stores the uid cookie, so its own DELETE carries no
        // ownership proof.
        let client = Client::new(StoreCookies(false)).await;

        let res = client.post_form().form(&Entry::default()).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let id = res
            .headers()
            .get("location")
            .unwrap()
            .to_str()?
            .replace('/', "");

        let res = client.delete(&format!("/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        // The paste must survive an unauthorized delete.
        let res = client.get(&format!("/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn delete_with_forged_cookie_is_forbidden() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let res = client.post_form().form(&Entry::default()).send().await?;
        let id = res
            .headers()
            .get("location")
            .unwrap()
            .to_str()?
            .replace('/', "");

        // An unsigned `uid` cookie fails HMAC verification and is dropped.
        let res = client
            .delete(&format!("/{id}"))
            .header(reqwest::header::COOKIE, "uid=99")
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        let res = client.get(&format!("/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        Ok(())
    }
}
