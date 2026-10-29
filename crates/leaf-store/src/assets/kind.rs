//! Telling an image's type from its first bytes ("magic numbers"), without a dependency: the handful
//! of formats a notes app is pasted or dragged, each with a fixed signature.

/// The media type of `bytes` if they are an image Leaf stores: PNG, JPEG, GIF, WebP, BMP, TIFF,
/// AVIF, HEIC or SVG. `None` for anything else.
#[must_use]
pub fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    let starts = |signature: &[u8]| bytes.starts_with(signature);
    if starts(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if starts(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if starts(b"GIF87a") || starts(b"GIF89a") {
        Some("image/gif")
    } else if starts(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else if starts(b"BM") && bytes.len() >= 26 {
        Some("image/bmp")
    } else if starts(b"II*\0") || starts(b"MM\0*") {
        Some("image/tiff")
    } else if bytes.get(4..8) == Some(b"ftyp") {
        iso_image(bytes.get(8..12)?)
    } else if is_svg(bytes) {
        Some("image/svg+xml")
    } else {
        None
    }
}

/// AVIF and HEIC share the ISO media file layout; the brand after `ftyp` says which.
fn iso_image(brand: &[u8]) -> Option<&'static str> {
    match brand {
        b"avif" | b"avis" => Some("image/avif"),
        b"heic" | b"heix" | b"heim" | b"heis" | b"mif1" => Some("image/heic"),
        _ => None,
    }
}

/// SVG is text: an `<svg` element near the start, after an optional byte-order mark, XML
/// declaration, comments or doctype.
fn is_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let Ok(text) = std::str::from_utf8(head).or_else(|err| std::str::from_utf8(&head[..err.valid_up_to()]))
    else {
        return false;
    };
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with('<') && text.contains("<svg")
}

#[cfg(test)]
mod tests;
