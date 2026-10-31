//! What a web bookmark's card shows (E15-S04): the page's title, two lines of description, the site's
//! icon and domain, and the preview picture at the side. Parts the page didn't have are left out, so
//! the card collapses cleanly; before any preview (or with fetching off) it shows the address and the
//! domain. The UI only lays this out and opens [`WebBookmarkCard::address`] on a click.

use leaf_core::AssetId;
use leaf_model::WebPreview;

/// The most characters of a title or description the card keeps; the UI also truncates to its width.
const MAX_TITLE: usize = 300;
const MAX_DESCRIPTION: usize = 500;

/// A web bookmark's card, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebBookmarkCard {
    /// The top line: the page's title, or the address when there is none.
    pub title: String,
    /// Up to two lines under it; `None` collapses them.
    pub description: Option<String>,
    /// The site, `leaf.app` for `https://www.leaf.app/blog`, beside its icon on the bottom line.
    pub domain: String,
    /// What a click opens.
    pub address: String,
    /// The preview picture at the side; `None` collapses it.
    pub image: Option<AssetId>,
    /// The site's icon before the domain; a globe-like placeholder when `None`.
    pub favicon: Option<AssetId>,
    /// Whether a preview was ever fetched: without one the card says only where the link goes.
    pub fetched: bool,
    /// Whether a fetch is under way, for a subtle loading state.
    pub fetching: bool,
}

impl WebBookmarkCard {
    /// The card for a bookmark of `url` with `preview`; `fetching` while a fetch is under way.
    #[must_use]
    pub fn new(url: &str, preview: &WebPreview, fetching: bool) -> Self {
        let tidy = |text: &Option<String>, max: usize| {
            text.as_deref()
                .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|t| !t.is_empty())
                .map(|t| t.chars().take(max).collect::<String>())
        };
        Self {
            title: tidy(&preview.title, MAX_TITLE).unwrap_or_else(|| url.to_owned()),
            description: tidy(&preview.description, MAX_DESCRIPTION),
            domain: domain_of(url),
            address: url.to_owned(),
            image: preview.image,
            favicon: preview.favicon,
            fetched: preview.fetched_at.is_some() || !preview.is_empty(),
            fetching,
        }
    }
}

/// The site an address points at, as people say it: lower case, without `www.`, a port or a user name.
/// `https://www.Leaf.app:8443/blog` gives `leaf.app`.
#[must_use]
pub fn domain_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = match host.strip_prefix('[') {
        // An IPv6 address keeps its brackets and drops only the port after them.
        Some(_) => host.split_inclusive(']').next().unwrap_or(host),
        None => host.split(':').next().unwrap_or(host),
    };
    let host = host.to_lowercase();
    host.strip_prefix("www.").map(str::to_owned).unwrap_or(host)
}

#[cfg(test)]
mod tests;
