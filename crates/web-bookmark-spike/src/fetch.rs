//! The limits from the ADR in one place, and a fetch that keeps to them.

use std::io::Read as _;
use std::time::Duration;

use ureq::unversioned::transport::DefaultConnector;
use ureq::{Agent, ResponseExt as _};

use crate::guard::PublicOnly;
use crate::head::{Preview, read_head};

/// The most HTML read: previews are in `<head>`, which is almost always in the first few KiB.
pub const HTML_LIMIT: u64 = 512 * 1024;
/// The largest preview image or icon saved.
pub const IMAGE_LIMIT: u64 = 5 * 1024 * 1024;
/// The whole fetch, redirects and body included.
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(10);
/// Redirects followed (each hop goes through [`PublicOnly`] again).
pub const MAX_REDIRECTS: u32 = 5;

/// Why a preview could not be fetched; the bookmark then shows its address.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("only http and https addresses are fetched")]
    Scheme,
    #[error("not a web page ({0})")]
    NotHtml(String),
    #[error("not an image ({0})")]
    NotImage(String),
    #[error(transparent)]
    Http(#[from] ureq::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
}

/// An agent with every limit set. `proxy` is off here so tests reach the local server; the ADR decides
/// what the app does with a system proxy.
#[must_use]
pub fn agent(guard: PublicOnly) -> Agent {
    let config = Agent::config_builder()
        .https_only(false)
        .max_redirects(MAX_REDIRECTS)
        .timeout_global(Some(TOTAL_TIMEOUT))
        .timeout_connect(Some(Duration::from_secs(5)))
        .max_response_header_size(64 * 1024)
        .proxy(None)
        .user_agent("Leaf (link preview)")
        .build();
    Agent::with_parts(config, DefaultConnector::default(), guard)
}

fn checked(address: &str) -> Result<url::Url, FetchError> {
    let url = url::Url::parse(address)?;
    if matches!(url.scheme(), "http" | "https") {
        Ok(url)
    } else {
        Err(FetchError::Scheme)
    }
}

/// Fetches a page and reads its preview. Never runs scripts or loads anything else.
pub fn preview(agent: &Agent, address: &str) -> Result<Preview, FetchError> {
    let url = checked(address)?;
    let mut response = agent.get(url.as_str()).header("Accept", "text/html").call()?;
    let kind = response.body().mime_type().unwrap_or_default().to_owned();
    if kind != "text/html" && kind != "application/xhtml+xml" {
        return Err(FetchError::NotHtml(kind));
    }
    // The final address after redirects, for relative image and icon addresses.
    let base = response.get_uri().to_string();
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(HTML_LIMIT)
        .read_to_end(&mut bytes)?;
    // Charset handling (encoding_rs, from the header or <meta charset>) is left to E15-S05.
    let html = String::from_utf8_lossy(&bytes);
    Ok(read_head(&html, &url::Url::parse(&base)?))
}

/// Fetches an image: refused unless it says it is an image and fits [`IMAGE_LIMIT`].
pub fn image(agent: &Agent, address: &str) -> Result<Vec<u8>, FetchError> {
    let url = checked(address)?;
    let mut response = agent.get(url.as_str()).call()?;
    let kind = response.body().mime_type().unwrap_or_default().to_owned();
    if !kind.starts_with("image/") {
        return Err(FetchError::NotImage(kind));
    }
    Ok(response
        .body_mut()
        .with_config()
        .limit(IMAGE_LIMIT)
        .read_to_vec()?)
}
