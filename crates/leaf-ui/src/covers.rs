//! The built-in cover images: where each one lives among the app's assets, and their bytes.
//!
//! Covers are bundled SVG files drawn with GPUI's `img()`, which renders SVG in full colour (ADR 0045).
//! Which covers exist, and what a page's stored cover means, is `leaf_editor::BuiltinCover`'s business.

use std::borrow::Cow;

use leaf_editor::BuiltinCover;

/// Where the cover's image is in the app's assets, for `img()`.
#[must_use]
pub fn asset_path(cover: BuiltinCover) -> &'static str {
    match cover {
        BuiltinCover::Dawn => "covers/dawn.svg",
    }
}

/// The bytes of a cover image at `path`, if `path` is one.
pub(crate) fn load(path: &str) -> Option<Cow<'static, [u8]>> {
    let bytes: &'static [u8] = match path {
        "covers/dawn.svg" => include_bytes!("../assets/covers/dawn.svg"),
        _ => return None,
    };
    Some(Cow::Borrowed(bytes))
}

#[cfg(test)]
mod tests {
    use leaf_editor::BuiltinCover;

    #[test]
    fn every_built_in_cover_has_an_svg_image() {
        for &cover in BuiltinCover::ALL {
            let path = super::asset_path(cover);
            assert_eq!(path, format!("covers/{}.svg", cover.name()));
            let bytes = super::load(path).unwrap_or_else(|| panic!("{path} is not bundled"));
            assert!(bytes.starts_with(b"<svg"), "{path} is an SVG file");
        }
        assert!(super::load("covers/none.svg").is_none());
    }
}
