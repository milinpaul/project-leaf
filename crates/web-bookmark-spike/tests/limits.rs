//! The ADR's limits, against a local server (no real internet).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::thread;
use std::time::{Duration, Instant};

use web_bookmark_spike::fetch::{self, FetchError};
use web_bookmark_spike::guard::{PublicOnly, is_public};

/// Serves `respond(path)` (a whole raw response, or `None` to stall) to every request, forever.
fn serve(respond: fn(&str, SocketAddr) -> Option<Vec<u8>>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap_or(0) <= 2 {
                        break;
                    }
                }
                match respond(&path, addr) {
                    Some(bytes) => {
                        let _ = stream.write_all(&bytes);
                    }
                    None => thread::sleep(Duration::from_secs(30)),
                }
            });
        }
    });
    addr
}

fn ok(kind: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

fn redirect(to: &str) -> Vec<u8> {
    format!("HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .into_bytes()
}

const PAGE: &str = r#"<!doctype html><html><head>
<title>Fallback &amp; title</title>
<meta property="og:title" content="  The   real title ">
<meta name="description" content="Plain description">
<meta property="og:image" content="/cover.png">
<link rel="shortcut icon" href="icons/fav.png">
<script>document.write('<meta property="og:title" content="from a script">')</script>
</head><body><meta property="og:description" content="in the body, ignored"></body></html>"#;

fn routes(path: &str, addr: SocketAddr) -> Option<Vec<u8>> {
    Some(match path {
        "/page" => ok("text/html; charset=utf-8", PAGE.as_bytes()),
        "/hop" => redirect("/page"),
        "/to-loopback" => redirect(&format!("http://127.0.0.2:{}/page", addr.port())),
        "/to-lan" => redirect("http://192.168.1.1/admin"),
        // Another port: the test allowance is the server's exact address.
        "/to-localhost" => redirect("http://localhost:8080/page"),
        "/loop" => redirect("/loop"),
        "/pdf" => ok("application/pdf", b"%PDF-1.7"),
        "/cover.png" => ok("image/png", b"\x89PNG fake"),
        "/huge.png" => ok("image/png", &vec![0; 6 * 1024 * 1024]),
        "/huge-page" => {
            let mut body = b"<html><head><title>Big</title></head><body>".to_vec();
            body.resize(50 * 1024 * 1024, b'x');
            ok("text/html", &body)
        }
        "/slow" => return None,
        _ => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
    })
}

fn setup() -> (ureq::Agent, String) {
    let addr = serve(routes);
    (
        fetch::agent(PublicOnly { allow: Some(addr) }),
        format!("http://{addr}"),
    )
}

#[test]
fn reads_open_graph_then_falls_back() {
    let (agent, base) = setup();
    let preview = fetch::preview(&agent, &format!("{base}/hop")).unwrap();
    assert_eq!(preview.title.as_deref(), Some("The real title"));
    assert_eq!(
        preview.description.as_deref(),
        Some("Plain description"),
        "no og:description in head: meta description"
    );
    assert_eq!(
        preview.image,
        Some(format!("{base}/cover.png")),
        "relative to the final address"
    );
    assert_eq!(preview.icon, Some(format!("{base}/icons/fav.png")));
    assert_eq!(
        fetch::image(&agent, preview.image.as_deref().unwrap()).unwrap(),
        b"\x89PNG fake"
    );
}

#[test]
fn only_http_and_https() {
    let (agent, _) = setup();
    for address in [
        "file:///etc/passwd",
        "ftp://example.com/",
        "javascript:alert(1)",
        "data:text/html,hi",
    ] {
        assert!(
            matches!(fetch::preview(&agent, address), Err(FetchError::Scheme)),
            "{address}"
        );
    }
}

#[test]
fn local_network_refused_even_through_a_redirect() {
    let (agent, base) = setup();
    for path in ["/to-loopback", "/to-lan", "/to-localhost"] {
        let err = fetch::preview(&agent, &format!("{base}{path}")).unwrap_err();
        assert!(
            matches!(err, FetchError::Http(ureq::Error::HostNotFound)),
            "{path}: {err:?}"
        );
    }
    // Without the test allowance the server itself is refused.
    let strict = fetch::agent(PublicOnly::default());
    assert!(fetch::preview(&strict, &format!("{base}/page")).is_err());
}

#[test]
fn redirect_loops_stop() {
    let (agent, base) = setup();
    assert!(matches!(
        fetch::preview(&agent, &format!("{base}/loop")),
        Err(FetchError::Http(ureq::Error::TooManyRedirects))
    ));
}

#[test]
fn not_a_page_or_not_an_image_is_refused() {
    let (agent, base) = setup();
    assert!(matches!(
        fetch::preview(&agent, &format!("{base}/pdf")),
        Err(FetchError::NotHtml(_))
    ));
    assert!(matches!(
        fetch::image(&agent, &format!("{base}/page")),
        Err(FetchError::NotImage(_))
    ));
}

#[test]
fn size_caps_hold() {
    let (agent, base) = setup();
    let err = fetch::image(&agent, &format!("{base}/huge.png")).unwrap_err();
    assert!(
        matches!(err, FetchError::Http(ureq::Error::BodyExceedsLimit(_))),
        "{err:?}"
    );
    // A 50 MiB page: only the first 512 KiB is read, and the title is still found.
    let started = Instant::now();
    let preview = fetch::preview(&agent, &format!("{base}/huge-page")).unwrap();
    assert_eq!(preview.title.as_deref(), Some("Big"));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_stalled_server_times_out() {
    let (agent, base) = setup();
    let started = Instant::now();
    let err = fetch::preview(&agent, &format!("{base}/slow")).unwrap_err();
    assert!(
        matches!(err, FetchError::Http(ureq::Error::Timeout(_))),
        "{err:?}"
    );
    assert!(started.elapsed() < fetch::TOTAL_TIMEOUT + Duration::from_secs(2));
}

#[test]
fn which_addresses_are_public() {
    let public = [
        "8.8.8.8",
        "1.1.1.1",
        "2606:4700:4700::1111",
        "64:ff9b::808:808",
        "2002:808:808::1",
    ];
    let private = [
        "0.0.0.0",
        "127.0.0.1",
        "10.1.2.3",
        "172.16.0.1",
        "172.31.255.255",
        "192.168.0.1",
        "169.254.169.254",
        "100.64.0.1",
        "224.0.0.1",
        "255.255.255.255",
        "192.0.2.1",
        "::",
        "::1",
        "fe80::1",
        "fc00::1",
        "fd12::1",
        "ff02::1",
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.1",
        "64:ff9b::7f00:1",
        "2002:7f00:1::1",
        "2001:db8::1",
        "2001:0::1",
    ];
    for ip in public {
        assert!(is_public(ip.parse::<IpAddr>().unwrap()), "{ip}");
    }
    for ip in private {
        assert!(!is_public(ip.parse::<IpAddr>().unwrap()), "{ip}");
    }
}
