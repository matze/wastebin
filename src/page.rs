use crate::assets::{Asset, Css, Kind};
use crate::highlight::Theme;
use url::Url;

/// Static page assets.
pub(crate) struct Assets {
    pub favicon: Asset,
    pub css: Css,
    pub base_js: Asset,
    pub index_js: Asset,
    pub paste_js: Asset,
}

pub(crate) struct Expiration {
    pub repr: &'static str,
    pub seconds: u32,
    pub selected: bool,
}

pub(crate) struct Page {
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
    pub fn new(title: String, base_url: Url, theme: Theme, max_expiration: Option<u32>) -> Self {
        const OPTIONS: [Expiration; 7] = [
            Expiration::new("never", u32::MAX),
            Expiration::new("10 minutes", 600),
            Expiration::new("1 hour", 3600),
            Expiration::new("1 day", 86400),
            Expiration::new("1 week", 604_800),
            Expiration::new("1 month", 2_592_000),
            Expiration::new("1 year", 31_536_000),
        ];

        let assets = Assets::new(theme);

        let expirations = OPTIONS
            .into_iter()
            .filter(|expiration| max_expiration.is_none_or(|max| expiration.seconds <= max))
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

impl Assets {
    /// Create page [`Assets`] for the given `theme`.
    fn new(theme: Theme) -> Self {
        Self {
            favicon: Asset::new(
                "favicon.ico",
                mime::IMAGE_PNG,
                include_bytes!("../assets/favicon.png").to_vec(),
            ),
            css: Css::new(theme),
            base_js: Asset::new_hashed(
                "base",
                Kind::Js,
                include_bytes!("javascript/base.js").to_vec(),
            ),
            index_js: Asset::new_hashed(
                "index",
                Kind::Js,
                include_bytes!("javascript/index.js").to_vec(),
            ),
            paste_js: Asset::new_hashed(
                "paste",
                Kind::Js,
                include_bytes!("javascript/paste.js").to_vec(),
            ),
        }
    }
}
