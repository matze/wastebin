use crate::handlers::extract::{Theme, Uid};
use crate::handlers::html::{ErrorResponse, make_error};
use crate::{Database, Page};
use axum::extract::{Path, State};
use axum::response::Redirect;

pub async fn delete(
    Path(id): Path<String>,
    State(db): State<Database>,
    State(page): State<Page>,
    Uid(uid): Uid,
    theme: Option<Theme>,
) -> Result<Redirect, ErrorResponse> {
    async {
        let id = id.parse()?;
        db.delete_for(id, uid).await?;
        Ok(Redirect::to("/"))
    }
    .await
    .map_err(|err| make_error(err, page.clone(), theme))
}

#[cfg(test)]
mod tests {
    use crate::handlers::insert::form::Entry;
    use crate::test_helpers::{Client, StoreCookies};
    use reqwest::StatusCode;

    #[tokio::test]
    async fn delete_via_link() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(true)).await;

        let res = client.post_form().form(&Entry::default()).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;
        let id = location.replace("/", "");

        let res = client.get(&format!("/delete/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let res = client.get(&format!("/{id}")).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }
}
