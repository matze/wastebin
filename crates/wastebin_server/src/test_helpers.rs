use crate::cache::Cache;
use crate::expiration::ExpirationSet;
use crate::highlight::{Highlighter, Theme};
use crate::page;
use axum_extra::extract::cookie::Key;
use reqwest::RequestBuilder;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use wastebin_core::db::{self, Database};

pub(crate) struct Client {
    client: reqwest::Client,
    addr: SocketAddr,
}

/// Determine if the client should store cookies.
pub(crate) struct StoreCookies(pub bool);

impl Client {
    pub(crate) async fn new(store_cookies: StoreCookies) -> Self {
        let (db, handler) = Database::new(db::Open::Memory).expect("open memory database");
        let cache = Cache::new(NonZeroUsize::new(128).unwrap());
        let key = Key::generate();
        let expirations = "0".parse::<ExpirationSet>().unwrap();
        let page = Arc::new(page::Page::new(
            String::from("test"),
            url::Url::parse("https://localhost:8888").unwrap(),
            Theme::Ayu,
            expirations,
        ));
        let state = crate::AppState {
            db,
            cache,
            key,
            page,
            highlighter: Arc::new(Highlighter::default()),
        };

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Could not bind ephemeral socket");

        let addr = listener.local_addr().unwrap();

        tokio::spawn(handler);

        tokio::spawn(async move {
            let app = crate::make_app(state, Duration::from_secs(30), 1024 * 1024);

            axum::serve(listener, app)
                .with_graceful_shutdown(crate::shutdown_signal())
                .await
                .unwrap();
        });

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .cookie_store(store_cookies.0)
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

    pub(crate) fn post_form(&self) -> RequestBuilder {
        self.client.post(format!("http://{}/new", self.addr))
    }

    pub(crate) fn post_json(&self) -> RequestBuilder {
        self.client.post(format!("http://{}/", self.addr))
    }

    pub(crate) fn delete(&self, url: &str) -> RequestBuilder {
        self.client.delete(format!("http://{}{}", self.addr, url))
    }
}
