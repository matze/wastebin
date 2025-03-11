/// Names of environment variables.
pub mod vars {
    /// Address and port the server binds to.
    pub const ADDRESS_PORT: &str = "WASTEBIN_ADDRESS_PORT";
    /// Base URL to use for the QR code link.
    pub const BASE_URL: &str = "WASTEBIN_BASE_URL";
    /// Number of cached items.
    pub const CACHE_SIZE: &str = "WASTEBIN_CACHE_SIZE";
    /// Path to the database file.
    pub const DATABASE_PATH: &str = "WASTEBIN_DATABASE_PATH";
    /// Time before a request times uot.
    pub const HTTP_TIMEOUT: &str = "WASTEBIN_HTTP_TIMEOUT";
    /// Maximum body size.
    pub const MAX_BODY_SIZE: &str = "WASTEBIN_MAX_BODY_SIZE";
    /// Password salt for encryption.
    pub const PASSWORD_SALT: &str = "WASTEBIN_PASSWORD_SALT";
    /// Expirations list.
    pub const PASTE_EXPIRATIONS: &str = "WASTEBIN_PASTE_EXPIRATIONS";
    /// Signing key for signed cookie store.
    pub const SIGNING_KEY: &str = "WASTEBIN_SIGNING_KEY";
    /// Theme to use.
    pub const THEME: &str = "WASTEBIN_THEME";
    /// Title.
    pub const TITLE: &str = "WASTEBIN_TITLE";
}

pub(crate) fn password_hash_salt() -> String {
    std::env::var(vars::PASSWORD_SALT).unwrap_or_else(|_| "somesalt".to_string())
}
