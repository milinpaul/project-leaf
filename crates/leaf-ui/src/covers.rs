//! The built-in cover images: where each one lives among the app's assets, and their bytes.
//!
//! Covers are bundled image files drawn with GPUI's `img()`: SVG for the hand-built ones, WebP for ones made with
//! an image model (Travel), which `img()` decodes by the file extension (ADR 0045).
//! Which covers exist, and what a page's stored cover means, is `leaf_editor::BuiltinCover`'s business.

use std::borrow::Cow;

use leaf_editor::BuiltinCover;

/// Where the cover's image is in the app's assets, for `img()`.
#[must_use]
pub fn asset_path(cover: BuiltinCover) -> &'static str {
    match cover {
        BuiltinCover::Dawn => "covers/dawn.svg",
        BuiltinCover::Forest => "covers/forest.svg",
        BuiltinCover::Ocean => "covers/ocean.svg",
        BuiltinCover::Paper => "covers/paper.svg",
        BuiltinCover::Night => "covers/night.svg",
        BuiltinCover::Travel => "covers/travel.webp",
    }
}

/// The bytes of a cover image at `path`, if `path` is one.
pub(crate) fn load(path: &str) -> Option<Cow<'static, [u8]>> {
    let bytes: &'static [u8] = match path {
        "covers/dawn.svg" => include_bytes!("../assets/covers/dawn.svg"),
        "covers/forest.svg" => include_bytes!("../assets/covers/forest.svg"),
        "covers/ocean.svg" => include_bytes!("../assets/covers/ocean.svg"),
        "covers/paper.svg" => include_bytes!("../assets/covers/paper.svg"),
        "covers/night.svg" => include_bytes!("../assets/covers/night.svg"),
        "covers/travel.webp" => include_bytes!("../assets/covers/travel.webp"),
        _ => return None,
    };
    Some(Cow::Borrowed(bytes))
}

#[cfg(test)]
mod tests {
    use leaf_editor::BuiltinCover;

    #[test]
    fn every_built_in_cover_has_an_image_img_can_draw() {
        for &cover in BuiltinCover::ALL {
            let path = super::asset_path(cover);
            let bytes = super::load(path).unwrap_or_else(|| panic!("{path} is not bundled"));
            let svg = path == format!("covers/{}.svg", cover.name()) && bytes.starts_with(b"<svg");
            // A WebP file is a RIFF container whose form type is "WEBP".
            let webp = path == format!("covers/{}.webp", cover.name())
                && bytes.starts_with(b"RIFF")
                && bytes.get(8..12) == Some(b"WEBP".as_slice());
            assert!(svg || webp, "{path} is an SVG or WebP file named after the cover");
        }
        assert!(super::load("covers/none.svg").is_none());
    }

    #[test]
    fn every_cover_stays_small() {
        for &cover in BuiltinCover::ALL {
            let path = super::asset_path(cover);
            let size = super::load(path).map_or(0, |bytes| bytes.len());
            assert!(
                size < 100 * 1024,
                "{path} is {size} bytes; keep each cover under 100 KB"
            );
        }
    }
}
