//! E15-S01 trial (throwaway): can Leaf fetch a web page's preview within the safety limits, with a small,
//! `cargo deny`-clean dependency set? Three parts: [`guard`] refuses local-network addresses at DNS time,
//! [`fetch`] sets the limits, [`head`] reads only the page's `<head>` tags.

#![allow(missing_docs, clippy::cast_possible_truncation)]

pub mod fetch;
pub mod guard;
pub mod head;
