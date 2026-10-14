use std::io::Write as _;
use std::time::Duration;

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum_extra::{TypedHeader, headers};
use sha2::{Digest, Sha256};

use wastebin_highlight::Theme;

/// Buffer size handed to the Brotli encoder.
const BROTLI_BUFFER: usize = 4096;

/// Brotli quality used for precompression.
///
/// Assets are compressed once per process, so a high quality is affordable. Quality 11 costs
/// roughly eight times as much as quality 9 for a few percent less output.
const BROTLI_QUALITY: u32 = 10;

/// Brotli window size used for precompression.
const BROTLI_WINDOW: u32 = 22;

/// Gzip level used for precompression.
const GZIP_LEVEL: u32 = 9;

/// Maximum age assets may be cached for, given that their routes embed a content hash.
const MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30);

/// Content coding an asset is available in.
///
/// The declaration order doubles as the server's preference when a client accepts several
/// codings equally well, most compact first.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Encoding {
    /// No content coding.
    Identity,
    /// Gzip, see RFC 1952.
    Gzip,
    /// Brotli, see RFC 7932.
    Brotli,
}

impl Encoding {
    /// Value for the `Content-Encoding` response header, `None` for identity.
    fn header_value(self) -> Option<&'static str> {
        match self {
            Self::Identity => None,
            Self::Gzip => Some("gzip"),
            Self::Brotli => Some("br"),
        }
    }
}

/// An asset associated with a MIME type.
///
/// Cloning shares the payload buffers because they are held as [`Bytes`].
#[derive(Clone)]
pub(crate) struct Asset {
    /// Route that this will be served under.
    pub route: String,
    /// MIME type of this asset determined for the `ContentType` response header.
    mime: mime::Mime,
    /// Uncompressed asset content.
    identity: Bytes,
    /// Brotli encoded content, `None` if that is not smaller than [`Self::identity`].
    brotli: Option<Bytes>,
    /// Gzip encoded content, `None` if that is not smaller than [`Self::identity`].
    gzip: Option<Bytes>,
}

/// Asset kind.
#[derive(Copy, Clone)]
pub(crate) enum Kind {
    Css,
    Js,
    Png,
}

impl Kind {
    /// Whether a content coding is compressible.
    ///
    /// PNG data is already deflate compressed, so encoding it again only costs startup time.
    fn compressible(self) -> bool {
        match self {
            Self::Css | Self::Js => true,
            Self::Png => false,
        }
    }
}

impl Asset {
    /// Construct new hashed asset under the given `name`, `kind` and `content`.
    ///
    /// Assets are immutable and served from memory, so every encoding is computed here once instead
    /// of on each request. Encodings that do not make the payload smaller are dropped, and kinds
    /// which no content coding shrinks are stored unencoded from the start. Small assets such as
    /// `burn.js` would otherwise grow under a content coding. The served route is derived from the
    /// uncompressed `content` and thus shared by all encodings of an asset.
    pub fn new(name: &str, kind: Kind, content: Vec<u8>) -> Self {
        let (mime, ext) = match kind {
            Kind::Css => (mime::TEXT_CSS, "css"),
            Kind::Js => (mime::TEXT_JAVASCRIPT, "js"),
            Kind::Png => (mime::IMAGE_PNG, "png"),
        };

        let route = format!(
            "/{name}.{}.{ext}",
            hex::encode(Sha256::digest(&content))
                .get(0..16)
                .expect("at least 16 characters")
        );

        let identity = Bytes::from(content);
        let (brotli, gzip) = if kind.compressible() {
            (
                compress_brotli(&identity).filter(|c| c.len() < identity.len()),
                compress_gzip(&identity).filter(|c| c.len() < identity.len()),
            )
        } else {
            (None, None)
        };

        Self {
            route,
            mime,
            identity,
            brotli,
            gzip,
        }
    }

    pub fn route(&self) -> &str {
        &self.route
    }

    /// Serve the asset in the content coding preferred by the request `headers`.
    ///
    /// Answers `406 Not Acceptable` if the client refuses every coding the asset is available
    /// in, identity included.
    pub(crate) fn to_response(&self, headers: &HeaderMap) -> Response {
        let Some(encoding) = self.encoding(headers) else {
            return StatusCode::NOT_ACCEPTABLE.into_response();
        };

        let body = match encoding {
            Encoding::Identity => Some(self.identity.clone()),
            Encoding::Gzip => self.gzip.clone(),
            Encoding::Brotli => self.brotli.clone(),
        };

        let body = body.expect("only available codings are negotiated");

        let mut response = (
            TypedHeader(headers::ContentType::from(self.mime.clone())),
            TypedHeader(
                headers::CacheControl::new()
                    .with_max_age(MAX_AGE)
                    .with_immutable(),
            ),
            body,
        )
            .into_response();

        let headers = response.headers_mut();

        // The response varies per client even when served plain; add the header here because
        // the compression layer only does so for responses that it compresses itself.
        headers.insert(
            http::header::VARY,
            HeaderValue::from_static("accept-encoding"),
        );

        if let Some(value) = encoding.header_value() {
            headers.insert(
                http::header::CONTENT_ENCODING,
                HeaderValue::from_static(value),
            );
        }

        response
    }

    /// Content coding to serve for the request `headers`, `None` if the client refuses them all.
    fn encoding(&self, headers: &HeaderMap) -> Option<Encoding> {
        // Several field lines with the same name are equivalent to one comma separated line
        // (RFC 9110, section 5.2), so all of them are read in order. Values which are not ASCII
        // cannot express a preference we can honour and are skipped.
        let mut values = headers
            .get_all(http::header::ACCEPT_ENCODING)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .peekable();

        if values.peek().is_none() {
            // Absent header would permit any coding; identity is the safe answer.
            return Some(Encoding::Identity);
        }

        let accepted = Accepted::parse(values);

        [Encoding::Brotli, Encoding::Gzip, Encoding::Identity]
            .into_iter()
            .filter(|encoding| self.available(*encoding))
            .max_by_key(|encoding| (accepted.quality(*encoding), *encoding))
            .filter(|encoding| accepted.quality(*encoding) > 0)
    }

    /// Whether the asset is available in `encoding`.
    fn available(&self, encoding: Encoding) -> bool {
        match encoding {
            Encoding::Identity => true,
            Encoding::Gzip => self.gzip.is_some(),
            Encoding::Brotli => self.brotli.is_some(),
        }
    }
}

/// Quality values from an `Accept-Encoding` request header, in per mille.
///
/// Only codings this server can produce are tracked; all others either fall under the wildcard
/// or cannot be served anyway.
#[derive(Default)]
struct Accepted {
    /// `br`
    brotli: Option<u16>,
    /// `gzip` and its `x-gzip` alias
    gzip: Option<u16>,
    /// `identity`
    identity: Option<u16>,
    /// `*`
    wildcard: Option<u16>,
}

/// Quality of codings a client does not mention, i.e. `q=1`.
const QUALITY_MAX: u16 = 1000;

impl Accepted {
    /// Parse the `Accept-Encoding` field values of a request, in the order they were sent.
    ///
    /// Elements with an unparsable quality are treated as refused rather than being upgraded to
    /// [`QUALITY_MAX`]; if a coding is mentioned more than once, the last mention wins, across
    /// field lines just as within one.
    fn parse<'a>(values: impl IntoIterator<Item = &'a str>) -> Self {
        let mut accepted = Self::default();

        for value in values {
            accepted.add(value);
        }

        accepted
    }

    /// Add the elements of one field value to this set.
    fn add(&mut self, value: &str) {
        for element in value.split(',') {
            let mut parts = element.split(';');
            let Some(name) = parts.next() else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() {
                continue;
            }

            let mut quality = QUALITY_MAX;
            for parameter in parts {
                let Some((key, value)) = parameter.split_once('=') else {
                    // Parameters other than the quality are of no interest here.
                    continue;
                };
                if key.trim().eq_ignore_ascii_case("q") {
                    quality = parse_quality(value.trim()).unwrap_or(0);
                }
            }

            if name.eq_ignore_ascii_case("br") {
                self.brotli = Some(quality);
            } else if name.eq_ignore_ascii_case("gzip") || name.eq_ignore_ascii_case("x-gzip") {
                self.gzip = Some(quality);
            } else if name.eq_ignore_ascii_case("identity") {
                self.identity = Some(quality);
            } else if name == "*" {
                self.wildcard = Some(quality);
            }
        }
    }

    /// Quality the client assigns to `encoding`.
    ///
    /// Codings that are not mentioned are only acceptable through a wildcard. Identity is the
    /// exception: it stays acceptable unless it is refused through `identity;q=0` or `*;q=0`
    /// (RFC 9110, section 12.5.3).
    fn quality(&self, encoding: Encoding) -> u16 {
        let explicit = match encoding {
            Encoding::Brotli => self.brotli,
            Encoding::Gzip => self.gzip,
            Encoding::Identity => self.identity,
        };

        explicit.unwrap_or_else(|| match (encoding, self.wildcard) {
            (Encoding::Identity, Some(0)) => 0,
            (Encoding::Identity, _) => QUALITY_MAX,
            (_, wildcard) => wildcard.unwrap_or(0),
        })
    }
}

/// Parse a `q` parameter value such as `0.5` into per mille.
fn parse_quality(value: &str) -> Option<u16> {
    let (integer, fraction) = value.split_once('.').unwrap_or((value, ""));

    if fraction.len() > 3 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let base = match integer {
        "0" => 0,
        "1" => QUALITY_MAX,
        _ => return None,
    };

    let mut scale = 100_u16;
    let mut remainder = 0_u16;
    for byte in fraction.bytes() {
        remainder += u16::from(byte - b'0') * scale;
        scale /= 10;
    }

    // `1.5` is as invalid as anything above it.
    if base == QUALITY_MAX && remainder > 0 {
        return None;
    }

    Some(base + remainder)
}

/// Compress `content` with Brotli, `None` if the encoder fails.
fn compress_brotli(content: &[u8]) -> Option<Bytes> {
    let mut encoder =
        brotli::CompressorWriter::new(Vec::new(), BROTLI_BUFFER, BROTLI_QUALITY, BROTLI_WINDOW);
    encoder.write_all(content).ok()?;

    // `into_inner` terminates the stream before handing the buffer back.
    Some(Bytes::from(encoder.into_inner()))
}

/// Compress `content` with gzip, `None` if the encoder fails.
fn compress_gzip(content: &[u8]) -> Option<Bytes> {
    let mut encoder =
        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(GZIP_LEVEL));
    encoder.write_all(content).ok()?;

    // `finish` writes the gzip trailer.
    Some(Bytes::from(encoder.finish().ok()?))
}

/// Collection of light and dark CSS and main UI style CSS derived from them.
pub(crate) struct Css {
    /// Main UI CSS stylesheet.
    pub style: Asset,
    /// Light theme colors.
    pub light: Asset,
    /// Dark theme colors.
    pub dark: Asset,
    /// Overrides applied when JavaScript is disabled.
    pub no_js: Asset,
}

impl Css {
    /// Create CSS assets for `theme`.
    pub fn new(theme: Theme) -> Self {
        let style = Asset::new("style", Kind::Css, include_str!("style.css").into());
        let light = Asset::new("light", Kind::Css, theme.light_css());
        let dark = Asset::new("dark", Kind::Css, theme.dark_css());
        let no_js = Asset::new("no-js", Kind::Css, include_str!("no-js.css").into());

        Self {
            style,
            light,
            dark,
            no_js,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use super::*;

    /// CSS content that both encodings make smaller.
    const CSS: &str = "body { background: #fff; color: #000; }\n";

    /// Asset with both content codings available.
    fn css_asset() -> Asset {
        let asset = Asset::new("style", Kind::Css, CSS.repeat(64).into_bytes());

        assert!(asset.brotli.is_some(), "brotli available");
        assert!(asset.gzip.is_some(), "gzip available");

        asset
    }

    /// Serve `asset`, with one `Accept-Encoding` field line per element of `accept_encoding`.
    fn response_for(asset: &Asset, accept_encoding: &[&str]) -> Response {
        let mut headers = HeaderMap::new();
        for value in accept_encoding {
            headers.append(
                http::header::ACCEPT_ENCODING,
                HeaderValue::from_str(value).expect("valid header value"),
            );
        }

        asset.to_response(&headers)
    }

    /// Decodes a body that was encoded with one content coding.
    type Decoder = fn(&[u8]) -> Vec<u8>;

    /// Decode a Brotli encoded body.
    fn decode_brotli(body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        brotli::Decompressor::new(body, 4096)
            .read_to_end(&mut out)
            .expect("decoding brotli");

        out
    }

    /// Decode a gzip encoded body.
    fn decode_gzip(body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(body)
            .read_to_end(&mut out)
            .expect("decoding gzip");

        out
    }

    #[test]
    fn hashed_asset() {
        let asset = Asset::new("style", Kind::Css, String::from("body {}").into_bytes());
        assert_eq!(asset.route, "/style.62368a1a29259b30.css");

        let asset = Asset::new("main", Kind::Js, String::from("1 + 1").into_bytes());
        assert_eq!(asset.route, "/main.72fce59447a01f48.js");

        let asset = Asset::new("icon", Kind::Png, String::from("1 + 1").into_bytes());
        assert_eq!(asset.route, "/icon.72fce59447a01f48.png");
    }

    #[test]
    fn asset_response() {
        let response = response_for(&css_asset(), &[]);
        let headers = response.headers();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(headers.get(http::header::CONTENT_TYPE).unwrap(), "text/css");
        assert_eq!(
            headers.get(http::header::CACHE_CONTROL).unwrap(),
            "immutable, max-age=2592000"
        );
        assert_eq!(headers.get(http::header::VARY).unwrap(), "accept-encoding");
        assert!(!headers.contains_key(http::header::CONTENT_ENCODING));
    }

    #[test]
    fn negotiated_encoding() {
        let asset = css_asset();
        let cases: &[(&[&str], Option<&str>)] = &[
            // A single coding is served as is.
            (&["br"], Some("br")),
            (&["gzip"], Some("gzip")),
            // Equally accepted codings are broken in favour of the more compact one.
            (&["gzip, deflate, br, zstd"], Some("br")),
            (&["*"], Some("br")),
            // Client preference wins over ours.
            (&["gzip;q=1.0, br;q=0.5"], Some("gzip")),
            (&["br;q=0, gzip"], Some("gzip")),
            // Coding names are case insensitive, and `x-gzip` is an alias for gzip.
            (&["GZIP"], Some("gzip")),
            (&["x-gzip"], Some("gzip")),
            // Refusing identity does not matter as long as another coding is left.
            (&["identity;q=0, gzip"], Some("gzip")),
            (&["identity;q=0, *"], Some("br")),
            // Field lines combine in order, like one comma separated line (RFC 9110, section
            // 5.2): a refusal on one line does not hide a coding granted on another.
            (&["identity;q=0", "br"], Some("br")),
            (&["gzip;q=0", "br"], Some("br")),
            // Unknown codings leave identity, which is acceptable by default.
            (&["zstd, deflate"], None),
            (&["gzip;q=0"], None),
            // A wildcard below one does not lower the implicit quality of identity, which stays
            // acceptable at full quality unless it is refused (RFC 9110, section 12.5.3).
            (&["*;q=0.5, br;q=0.4"], None),
            // Unparsable or absent elements are refused rather than upgraded.
            (&["gzip;q=1.5"], None),
            (&["gzip;q=0.5.5"], None),
            (&["q=0.5"], None),
            (&[""], None),
        ];

        for &(accept_encoding, expected) in cases {
            let response = response_for(&asset, accept_encoding);
            let served = response
                .headers()
                .get(http::header::CONTENT_ENCODING)
                .map(|value| value.to_str().unwrap());

            assert_eq!(response.status(), StatusCode::OK, "{accept_encoding:?}");
            assert_eq!(served, expected, "{accept_encoding:?}");
        }
    }

    #[test]
    fn refused_codings() {
        let asset = css_asset();

        let refused: &[&[&str]] = &[
            &["identity;q=0, *;q=0"],
            &["identity;q=0"],
            &["*;q=0"],
            // A refusal on one field line does not resurrect a coding refused on another.
            &["br;q=0", "identity;q=0"],
        ];

        for accept_encoding in refused {
            let response = response_for(&asset, accept_encoding);

            assert_eq!(
                response.status(),
                StatusCode::NOT_ACCEPTABLE,
                "{accept_encoding:?}"
            );
        }

        // A payload that no encoding shrinks is only available uncompressed.
        let asset = Asset::new("tiny", Kind::Js, Vec::from(b"1 + 1"));
        assert!(asset.brotli.is_none(), "brotli dropped");
        assert!(asset.gzip.is_none(), "gzip dropped");

        let response = response_for(&asset, &["gzip"]);
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            !response
                .headers()
                .contains_key(http::header::CONTENT_ENCODING)
        );
        assert_eq!(
            response_for(&asset, &["identity;q=0"]).status(),
            StatusCode::NOT_ACCEPTABLE
        );

        // A PNG is stored unencoded for the same reason, even when its content would compress.
        let asset = Asset::new("icon", Kind::Png, CSS.repeat(64).into_bytes());
        let response = response_for(&asset, &["br, gzip"]);
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            !response
                .headers()
                .contains_key(http::header::CONTENT_ENCODING)
        );
    }

    #[tokio::test]
    async fn body_matches_content_encoding() {
        let asset = css_asset();
        let cases: &[(&[&str], Decoder)] = &[
            (&["br"], decode_brotli),
            (&["gzip"], decode_gzip),
            // The second field line grants Brotli, so that is what the body must be.
            (&["gzip;q=0", "br"], decode_brotli),
        ];

        for (accept_encoding, decode) in cases {
            let response = response_for(&asset, accept_encoding);
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("reading body");

            assert_eq!(
                decode(&body),
                asset.identity.to_vec(),
                "{accept_encoding:?}"
            );
        }

        let response = response_for(&asset, &[]);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("reading body");

        assert_eq!(body, asset.identity);
    }
}
