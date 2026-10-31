//! Turning a page's bytes into text: by the charset its `Content-Type` header names, else by its
//! `<meta charset>` (or `http-equiv` content type) near the top, else UTF-8. Undecodable bytes become
//! U+FFFD rather than failing the preview.

use encoding_rs::{Encoding, UTF_8};

/// How far into the page a `<meta charset>` is looked for; HTML says it must be in the first 1,024 bytes.
const META_WINDOW: usize = 1024;

/// The page's text. `content_type` is the response's `Content-Type` header, if it had one.
pub(crate) fn decode(bytes: &[u8], content_type: Option<&str>) -> String {
    let encoding = content_type
        .and_then(charset_in)
        .or_else(|| meta_charset(bytes))
        .and_then(|label| Encoding::for_label(label.trim().as_bytes()))
        .unwrap_or(UTF_8);
    let (text, _, _) = encoding.decode(bytes);
    text.into_owned()
}

/// The `charset=` value in a header or `content` attribute, without quotes.
fn charset_in(value: &str) -> Option<&str> {
    let lower = value.to_ascii_lowercase();
    let at = lower.find("charset=")? + "charset=".len();
    let rest = &value[at..];
    let rest = rest.trim_start_matches(['"', '\'']);
    let end = rest.find(['"', '\'', ';', ' ', '>', '/']).unwrap_or(rest.len());
    Some(&rest[..end]).filter(|label| !label.is_empty())
}

/// The charset a `<meta>` tag near the top of the page names: `<meta charset="…">` or
/// `<meta http-equiv="Content-Type" content="text/html; charset=…">`.
fn meta_charset(bytes: &[u8]) -> Option<&str> {
    let head = &bytes[..bytes.len().min(META_WINDOW)];
    // Charset labels are ASCII, so reading the window as ASCII (lossily) is enough to find one.
    let ascii = std::str::from_utf8(head).ok().or_else(|| {
        head.iter()
            .position(|b| !b.is_ascii())
            .and_then(|end| std::str::from_utf8(&head[..end]).ok())
    })?;
    let lower = ascii.to_ascii_lowercase();
    let meta = lower.find("<meta")?;
    let at = meta + lower[meta..].find("charset=")?;
    charset_in(&ascii[at..])
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn the_header_then_the_meta_tag_then_utf8() {
        let latin1 = b"<html><head><title>D\xe9j\xe0</title></head></html>";
        assert_eq!(
            decode(latin1, Some("text/html; charset=ISO-8859-1")),
            "<html><head><title>Déjà</title></head></html>"
        );
        let meta = b"<html><head><meta charset=\"windows-1252\"><title>D\xe9j\xe0</title>";
        assert!(decode(meta, Some("text/html")).contains("Déjà"));
        let http_equiv =
            b"<meta http-equiv=\"Content-Type\" content=\"text/html; charset=shift_jis\"><title>\x93\xfa\x96\x7b</title>";
        assert!(decode(http_equiv, None).contains("日本"));
        assert_eq!(
            decode("日本".as_bytes(), None),
            "日本",
            "UTF-8 when nothing says otherwise"
        );
        assert_eq!(
            decode(b"\xff", Some("text/html; charset=nonsense")),
            "\u{fffd}",
            "an unknown label is UTF-8"
        );
    }
}
