use crate::{AppState, Highlighter, Page};
use askama::Template;
use axum::extract::State;
use std::num::{NonZero, NonZeroU32};
use std::sync::OnceLock;

/// GET handler for the index page.
pub async fn index(
    State(state): State<AppState>,
    State(page): State<Page>,
    State(highlighter): State<Highlighter>,
) -> Index {
    Index {
        page,
        max_expiration: state.max_expiration,
        highlighter,
    }
}

/// Index page displaying a form for paste insertion and a selection box for languages.
#[derive(Template)]
#[template(path = "index.html")]
pub struct Index {
    page: Page,
    max_expiration: Option<NonZeroU32>,
    highlighter: Highlighter,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Expiration {
    None,
    Burn,
    Time(NonZeroU32),
}

impl std::fmt::Display for Expiration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expiration::None => write!(f, ""),
            Expiration::Burn => write!(f, "burn"),
            Expiration::Time(t) => write!(f, "{t}"),
        }
    }
}

#[allow(clippy::unwrap_used)]
const EXPIRATION_OPTIONS: [(&str, Expiration); 7] = [
    ("never", Expiration::None),
    ("10 minutes", Expiration::Time(NonZero::new(600).unwrap())),
    ("1 hour", Expiration::Time(NonZero::new(3600).unwrap())),
    ("1 day", Expiration::Time(NonZero::new(86400).unwrap())),
    ("1 week", Expiration::Time(NonZero::new(604_800).unwrap())),
    (
        "1 month",
        Expiration::Time(NonZero::new(2_592_000).unwrap()),
    ),
    (
        "1 year",
        Expiration::Time(NonZero::new(31_536_000).unwrap()),
    ),
];

impl Index {
    fn expiry_options(&self) -> &str {
        static EXPIRATION_OPTIONS_HTML: OnceLock<String> = OnceLock::new();

        EXPIRATION_OPTIONS_HTML.get_or_init(|| {

            let mut option_set = String::new();
            let mut wrote_first = false;

            option_set.push('\n');

            for (opt_name, opt_val) in EXPIRATION_OPTIONS {
                if self.max_expiration.is_none()
                    || opt_val == Expiration::Burn
                    || matches!((self.max_expiration, opt_val), (Some(exp), Expiration::Time(time)) if time <= exp)
                {
                    option_set.push_str("<option");
                    if !wrote_first {
                        option_set.push_str(" selected");
                        wrote_first = true;
                    }
                    option_set.push_str(" value=\"");
                    option_set.push_str(opt_val.to_string().as_ref());
                    option_set.push_str("\">");
                    option_set.push_str(opt_name);
                    option_set.push_str("</option>\n");
                }
            }

            option_set
        })
    }
}
