/// Return robots.txt content.
pub async fn get() -> &'static str {
    r"User-agent: *
Disallow: /"
}
