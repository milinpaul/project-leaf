//! Fetching web bookmark previews (E15-S05, ADR-080): the only Leaf crate that goes online.
//!
//! Given a bookmark's address, [`Fetcher::preview`] reads the page's title, description, preview
//! picture and site icon, and nothing else. Every limit from ADR-080 is built in:
//!
//! - `http` and `https` only, for the page, every redirect and both pictures; no user name or password;
//! - public internet only, checked inside the DNS lookup the connection uses ([`guard`]), so neither a
//!   redirect nor a second DNS answer can reach this computer or the home or office network;
//! - 5 redirects, 5 s to connect, 10 s for each request; 512 KiB of HTML (only `<head>` is read), 5 MiB
//!   for the picture, 1 MiB for the icon; the page must say it is HTML and the pictures that they are
//!   images Leaf can draw;
//! - no JavaScript, no cookies, no `Referer`, no other files, and a direct connection (the system proxy
//!   is ignored, so the address check always holds).
//!
//! **Boundaries:** native only, never compiled for wasm. Blocking: callers run it off the UI thread.
//! It never touches documents or files: it hands back what it found, and the app stores the pictures
//! and saves the preview through a command. It is called only when the owner has allowed previews.

pub mod guard;
mod head;
mod limits;
mod text;

pub use limits::{FetchError, Fetcher, HTML_LIMIT, ICON_LIMIT, IMAGE_LIMIT, MAX_REDIRECTS, Picture, Preview};
