//! Read a page's preview from its `<head>`: Open Graph first, then Twitter cards, then `<title>`,
//! `<meta name="description">` and `<link rel="icon">`. Only the tokenizer runs: no tree, no scripts,
//! no styles, and it stops at `<body>`.

use html5ever::tendril::StrTendril;
use html5ever::tokenizer::states::RawKind;
use html5ever::tokenizer::{
    BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};

/// What the page's `<head>` says about it. Addresses are absolute.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub icon: Option<String>,
    pub site: Option<String>,
}

#[derive(Default)]
struct Sink {
    og: Found,
    twitter: Found,
    plain: Found,
    in_title: bool,
    title: String,
    done: bool,
}

const MAX_TEXT: usize = 1000;

fn clean(text: &str) -> Option<String> {
    let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then(|| text.chars().take(MAX_TEXT).collect())
}

impl TokenSink for Sink {
    type Handle = ();

    fn process_token(&mut self, token: Token, _line: u64) -> TokenSinkResult<()> {
        if self.done {
            return TokenSinkResult::Continue;
        }
        match token {
            Token::TagToken(tag) => {
                let name = &*tag.name;
                let attr = |key: &str| {
                    tag.attrs
                        .iter()
                        .find(|a| &*a.name.local == key)
                        .map(|a| a.value.to_string())
                };
                match (tag.kind, name) {
                    (TagKind::StartTag, "body") | (TagKind::EndTag, "head") => self.done = true,
                    (TagKind::StartTag, "title") => {
                        self.in_title = true;
                        return TokenSinkResult::RawData(RawKind::Rcdata);
                    }
                    (TagKind::EndTag, "title") => {
                        self.in_title = false;
                        if self.plain.title.is_none() {
                            self.plain.title = clean(&self.title);
                        }
                    }
                    (TagKind::StartTag, "script") => return TokenSinkResult::RawData(RawKind::ScriptData),
                    (TagKind::StartTag, "style" | "noscript") => {
                        return TokenSinkResult::RawData(RawKind::Rawtext);
                    }
                    (TagKind::StartTag, "meta") => {
                        let key = attr("property")
                            .or_else(|| attr("name"))
                            .unwrap_or_default()
                            .to_lowercase();
                        let value = attr("content").and_then(|v| clean(&v));
                        let slot = match key.as_str() {
                            "og:title" => &mut self.og.title,
                            "og:description" => &mut self.og.description,
                            "og:image" | "og:image:url" | "og:image:secure_url" => &mut self.og.image,
                            "og:site_name" => &mut self.og.site,
                            "twitter:title" => &mut self.twitter.title,
                            "twitter:description" => &mut self.twitter.description,
                            "twitter:image" => &mut self.twitter.image,
                            "description" => &mut self.plain.description,
                            _ => return TokenSinkResult::Continue,
                        };
                        if slot.is_none() {
                            *slot = value;
                        }
                    }
                    (TagKind::StartTag, "link") => {
                        let rel = attr("rel").unwrap_or_default().to_lowercase();
                        let is_icon = rel
                            .split_whitespace()
                            .any(|r| r == "icon" || r == "apple-touch-icon");
                        if is_icon && self.plain.icon.is_none() {
                            self.plain.icon = attr("href");
                        }
                    }
                    _ => {}
                }
            }
            Token::CharacterTokens(text) if self.in_title => self.title.push_str(&text),
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

/// Reads the preview; `base` is the page's final address, for relative image and icon addresses.
#[must_use]
pub(crate) fn read_head(html: &str, base: &url::Url) -> Found {
    let mut tokenizer = Tokenizer::new(Sink::default(), TokenizerOpts::default());
    let mut input = BufferQueue::default();
    input.push_back(StrTendril::from_slice(html));
    let _ = tokenizer.feed(&mut input);
    tokenizer.end();
    let s = tokenizer.sink;
    let absolute = |a: Option<String>| {
        a.and_then(|a| base.join(&a).ok())
            .filter(|u| matches!(u.scheme(), "http" | "https"))
            .map(String::from)
    };
    Found {
        title: s.og.title.or(s.twitter.title).or(s.plain.title),
        description: s.og.description.or(s.twitter.description).or(s.plain.description),
        image: absolute(s.og.image.or(s.twitter.image)),
        // No icon tag: the site's /favicon.ico, which the fetcher may try.
        icon: absolute(s.plain.icon.or_else(|| Some("/favicon.ico".into()))),
        site: s.og.site,
    }
}
