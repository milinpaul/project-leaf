//! Checking an SVG before it is drawn (ADR-078).
//!
//! GPUI draws SVGs with `usvg` on its default settings, which load any image an SVG names by path
//! (`<image href="/Users/me/photo.jpg">`), including other SVGs, and draw the picture at whatever size
//! it claims. So a stored SVG, imported or arriving in a workspace someone else made, could show files
//! from elsewhere on the computer, or ask for gigabytes of pixels. Leaf can't change those settings, so
//! it reads the SVG first and draws only one that names nothing outside itself and fits the limits
//! below. The file itself is never changed.

use roxmltree::{Document, Node, ParsingOptions};

/// The largest SVG file drawn. Icons and illustrations are far smaller.
pub(crate) const MAX_BYTES: usize = 10 * 1024 * 1024;
/// The most XML nodes (elements, text, comments) in one SVG. Detailed maps reach tens of thousands.
pub(crate) const MAX_NODES: u32 = 250_000;
/// The deepest nesting of elements. `usvg` walks the tree recursively, so this keeps its stack small.
pub(crate) const MAX_DEPTH: usize = 256;
/// The widest or tallest picture drawn, in CSS pixels. GPUI draws SVGs at twice their size.
const MAX_SIDE: f64 = 8192.0;
/// The most pixels in a picture (width × height, CSS pixels): about 128 MB once GPUI doubles it.
const MAX_AREA: f64 = 8_000_000.0;
/// `usvg`'s size for an SVG that gives neither a size nor a `viewBox`.
const DEFAULT_SIDE: f64 = 100.0;

/// Why a stored SVG isn't drawn. The messages are shown in the image's frame, for the owner.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SvgProblem {
    /// It names a file, a web address, or another SVG (the text in the error) that the drawing would
    /// load. Only references inside the SVG (`#id`) and embedded pictures (`data:image/png...`) are
    /// drawn.
    #[error("This SVG points to a file or web address outside it (“{0}”), so it isn't shown.")]
    OutsideReference(String),
    /// The file is bigger than [`MAX_BYTES`].
    #[error("This SVG is too large to show (over 10 MB).")]
    TooLarge,
    /// It has more than [`MAX_NODES`] parts, or nests them deeper than [`MAX_DEPTH`].
    #[error("This SVG is too complex to show.")]
    TooComplex,
    /// The picture it describes is bigger than Leaf draws (sizes in CSS pixels, rounded).
    #[error("This SVG is too big to draw ({width} × {height}).")]
    TooBig {
        /// Its width.
        width: u64,
        /// Its height.
        height: u64,
    },
    /// It isn't well-formed XML text, or its entities expand without end (a "billion laughs" file).
    #[error("This SVG can't be read.")]
    Unreadable,
}

/// Whether the SVG `bytes` are safe to hand to GPUI: nothing outside the file is named, and its size
/// and complexity are within the limits above.
pub fn check_svg(bytes: &[u8]) -> Result<(), SvgProblem> {
    if bytes.len() > MAX_BYTES {
        return Err(SvgProblem::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| SvgProblem::Unreadable)?;
    // The same options as usvg, so a file reads here exactly as it would be drawn. roxmltree stops
    // runaway entity expansion with an error.
    let options = ParsingOptions {
        allow_dtd: true,
        nodes_limit: MAX_NODES,
        // No resolver: entities that name outside files are never loaded.
        ..ParsingOptions::default()
    };
    let document = Document::parse_with_options(text, options).map_err(|err| match err {
        roxmltree::Error::NodesLimitReached => SvgProblem::TooComplex,
        _ => SvgProblem::Unreadable,
    })?;
    check_depth(document.root())?;
    for node in document.descendants() {
        if node.is_element() {
            check_element(node)?;
        }
    }
    check_size(document.root_element())
}

/// Walks the tree without recursion, failing past [`MAX_DEPTH`].
fn check_depth(root: Node<'_, '_>) -> Result<(), SvgProblem> {
    let mut stack = vec![(root, 0_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(SvgProblem::TooComplex);
        }
        stack.extend(
            node.children()
                .filter(Node::is_element)
                .map(|child| (child, depth + 1)),
        );
    }
    Ok(())
}

/// An element's links (`href` in any namespace), inline `style`, and `<style>` sheet.
fn check_element(node: Node<'_, '_>) -> Result<(), SvgProblem> {
    for attribute in node.attributes() {
        match attribute.name() {
            "href" => check_reference(attribute.value())?,
            "style" => check_css(attribute.value())?,
            // Presentation attributes such as fill="url(#g)" or a mask take the same url() form.
            _ if attribute.value().contains("url(") => check_css(attribute.value())?,
            _ => {}
        }
    }
    if node.tag_name().name() == "style" {
        let sheet: String = node.children().filter_map(|child| child.text()).collect();
        check_css(&sheet)?;
    }
    Ok(())
}

/// `url(...)` targets and `@import` in CSS text.
fn check_css(css: &str) -> Result<(), SvgProblem> {
    if css.contains("@import") {
        return Err(SvgProblem::OutsideReference("@import".to_owned()));
    }
    let mut rest = css;
    while let Some(start) = rest.find("url(") {
        let after = &rest[start + 4..];
        let end = after.find(')').unwrap_or(after.len());
        let target = after[..end].trim().trim_matches(['"', '\'']).trim();
        check_reference(target)?;
        rest = &after[end..];
    }
    Ok(())
}

/// A link may point inside the SVG (`#id`) or embed a raster picture; nothing else.
fn check_reference(target: &str) -> Result<(), SvgProblem> {
    let target = target.trim();
    // An empty link points nowhere; usvg skips it.
    if target.is_empty() || target.starts_with('#') || is_embedded_raster(target) {
        Ok(())
    } else {
        Err(SvgProblem::OutsideReference(target.to_owned()))
    }
}

/// A `data:` URL holding a PNG, JPEG, GIF or WebP. An embedded SVG is refused: usvg would draw it with
/// the same settings, links and all, and checking SVGs inside SVGs isn't worth the code.
fn is_embedded_raster(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    [
        "data:image/png",
        "data:image/jpeg",
        "data:image/jpg",
        "data:image/gif",
        "data:image/webp",
    ]
    .iter()
    .any(|kind| {
        lower
            .strip_prefix(kind)
            .is_some_and(|rest| rest.starts_with(';') || rest.starts_with(','))
    })
}

/// The picture's size as usvg works it out: `width`/`height` when given in absolute units, else the
/// `viewBox` size, else 100 × 100.
fn check_size(svg: Node<'_, '_>) -> Result<(), SvgProblem> {
    let view_box = svg.attribute("viewBox").and_then(view_box_size);
    let width = svg
        .attribute("width")
        .and_then(css_pixels)
        .or(view_box.map(|(width, _)| width))
        .unwrap_or(DEFAULT_SIDE);
    let height = svg
        .attribute("height")
        .and_then(css_pixels)
        .or(view_box.map(|(_, height)| height))
        .unwrap_or(DEFAULT_SIDE);
    if width > MAX_SIDE || height > MAX_SIDE || width * height > MAX_AREA {
        return Err(SvgProblem::TooBig {
            width: rounded(width),
            height: rounded(height),
        });
    }
    Ok(())
}

/// A size rounded for the message. Float-to-integer casts saturate, so a huge size reads as `u64::MAX`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "casts saturate, and sizes are positive"
)]
fn rounded(size: f64) -> u64 {
    size.round() as u64
}

/// The width and height of a `viewBox="x y width height"`.
fn view_box_size(value: &str) -> Option<(f64, f64)> {
    let numbers: Vec<f64> = value
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|part| !part.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    match numbers.as_slice() {
        [_, _, width, height] => Some((width.abs(), height.abs())),
        _ => None,
    }
}

/// A length in CSS pixels, for absolute units. `%`, `em`, `auto` and the like give `None`, so the
/// `viewBox` decides, as in usvg.
fn css_pixels(value: &str) -> Option<f64> {
    let value = value.trim();
    let split = value
        .find(|c: char| (c.is_ascii_alphabetic() && c != 'e' && c != 'E') || c == '%')
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let number: f64 = number.trim().parse().ok()?;
    let per_unit = match unit.trim() {
        "" | "px" => 1.0,
        "pt" => 4.0 / 3.0,
        "pc" => 16.0,
        "mm" => 96.0 / 25.4,
        "cm" => 96.0 / 2.54,
        "in" => 96.0,
        _ => return None,
    };
    Some((number * per_unit).abs())
}

#[cfg(test)]
mod tests;
