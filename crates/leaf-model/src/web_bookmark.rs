//! Web bookmarks (E15, ADR-080, ADR-083): a web address shown as a card, and the preview fetched for it.
//!
//! The block stores the address and, once fetched, the page's title, description, preview image and
//! icon (the images as workspace assets, so cards work offline). Fetching happens elsewhere, opt-in
//! (`leaf-fetch`, E15-S05); the result comes back through
//! [`PageCommand::SetWebBookmarkPreview`](crate::PageCommand::SetWebBookmarkPreview).

use leaf_core::AssetId;
use loro::{LoroMap, LoroValue};

use crate::ModelError;
use crate::block_kind::{keys, write_optional};
use crate::page_meta::value;

/// The longest address a web bookmark takes, in bytes: longer ones are almost always pasted junk.
const MAX_ADDRESS: usize = 4096;

/// What a fetch found for a web bookmark. Every part is optional: a page may have no description, and
/// a bookmark that was never fetched has none of it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WebPreview {
    /// The page's title.
    pub title: Option<String>,
    /// A sentence or two describing the page.
    pub description: Option<String>,
    /// The page's preview picture, stored in the workspace's assets.
    pub image: Option<AssetId>,
    /// The site's icon, stored in the workspace's assets.
    pub favicon: Option<AssetId>,
    /// When the preview was fetched, in Unix milliseconds; `None` when it never was.
    pub fetched_at: Option<i64>,
}

impl WebPreview {
    /// Whether nothing was fetched: the card shows the address and domain only.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Checks `url` is a web address a bookmark may hold: `http://` or `https://`, a host, no spaces or
/// control characters, no user name or password (ADR-080), at most 4 KiB. The address is kept as
/// written; nothing is added or changed.
pub fn check_web_address(url: &str) -> Result<(), ModelError> {
    let refuse = |why: &str| Err(ModelError::InvalidWebAddress(format!("{url:?} {why}")));
    if url.len() > MAX_ADDRESS {
        return refuse("is too long");
    }
    let lower = url.to_ascii_lowercase();
    let Some(rest) = ["http://", "https://"]
        .iter()
        .find_map(|scheme| lower.starts_with(scheme).then(|| &url[scheme.len()..]))
    else {
        return refuse("is not an http or https address");
    };
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return refuse("has spaces or control characters");
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return refuse("has a user name or password");
    }
    let host = authority
        .rsplit_once(':')
        .filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit()))
        .map_or(authority, |(host, _)| host);
    if host.is_empty() || host.starts_with('.') {
        return refuse("has no host");
    }
    Ok(())
}

/// Reads a web bookmark's props. The address is required; a preview part that is missing or not
/// readable (from a newer version, say) reads as not there, and stays stored until a new preview
/// replaces it.
pub(crate) fn read(meta: &LoroMap) -> Result<(String, WebPreview), (&'static str, String)> {
    let url = match value(meta, keys::URL) {
        Some(LoroValue::String(url)) => url.to_string(),
        Some(_) => return Err((keys::URL, "is not text".into())),
        None => return Err((keys::URL, "is missing".into())),
    };
    let text = |key| match value(meta, key) {
        Some(LoroValue::String(text)) => Some(text.to_string()),
        _ => None,
    };
    let asset = |key| text(key).and_then(|hex| hex.parse().ok());
    let fetched_at = match value(meta, keys::FETCHED_AT) {
        Some(LoroValue::I64(at)) => Some(at),
        _ => None,
    };
    let preview = WebPreview {
        title: text(keys::TITLE),
        description: text(keys::DESCRIPTION),
        image: asset(keys::IMAGE),
        favicon: asset(keys::FAVICON),
        fetched_at,
    };
    Ok((url, preview))
}

/// Writes the address and preview into a block's meta; the parts the preview doesn't have are removed.
pub(crate) fn write(meta: &LoroMap, url: &str, preview: &WebPreview) -> loro::LoroResult<()> {
    meta.insert(keys::URL, url)?;
    write_preview(meta, preview)
}

/// Writes just the preview.
pub(crate) fn write_preview(meta: &LoroMap, preview: &WebPreview) -> loro::LoroResult<()> {
    write_optional(meta, keys::TITLE, preview.title.clone())?;
    write_optional(meta, keys::DESCRIPTION, preview.description.clone())?;
    write_optional(meta, keys::IMAGE, preview.image.map(|id| id.to_string()))?;
    write_optional(meta, keys::FAVICON, preview.favicon.map(|id| id.to_string()))?;
    match preview.fetched_at {
        Some(at) => meta.insert(keys::FETCHED_AT, at),
        None if meta.get(keys::FETCHED_AT).is_some() => meta.delete(keys::FETCHED_AT),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::check_web_address;

    #[test]
    fn web_addresses() {
        for ok in [
            "https://leaf.app",
            "http://example.com/a?b=c#d",
            "HTTPS://Example.COM",
            "https://example.com:8443/x",
            "https://[2606:4700::1111]/",
            "https://例え.jp/パス",
        ] {
            assert!(check_web_address(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "leaf.app",
            "ftp://leaf.app",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https://",
            "https:///path",
            "https://leaf .app",
            " https://leaf.app",
            "https://leaf.app\n",
            "https://user:pass@leaf.app/",
            "https://user@leaf.app",
            "https://:443/",
        ] {
            assert!(check_web_address(bad).is_err(), "{bad:?}");
        }
        assert!(check_web_address(&format!("https://leaf.app/{}", "a".repeat(5000))).is_err());
    }
}
