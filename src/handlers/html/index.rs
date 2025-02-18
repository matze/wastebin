use crate::{handlers::extract::Theme, Highlighter, Page};
use askama::Template;
use axum::extract::State;

/// GET handler for the index page.
pub async fn get(
    State(page): State<Page>,
    State(highlighter): State<Highlighter>,
    theme: Option<Theme>,
) -> Index {
    Index {
        page,
        theme,
        highlighter,
    }
}

/// Index page displaying a form for paste insertion and a selection box for languages.
#[derive(Template)]
#[template(path = "index.html")]
pub struct Index {
    page: Page,
    theme: Option<Theme>,
    highlighter: Highlighter,
}
