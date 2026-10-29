//! Tests for telling an image's type from its bytes.

use super::image_mime;

#[test]
fn common_image_formats_are_recognised_by_their_first_bytes() {
    let mut webp = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
    webp.extend_from_slice(&[0; 8]);
    let mut bmp = b"BM".to_vec();
    bmp.extend_from_slice(&[0; 30]);
    let cases: &[(&[u8], &str)] = &[
        (b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR", "image/png"),
        (&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10], "image/jpeg"),
        (b"GIF89a\x01\0\x01\0", "image/gif"),
        (b"GIF87a\x01\0\x01\0", "image/gif"),
        (&webp, "image/webp"),
        (&bmp, "image/bmp"),
        (b"II*\0\x08\0\0\0", "image/tiff"),
        (b"MM\0*\0\0\0\x08", "image/tiff"),
        (b"\0\0\0\x1cftypavif\0\0\0\0", "image/avif"),
        (b"\0\0\0\x18ftypheic\0\0\0\0", "image/heic"),
        (b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>", "image/svg+xml"),
        (
            b"\xEF\xBB\xBF<?xml version=\"1.0\"?>\n<!-- logo -->\n<svg viewBox=\"0 0 1 1\"></svg>",
            "image/svg+xml",
        ),
    ];
    for (bytes, mime) in cases {
        assert_eq!(image_mime(bytes), Some(*mime), "{bytes:?}");
    }
}

#[test]
fn anything_else_is_not_an_image() {
    let not_images: &[&[u8]] = &[
        b"",
        b"hello, world",
        b"%PDF-1.7",
        b"PK\x03\x04 a zip file",
        b"RIFF\0\0\0\0WAVEfmt ", // audio, not WebP
        b"BM",                   // too short for a bitmap header
        b"\0\0\0\x18ftypmp42",   // a video
        b"<html><body>not svg</body></html>",
        &[0xFF, 0xFE, 0x00],
    ];
    for bytes in not_images {
        assert_eq!(image_mime(bytes), None, "{bytes:?}");
    }
}
