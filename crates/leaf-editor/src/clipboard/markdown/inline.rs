//! Inline Markdown: `**bold**`, `*italic*`, `~~strike~~`, `` `code` ``, `[text](url)` and `<url>`,
//! read into formatted runs and written back from them.
//!
//! The reader follows `CommonMark` where it matters for pasted text: a code span closes only at a run
//! of the same number of backticks (otherwise the backticks are just text), nothing inside code is
//! read as formatting, `_` emphasis only works at word edges (so `snake_case_names` stay as they
//! are), and a backslash makes the next punctuation literal.

use leaf_model::{Marks, TextSpan};

/// Reads one line of inline Markdown into runs.
pub(crate) fn parse(line: &str) -> Vec<TextSpan> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    read(&chars, &Marks::default(), &mut out);
    merge(out)
}

fn read(s: &[char], marks: &Marks, out: &mut Vec<TextSpan>) {
    let mut plain = String::new();
    let mut i = 0;
    while i < s.len() {
        if let Some((inner, inner_marks, next)) = token(s, i, marks) {
            flush(&mut plain, marks, out);
            match inner {
                Inner::Literal(text) => out.push(TextSpan {
                    text,
                    marks: inner_marks,
                }),
                Inner::Parse(range) => read(&s[range], &inner_marks, out),
            }
            i = next;
            continue;
        }
        if s[i] == '`' {
            // A backtick run with no matching run after it is text, all of it.
            let run = s[i..].iter().take_while(|&&c| c == '`').count();
            plain.extend(&s[i..i + run]);
            i += run;
            continue;
        }
        if s[i] == '\\' && s.get(i + 1).is_some_and(char::is_ascii_punctuation) {
            plain.push(s[i + 1]);
            i += 2;
            continue;
        }
        plain.push(s[i]);
        i += 1;
    }
    flush(&mut plain, marks, out);
}

enum Inner {
    /// Text taken as it is (code spans).
    Literal(String),
    /// A part of the line to read further, with more marks.
    Parse(std::ops::Range<usize>),
}

/// A formatting token starting at `i`: what it contains, its marks, and where reading continues.
fn token(s: &[char], i: usize, marks: &Marks) -> Option<(Inner, Marks, usize)> {
    let at = |k: usize| s.get(k).copied();
    match s[i] {
        '`' => {
            let run = s[i..].iter().take_while(|&&c| c == '`').count();
            let start = i + run;
            let end = find_backtick_run(s, start, run)?;
            let mut text: String = s[start..end].iter().collect();
            // One space just inside each end is padding, so `` ` `` can hold a backtick.
            if text.len() > 2 && text.starts_with(' ') && text.ends_with(' ') {
                text = text[1..text.len() - 1].to_string();
            }
            Some((
                Inner::Literal(text),
                Marks {
                    code: true,
                    ..marks.clone()
                },
                end + run,
            ))
        }
        '[' => {
            // The link text ends at the first `]`, which must be followed by `(`.
            let close = s[i + 1..].iter().position(|&c| c == ']')? + i + 1;
            if s.get(close + 1) != Some(&'(') {
                return None;
            }
            let url_end = s[close + 2..].iter().position(|&c| c == ')')? + close + 2;
            let url: String = s[close + 2..url_end].iter().collect();
            let url = url.trim().to_owned();
            if url.is_empty() || url.contains(' ') {
                return None;
            }
            Some((
                Inner::Parse(i + 1..close),
                Marks {
                    link: Some(url),
                    ..marks.clone()
                },
                url_end + 1,
            ))
        }
        '<' => {
            let close = s[i + 1..].iter().position(|&c| c == '>')? + i + 1;
            let url: String = s[i + 1..close].iter().collect();
            let is_url =
                (url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:"))
                    && !url.contains(char::is_whitespace);
            is_url.then(|| {
                (
                    Inner::Literal(url.clone()),
                    Marks {
                        link: Some(url),
                        ..marks.clone()
                    },
                    close + 1,
                )
            })
        }
        '~' if at(i + 1) == Some('~') => delimited(
            s,
            i,
            &['~', '~'],
            Marks {
                strike: true,
                ..marks.clone()
            },
        ),
        c @ ('*' | '_') if at(i + 1) == Some(c) => {
            if c == '_' && !word_edge_before(s, i) {
                return None;
            }
            delimited(
                s,
                i,
                &[c, c],
                Marks {
                    bold: true,
                    ..marks.clone()
                },
            )
        }
        c @ ('*' | '_') => {
            if c == '_' && !word_edge_before(s, i) {
                return None;
            }
            delimited(
                s,
                i,
                &[c],
                Marks {
                    italic: true,
                    ..marks.clone()
                },
            )
        }
        _ => None,
    }
}

/// Text between `delim` at `i` and the next matching `delim`; it must not start or end with a space.
fn delimited(s: &[char], i: usize, delim: &[char], marks: Marks) -> Option<(Inner, Marks, usize)> {
    let start = i + delim.len();
    if s.get(start).is_none_or(|c| c.is_whitespace()) {
        return None;
    }
    let mut k = start + 1;
    while k + delim.len() <= s.len() {
        let closes = s[k..k + delim.len()] == *delim
            && !s[k - 1].is_whitespace()
            // `*italic*` must not close on half of a `**`.
            && (delim.len() > 1 || s.get(k + 1) != Some(&delim[0]))
            && (delim[0] != '_' || s.get(k + delim.len()).is_none_or(|c| !c.is_alphanumeric()));
        if closes {
            return Some((Inner::Parse(start..k), marks, k + delim.len()));
        }
        k += 1;
    }
    None
}

fn word_edge_before(s: &[char], i: usize) -> bool {
    i == 0 || !s[i - 1].is_alphanumeric()
}

fn find_backtick_run(s: &[char], from: usize, run: usize) -> Option<usize> {
    let mut k = from;
    while k < s.len() {
        if s[k] == '`' {
            let len = s[k..].iter().take_while(|&&c| c == '`').count();
            if len == run {
                return Some(k);
            }
            k += len;
        } else {
            k += 1;
        }
    }
    None
}

fn flush(plain: &mut String, marks: &Marks, out: &mut Vec<TextSpan>) {
    if !plain.is_empty() {
        out.push(TextSpan {
            text: std::mem::take(plain),
            marks: marks.clone(),
        });
    }
}

/// Joins neighbouring runs with the same marks, as the document reports them.
pub(crate) fn merge(spans: Vec<TextSpan>) -> Vec<TextSpan> {
    let mut out: Vec<TextSpan> = Vec::new();
    for span in spans.into_iter().filter(|s| !s.text.is_empty()) {
        match out.last_mut() {
            Some(last) if last.marks == span.marks => last.text.push_str(&span.text),
            _ => out.push(span),
        }
    }
    out
}

/// Writes runs as inline Markdown. Characters that would read as formatting are escaped, so
/// [`parse`] gives the same runs back.
pub(crate) fn write(spans: &[TextSpan]) -> String {
    let mut out = String::new();
    for span in spans {
        let marks = &span.marks;
        let mut text = if marks.code {
            code_span(&span.text)
        } else {
            escape(&span.text)
        };
        if marks.strike {
            text = format!("~~{text}~~");
        }
        // `_` rather than `*`, so italic inside bold doesn't read as `***`.
        if marks.italic {
            text = format!("_{text}_");
        }
        if marks.bold {
            text = format!("**{text}**");
        }
        if let Some(href) = &marks.link {
            text = format!("[{text}]({href})");
        }
        // A page link, named by `Fragment::with_link_names`: `[[Title]]` (ADR 0071).
        if marks.page.is_some() {
            text = format!("[[{}]]", span.text.replace("]]", "] ]"));
        }
        out.push_str(&text);
    }
    out
}

/// Escapes only what could read as formatting: `*`, `` ` ``, `~` and `\` always; `_` at a word edge
/// (inside a word it can't, so `snake_case` stays readable); `[` when its `]` is followed by `(`;
/// `<` before a URL.
fn escape(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let alnum = |k: Option<usize>| k.and_then(|k| chars.get(k)).is_some_and(|c| c.is_alphanumeric());
        let rest: String = chars[i + 1..].iter().collect();
        let needs = match c {
            '\\' | '`' | '*' | '~' => true,
            '_' => !(alnum(i.checked_sub(1)) && alnum(Some(i + 1))),
            // Only when its own closing `]` is followed by `(`, making a link.
            '[' => rest.find(']').is_some_and(|k| rest[k + 1..].starts_with('(')),
            '<' => ["http://", "https://", "mailto:"]
                .iter()
                .any(|p| rest.starts_with(p)),
            _ => false,
        };
        if needs {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A code span long enough to hold any backticks in `text`.
fn code_span(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    let pad = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{text}{pad}{fence}")
}
