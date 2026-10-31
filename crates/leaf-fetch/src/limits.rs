//! The fetch itself, with every limit from ADR-080 in one place.

use std::io::Read as _;
use std::net::SocketAddr;
use std::time::Duration;

use ureq::unversioned::transport::DefaultConnector;
use ureq::{Agent, ResponseExt as _};

use crate::guard::PublicOnly;
use crate::head::read_head;
use crate::text::decode;

/// The most HTML read: the preview is in `<head>`, almost always in the first few KiB.
pub const HTML_LIMIT: u64 = 512 * 1024;
/// The largest preview picture kept.
pub const IMAGE_LIMIT: u64 = 5 * 1024 * 1024;
/// The largest site icon kept.
pub const ICON_LIMIT: u64 = 1024 * 1024;
/// Redirects followed; each hop's address goes through [`PublicOnly`] again.
pub const MAX_REDIRECTS: u32 = 5;
/// Time to connect, and for each whole request (redirects and body included).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(10);
/// The response headers' total size.
const HEADER_LIMIT: usize = 64 * 1024;

/// Picture types Leaf can draw. Anything else (an `.ico` favicon, say) is skipped.
const DRAWABLE: [&str; 5] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/svg+xml",
];

/// Why a preview could not be fetched. The bookmark then keeps showing its address.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// Not an address Leaf fetches: not `http`/`https`, or with a user name or password.
    #[error("not fetched: {0}")]
    Address(String),
    /// The address answered with something other than a web page.
    #[error("not a web page ({0})")]
    NotHtml(String),
    /// A picture's address answered with something other than a picture Leaf can draw.
    #[error("not a picture Leaf can show ({0})")]
    NotImage(String),
    /// The request failed: no answer in time, a local-network address, too many redirects, too big.
    #[error(transparent)]
    Http(#[from] ureq::Error),
    /// Reading the answer failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A picture as it came: its bytes and type, for the asset store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// Its media type, one Leaf can draw.
    pub kind: String,
}

/// What a page offered for its bookmark. Every part may be missing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preview {
    /// The page's title (Open Graph, then Twitter, then `<title>`).
    pub title: Option<String>,
    /// Its description (Open Graph, then Twitter, then `<meta name="description">`).
    pub description: Option<String>,
    /// The site's name (`og:site_name`).
    pub site: Option<String>,
    /// The preview picture; a failed or refused picture is just left out.
    pub image: Option<Picture>,
    /// The site's icon; likewise.
    pub favicon: Option<Picture>,
}

/// Fetches previews. One per app is enough; it holds no state between fetches (no cookies, no cache).
#[derive(Debug)]
pub struct Fetcher {
    agent: Agent,
}

impl Default for Fetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetcher {
    /// A fetcher for the public internet.
    #[must_use]
    pub fn new() -> Self {
        Self::with_guard(PublicOnly::default())
    }

    /// A fetcher that may also reach `server` on this computer: for tests with a local server only.
    #[must_use]
    pub fn allowing(server: SocketAddr) -> Self {
        Self::with_guard(PublicOnly::allowing(server))
    }

    fn with_guard(guard: PublicOnly) -> Self {
        let config = Agent::config_builder()
            .https_only(false)
            .max_redirects(MAX_REDIRECTS)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(TOTAL_TIMEOUT))
            .max_response_header_size(HEADER_LIMIT)
            // A direct connection only: through a proxy, the proxy looks names up, not the guard.
            .proxy(None)
            .user_agent(format!(
                "Leaf/{} (web bookmark preview)",
                env!("CARGO_PKG_VERSION")
            ))
            .build();
        Self {
            agent: Agent::with_parts(config, DefaultConnector::default(), guard),
        }
    }

    /// Fetches the page at `address` and reads its preview, then its preview picture and site icon.
    /// Blocking: up to three requests of at most 10 s each. A picture that fails is left out; only a
    /// page that can't be read is an error.
    pub fn preview(&self, address: &str) -> Result<Preview, FetchError> {
        let found = self.page(address)?;
        let picture = |url: Option<&String>, limit| url.and_then(|url| self.picture(url, limit).ok());
        Ok(Preview {
            title: found.title.clone(),
            description: found.description.clone(),
            site: found.site.clone(),
            image: picture(found.image.as_ref(), IMAGE_LIMIT),
            favicon: picture(found.icon.as_ref(), ICON_LIMIT),
        })
    }

    /// The page's `<head>` preview; relative addresses resolve against its final address after redirects.
    fn page(&self, address: &str) -> Result<crate::head::Found, FetchError> {
        checked(address)?;
        let mut response = self.agent.get(address).header("Accept", "text/html").call()?;
        let kind = response.body().mime_type().unwrap_or_default().to_owned();
        if kind != "text/html" && kind != "application/xhtml+xml" {
            return Err(FetchError::NotHtml(kind));
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let base = url::Url::parse(&response.get_uri().to_string())
            .map_err(|err| FetchError::Address(err.to_string()))?;
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(HTML_LIMIT)
            .read_to_end(&mut bytes)?;
        let html = decode(&bytes, content_type.as_deref());
        Ok(read_head(&html, &base))
    }

    /// A picture, if it is one Leaf can draw and fits `limit` bytes.
    fn picture(&self, address: &str, limit: u64) -> Result<Picture, FetchError> {
        checked(address)?;
        let mut response = self.agent.get(address).header("Accept", "image/*").call()?;
        let kind = response
            .body()
            .mime_type()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !DRAWABLE.contains(&kind.as_str()) {
            return Err(FetchError::NotImage(kind));
        }
        let bytes = response.body_mut().with_config().limit(limit).read_to_vec()?;
        Ok(Picture { bytes, kind })
    }
}

/// Refuses an address before any request: the same rule bookmarks keep (`http`/`https`, a host, no
/// user name or password).
fn checked(address: &str) -> Result<(), FetchError> {
    leaf_model::check_web_address(address).map_err(|err| FetchError::Address(err.to_string()))
}
