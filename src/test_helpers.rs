use crate::cache::Cache;
use crate::db::{self, Database};
use axum::extract::Request;
use axum::response::Response;
use axum::Router;
use axum_extra::extract::cookie::Key;
use reqwest::RequestBuilder;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::make::Shared;
use tower_service::Service;

pub(crate) struct Client {
    client: reqwest::Client,
    addr: SocketAddr,
}

impl Client {
    pub(crate) async fn new<S>(svc: S) -> Self
    where
        S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
        S::Future: Send,
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Could not bind ephemeral socket");

        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, Shared::new(svc)).await.unwrap();
        });

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .cookie_store(true)
            .build()
            .unwrap();

        Self { client, addr }
    }

    pub(crate) fn get(&self, url: &str) -> RequestBuilder {
        self.client.get(format!("http://{}{}", self.addr, url))
    }

    pub(crate) fn post(&self, url: &str) -> RequestBuilder {
        self.client.post(format!("http://{}{}", self.addr, url))
    }
}

pub(crate) fn make_app() -> Result<Router, Box<dyn std::error::Error>> {
    let db = Database::new(db::Open::Memory)?;
    let cache = Cache::new(NonZeroUsize::new(128).unwrap());
    let key = Key::generate();
    let base_url = None;
    let state = crate::AppState {
        db,
        cache,
        key,
        base_url,
    };

    Ok(crate::make_app(4096, Duration::new(30, 0)).with_state(state))
}
