# ADR-XXX. Web bookmark previews: opt-in network access, safety limits and libraries

- **Status:** Proposed
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E15-S01

## Context

A **web bookmark** (E15, Phase 1) is a block showing a web address as a card: the page's title, description, site
icon and preview image, like Notion's bookmark block. The owner chose the name "web bookmark" (2026-10-31) so it is
never confused with a text link or a link to a Leaf page (E10).

Filling in the card means fetching the web page. That is Leaf's first network access, and the request comes from the
owner's own computer, so the site sees their IP address. The epic already settled that fetching is opt-in and off by
default, that only the preview is read, and that the result is saved in the page so cards work offline. This record
pins down the details and the libraries before any code.

A throwaway trial crate tested the libraries and every limit against a local test server. It is kept on the
branch `spike/E15-S01-trial`, `crates/web-bookmark-spike/`, and is never merged.

## Decision

**Naming.** The UI calls the block "Web bookmark" (slash menu, block menu, Settings). It is stored as block type
`web_bookmark`. An older Leaf keeps it untouched as an unknown kind (rule 5).

**Opt-in.** A setting, *Settings › Privacy › Fetch previews for web bookmarks*, is off by default and saved in
`settings.json`. It belongs to the device, not the workspace, because it is about this computer's connection. The
first time a web bookmark is created, Leaf asks once: "Fetch a preview? Leaf visits the address from this computer,
so the site sees your IP address." The choices are **Allow** and **Not now**. The answer is saved and Leaf doesn't
ask again; Settings changes it. With the setting off, no request is ever made. The card then shows the address and
domain.

**When a fetch happens.** Only on this device, and only for:
- a web bookmark the owner creates here (paste, slash menu, Turn into);
- *Refresh preview* in the block menu;
- a web bookmark with no preview yet, when its page opens with the setting on.

A preview that arrived by sync is never fetched again. Opening a page never re-fetches a preview it already has.

**What is read.** One GET of the address, then at most two more: the preview image and the site icon.
- Page: only the `<head>`, through an HTML tokenizer that stops at `<body>` or `</head>`. It reads:
  - the title: `og:title`, then `twitter:title`, then `<title>`;
  - the description: `og:description`, then `twitter:description`, then `<meta name="description">`;
  - the preview image: `og:image`, then `twitter:image`;
  - the icon: `<link rel="icon">`, falling back to `/favicon.ico`;
  - `og:site_name`.

  Text is trimmed and capped at 1,000 characters. Relative addresses resolve against the final address after
  redirects.
- No JavaScript runs. No CSS, fonts or other sub-resources load. No cookies are stored or sent. There is no
  `Referer` and no login. The User-Agent is fixed: `Leaf/<version> (web bookmark preview)`.
- Images are saved as workspace assets (content-addressed, like pasted images). Cards then work offline and sync
  later. An SVG passes the same check as any SVG (ADR-078).

**Limits.** The trial tested the schemes, addresses, redirects, time, HTML and image size, and content type. The rest are for E15-S05 to build and test.

| Limit | Value |
|---|---|
| Schemes | `http` and `https` only, for the page, every redirect, the image and the icon. Addresses with a user name or password are refused. |
| Addresses | Public internet only, checked inside the DNS resolver the connection uses: no loopback, private (10/8, 172.16/12, 192.168/16, fc00::/7), link-local (169.254/16, fe80::/10, the cloud metadata address), carrier NAT, multicast, documentation or reserved ranges, and no IPv4 hidden in IPv6 (mapped, NAT64, 6to4) pointing at those. A redirect or a second DNS answer is checked again, so it can't be used to slip past the check. |
| Redirects | At most 5 |
| Time | 5 s to connect; 10 s for the whole request, redirects included |
| Size | Response headers 64 KiB; HTML: the first 512 KiB only; preview image 5 MiB; icon 1 MiB |
| Content type | The page must say `text/html` or `application/xhtml+xml`; images must say `image/*` |
| Proxy | Direct connection only; the system proxy is ignored (see Open questions) |

**Where it runs.**
- A new native crate, **`leaf-fetch`**, is the only crate that goes online. It depends on `leaf-core` only and
  exposes `preview(address) -> Result<Preview, FetchError>` and `image(address, limit)`. These are blocking calls
  with the limits above built in. It is not a core crate, never compiles for wasm, and never touches documents.
- `leaf-app` / `leaf-ui` call it on GPUI's background executor, so typing never waits. Images go into the asset
  store (`leaf-store`).
- Results reach the page only through a command, `SetWebBookmarkPreview`, applied on the main thread like any edit
  (rule 2). The command does nothing if the block is gone or its address changed in the meantime.
- A preview is not an edit the owner made, so it is kept off the undo stack. Undoing the bookmark's creation removes
  it along with its preview.

**Libraries.**

| Need | Choice | Why |
|---|---|---|
| HTTP | `ureq` 3.5, features `rustls` + `gzip` | Small and blocking, so it fits a background thread with no async runtime. Its resolver hook makes the address check race-free. `reqwest` would bring `tokio` and `hyper`. |
| TLS | `rustls` + `ring`, with Mozilla's root certificates compiled in (`webpki-roots`) | Pure Rust, the same on all three OSes. `webpki-roots` is data under CDLA-Permissive-2.0. `deny.toml` gets a licence exception for that one crate. |
| HTML | `html5ever` 0.27, tokenizer only | Browser-grade and already in the build through `gpui-component`. We use the same version, so there is no duplicate, and it moves with gpui-component. |
| Addresses | `url` 2 | Already in the build through GPUI. |
| Text encodings | `encoding_rs` | Already in the build. It decodes non-UTF-8 pages by their header or `<meta charset>` (E15-S05). |

The trial adds 10 crates to the app build: `ureq`, `ureq-proto`, `utf8-zero`, `httparse`, `rustls`,
`rustls-webpki`, `rustls-pki-types`, `ring`, `untrusted` and `webpki-roots`. With the licence exception,
`cargo deny check` passes: advisories, bans (no new duplicates), licences and sources.

## Trial findings

- The 8 tests against a local server pass:
  - Open Graph is read, with fallbacks to the plain tags.
  - A tag written by a script is ignored, and a tag in the body is not read.
  - `file:`, `ftp:`, `javascript:` and `data:` addresses are refused.
  - Redirects to 127.0.0.2, 192.168.1.1 and `localhost` are refused at DNS time.
  - A redirect loop stops at 5 hops.
  - A PDF is refused as a page, and HTML is refused as an image.
  - A 6 MiB image is refused, and a 50 MiB page is read only up to 512 KiB (its title is still found).
  - A server that never answers times out at 10 s.
  - 28 address cases are classified correctly.
- ureq's resolver hook sits in its `unversioned` module, which may change in any release. We pin `ureq`
  exactly (`=3.5.x`), and the address tests catch any change on upgrade.
- The OS certificate store (`rustls-platform-verifier`) would trust company root certificates too. But it adds 6
  more crates (an Android bridge among them) and still needs the same licence exception, so it is not the default.
- The trial reads every page as UTF-8. Decoding other encodings is part of E15-S05.
- With a system proxy, the proxy looks up the address, not Leaf, so the local-network check can't work. Hence
  direct connections only for now.

## Alternatives considered

- **`reqwest`:** a well-known library, but its blocking mode still runs `tokio` and `hyper` inside. That is dozens more
  crates for one GET request.
- **Checking the address before the request, outside the resolver:** a DNS answer can change between the check and
  the connection, and redirects skip the check. Checking in the resolver covers both.
- **`scraper`, `tl` or `lol_html` for HTML:** `scraper` builds a whole tree we don't need. `tl` is not
  standards-compliant. `lol_html` is large. html5ever is already in the build.
- **Fetching in a hidden web view:** it would run JavaScript, which goes against principle 1 and the epic's
  non-goals.
- **Asking every time:** too noisy for a choice that rarely changes. One question plus a setting is enough.

## Consequences

- E15-S05 builds `leaf-fetch` from the trial: the limits, the resolver, the tokenizer and the test server. It adds
  the `deny.toml` licence exception, encoding detection and the `SetWebBookmarkPreview` command. The rule "no network
  access in Phase 0" stays true: `leaf-fetch` and its callers land in Phase 1, and core crates never go online.
- A web version of Leaf can't fetch previews from the browser, because sites block reading from other origins. It
  will need the sync server to fetch them. That is a later decision.
- Upgrading `gpui-component` may move html5ever's version. `leaf-fetch` moves with it.
- Plain English: Leaf only goes online for a web bookmark if you said yes once. Even then, it reads only the page's
  title, description and two small pictures, from the public internet only, with tight limits on time and size. It
  saves them in the page, so the card keeps working offline.

## Open questions for the owner

1. **Proxy.** Ignore the system proxy (recommended for now, so the local-network check can't be bypassed), or use
   it, for people whose network only allows the web through a proxy?
2. **"Not now".** Never ask again (recommended), or ask again the next time a web bookmark is created?
