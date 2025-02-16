use crate::Assets;
use url::Url;

pub struct Expiration {
    pub repr: &'static str,
    pub seconds: u32,
    pub selected: bool,
}

pub struct Page {
    pub version: &'static str,
    pub title: String,
    pub assets: Assets,
    pub base_url: Url,
    pub expirations: Vec<Expiration>,
}

impl Expiration {
    /// Create a new [`Expiration`] from the human-readable `repr` and given seconds.
    const fn new(repr: &'static str, seconds: u32) -> Self {
        Self {
            repr,
            seconds,
            selected: false,
        }
    }
}

impl Page {
    /// Create new page meta data from generated  `assets`, `title` and optional `base_url`.
    #[must_use]
    pub fn new(assets: Assets, title: String, base_url: Url, max_expiration: Option<u32>) -> Self {
        const OPTIONS: [Expiration; 7] = [
            Expiration::new("never", u32::MAX),
            Expiration::new("10 minutes", 600),
            Expiration::new("1 hour", 3600),
            Expiration::new("1 day", 86400),
            Expiration::new("1 week", 604_800),
            Expiration::new("1 month", 2_592_000),
            Expiration::new("1 year", 31_536_000),
        ];

        let expirations = OPTIONS
            .into_iter()
            .filter(|expiration| max_expiration.map_or(true, |max| expiration.seconds <= max))
            .collect();

        Self {
            version: env!("CARGO_PKG_VERSION"),
            title,
            assets,
            base_url,
            expirations,
        }
    }
}
