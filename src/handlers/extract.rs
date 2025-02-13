use crate::crypto;
use axum::extract::{Form, FromRequest, Request};
use serde::Deserialize;

pub struct Password(pub crypto::Password);

#[axum::async_trait]
impl<S> FromRequest<S> for Password
where
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        #[derive(Deserialize, Debug)]
        struct Data {
            password: String,
        }

        if let Some(password) = req
            .headers()
            .get("Wastebin-Password")
            .and_then(|header| header.to_str().ok())
        {
            return Ok(Password(password.as_bytes().to_vec().into()));
        }

        if let Some(data) = Option::<Form<Data>>::from_request(req, state)
            .await
            .ok()
            .flatten()
        {
            return Ok(Password(data.password.as_bytes().to_vec().into()));
        }

        Err(())
    }
}
