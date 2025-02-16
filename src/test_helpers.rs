use crate::cache::Cache;
use crate::db::{self, Database};
use crate::highlight::{Highlighter, Theme};
use crate::page;
use axum_extra::extract::cookie::Key;
use reqwest::RequestBuilder;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

pub(crate) struct Client {
    client: reqwest::Client,
    addr: SocketAddr,
}

impl Client {
    pub(crate) async fn new() -> Self {
        let db = Database::new(db::Open::Memory).expect("open memory database");
        let cache = Cache::new(NonZeroUsize::new(128).unwrap());
        let key = Key::generate();
        let page = Arc::new(page::Page::new(
            String::from("test"),
            url::Url::parse("https://localhost:8888").unwrap(),
            Theme::Ayu,
            None,
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

        tokio::spawn(async move {
            crate::serve(listener, state, Duration::new(30, 0), 1024 * 1024)
                .await
                .unwrap();
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
