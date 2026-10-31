use super::*;

fn svg(body: &str) -> Vec<u8> {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="40" height="30">{body}</svg>"#)
        .into_bytes()
}

#[test]
fn an_ordinary_svg_is_fine() {
    assert_eq!(
        check_svg(&svg(
            r##"<rect width="10" height="10" fill="url(#g)"/><use href="#r"/>"##
        )),
        Ok(())
    );
    // A tiny PNG embedded as a data URL is part of the file, so it may be drawn.
    let png = r#"<image href="data:image/png;base64,iVBORw0KGgo=" width="4" height="4"/>"#;
    assert_eq!(check_svg(&svg(png)), Ok(()));
}

#[test]
fn illustrator_style_entities_are_fine() {
    let text = r#"<?xml version="1.0"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd" [
  <!ENTITY ns_svg "http://www.w3.org/2000/svg">
]>
<svg xmlns="&ns_svg;" width="10" height="10"><rect width="5" height="5"/></svg>"#;
    assert_eq!(check_svg(text.as_bytes()), Ok(()));
}

#[test]
fn references_to_files_or_urls_are_refused() {
    for href in [
        "/etc/passwd",
        "file:///Users/me/secret.png",
        "../outside.png",
        "photo.jpg",
        "C:\\Users\\me\\a.png",
        "https://example.com/a.png",
        "//example.com/a.png",
        "data:image/svg+xml;base64,PHN2Zy8+",
        "data:text/html,hi",
    ] {
        for attribute in ["href", "xlink:href"] {
            let body = format!(r#"<image {attribute}="{href}" width="4" height="4"/>"#);
            assert_eq!(
                check_svg(&svg(&body)),
                Err(SvgProblem::OutsideReference(href.to_owned())),
                "{attribute}={href}"
            );
        }
    }
}

#[test]
fn css_that_reaches_outside_is_refused() {
    let in_attribute = svg(r#"<rect style="fill:url(/etc/x.png)" width="1" height="1"/>"#);
    assert_eq!(
        check_svg(&in_attribute),
        Err(SvgProblem::OutsideReference("/etc/x.png".to_owned()))
    );
    let import = svg(r#"<style>@import "theme.css"; rect { fill: red }</style>"#);
    assert_eq!(
        check_svg(&import),
        Err(SvgProblem::OutsideReference("@import".to_owned()))
    );
    let in_sheet = svg(r"<style>rect { fill: url('file:///x.png') }</style>");
    assert_eq!(
        check_svg(&in_sheet),
        Err(SvgProblem::OutsideReference("file:///x.png".to_owned()))
    );
}

#[test]
fn an_entity_bomb_is_refused() {
    let text = r#"<?xml version="1.0"?>
<!DOCTYPE svg [
  <!ENTITY a "aaaaaaaaaa">
  <!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;">
  <!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;">
  <!ENTITY d "&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;">
  <!ENTITY e "&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;">
  <!ENTITY f "&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;">
]>
<svg xmlns="http://www.w3.org/2000/svg"><text>&f;</text></svg>"#;
    assert_eq!(check_svg(text.as_bytes()), Err(SvgProblem::Unreadable));
}

#[test]
fn a_huge_picture_is_refused() {
    for size in [
        r#"width="100000" height="10""#,
        r#"width="9000px" height="9000px""#,
        r#"width="300cm" height="300cm""#,
        r#"viewBox="0 0 50000 50000""#,
        r#"width="100%" height="100%" viewBox="0 0 1e6 1e6""#,
    ] {
        let text = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {size}><rect/></svg>"#);
        assert!(
            matches!(check_svg(text.as_bytes()), Err(SvgProblem::TooBig { .. })),
            "{size}"
        );
    }
    let big_but_fine = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="2000"/>"#;
    assert_eq!(check_svg(big_but_fine.as_bytes()), Ok(()));
}

#[test]
fn deep_nesting_is_refused() {
    let depth = MAX_DEPTH + 1;
    let text = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">{}{}</svg>"#,
        "<g>".repeat(depth),
        "</g>".repeat(depth)
    );
    assert_eq!(check_svg(text.as_bytes()), Err(SvgProblem::TooComplex));
    let ok = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">{}{}</svg>"#,
        "<g>".repeat(100),
        "</g>".repeat(100)
    );
    assert_eq!(check_svg(ok.as_bytes()), Ok(()));
}

#[test]
fn too_many_elements_or_bytes_are_refused() {
    let many = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">{}</svg>"#,
        "<g/>".repeat(MAX_NODES as usize)
    );
    assert_eq!(check_svg(many.as_bytes()), Err(SvgProblem::TooComplex));
    let padded = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><!--{}--></svg>"#,
        " ".repeat(MAX_BYTES)
    );
    assert_eq!(check_svg(padded.as_bytes()), Err(SvgProblem::TooLarge));
}

#[test]
fn text_that_is_not_xml_is_unreadable() {
    assert_eq!(check_svg(b"<svg"), Err(SvgProblem::Unreadable));
    assert_eq!(check_svg(&[0xff, 0xfe, 0x00]), Err(SvgProblem::Unreadable));
}

#[test]
fn problems_are_explained_in_plain_words() {
    assert_eq!(
        SvgProblem::OutsideReference("/etc/x.png".to_owned()).to_string(),
        "This SVG points to a file or web address outside it (“/etc/x.png”), so it isn't shown."
    );
    assert_eq!(
        SvgProblem::TooBig {
            width: 9000,
            height: 9000
        }
        .to_string(),
        "This SVG is too big to draw (9000 × 9000)."
    );
}
