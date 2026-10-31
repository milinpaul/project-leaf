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
    while let Some(rest) = s.get(i..).filter(|r| !r.is_empty()) {
        if let Some((inner, inner_marks, next)) = token(s, i, marks) {
            flush(&mut plain, marks, out);
            match inner {
                Inner::Literal(text) => out.push(TextSpan {
                    text,
                    marks: inner_marks,
                }),
                Inner::Parse(range) => read(s.get(range).unwrap_or_default(), &inner_marks, out),
            }
            i = next;
            continue;
        }
        let used = match rest {
            // A backtick run with no matching run after it is text, all of it.
            ['`', ..] => {
                let run = backtick_run(rest);
                plain.extend(std::iter::repeat_n('`', run));
                run
            }
            ['\\', p, ..] if p.is_ascii_punctuation() => {
                plain.push(*p);
                2
            }
            [c, ..] => {
                plain.push(*c);
                1
            }
            [] => break,
        };
        // `used` is at most `rest.len()`, so this stays within `s.len()`.
        i = i.saturating_add(used);
    }
    flush(&mut plain, marks, out);
}

enum Inner {
    /// Text taken as it is (code spans).
    Literal(String),
    /// A part of the line to read further, with more marks.
    Parse(std::ops::Range<usize>),
}

/// Inline code starting at the backticks at `i`: its text, marked as code, and where reading continues.
fn code_token(s: &[char], i: usize, marks: &Marks) -> Option<(Inner, Marks, usize)> {
    let run = backtick_run(s.get(i..)?);
    let start = i.checked_add(run)?;
    let end = find_backtick_run(s, start, run)?;
    let mut text: String = s.get(start..end)?.iter().collect();
    // One space just inside each end is padding, so `` ` `` can hold a backtick.
    if text.len() > 2
        && let Some(inner) = text.strip_prefix(' ').and_then(|t| t.strip_suffix(' '))
    {
        text = inner.to_string();
    }
    Some((
        Inner::Literal(text),
        Marks {
            code: true,
            ..marks.clone()
        },
        end.checked_add(run)?,
    ))
}

/// A formatting token starting at `i`: what it contains, its marks, and where reading continues.
fn token(s: &[char], i: usize, marks: &Marks) -> Option<(Inner, Marks, usize)> {
    let following = s.get(i.checked_add(1)?).copied();
    match *s.get(i)? {
        '`' => code_token(s, i, marks),
        '[' => {
            // The link text ends at the first `]`, which must be followed by `(`.
            let text_start = i.checked_add(1)?;
            let close = find(s, text_start, ']')?;
            let open = close.checked_add(1)?;
            if s.get(open) != Some(&'(') {
                return None;
            }
            let url_start = open.checked_add(1)?;
            let url_end = find(s, url_start, ')')?;
            let url: String = s.get(url_start..url_end)?.iter().collect();
            let url = url.trim().to_owned();
            if url.is_empty() || url.contains(' ') {
                return None;
            }
            Some((
                Inner::Parse(text_start..close),
                Marks {
                    link: Some(url),
                    ..marks.clone()
                },
                url_end.checked_add(1)?,
            ))
        }
        '<' => {
            let start = i.checked_add(1)?;
            let close = find(s, start, '>')?;
            let next = close.checked_add(1)?;
            let url: String = s.get(start..close)?.iter().collect();
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
                    next,
                )
            })
        }
        '~' if following == Some('~') => delimited(
            s,
            i,
            &['~', '~'],
            Marks {
                strike: true,
                ..marks.clone()
            },
        ),
        c @ ('*' | '_') if following == Some(c) => {
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
    let first = *delim.first()?;
    let start = i.checked_add(delim.len())?;
    if s.get(start).is_none_or(|c| c.is_whitespace()) {
        return None;
    }
    for k in start.checked_add(1)?..=s.len().checked_sub(delim.len())? {
        let end = k.checked_add(delim.len())?;
        let closes = s.get(k..end) == Some(delim)
            && k.checked_sub(1).and_then(|p| s.get(p)).is_some_and(|c| !c.is_whitespace())
            // `*italic*` must not close on half of a `**`.
            && (delim.len() > 1 || k.checked_add(1).and_then(|n| s.get(n)) != Some(&first))
            && (first != '_' || s.get(end).is_none_or(|c| !c.is_alphanumeric()));
        if closes {
            return Some((Inner::Parse(start..k), marks, end));
        }
    }
    None
}

fn word_edge_before(s: &[char], i: usize) -> bool {
    i.checked_sub(1)
        .and_then(|p| s.get(p))
        .is_none_or(|c| !c.is_alphanumeric())
}

/// The index of the first `c` at or after `from`.
fn find(s: &[char], from: usize, c: char) -> Option<usize> {
    s.get(from..)?.iter().position(|&x| x == c)?.checked_add(from)
}

/// How many backticks `s` starts with.
fn backtick_run(s: &[char]) -> usize {
    s.iter().take_while(|&&c| c == '`').count()
}

fn find_backtick_run(s: &[char], from: usize, run: usize) -> Option<usize> {
    let mut k = from;
    while let Some(rest) = s.get(k..).filter(|r| !r.is_empty()) {
        let step = if rest.first() == Some(&'`') {
            let len = backtick_run(rest);
            if len == run {
                return Some(k);
            }
            len
        } else {
            1
        };
        k = k.checked_add(step)?;
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
        // A page link, named by `Fragment::with_link_names`: `[[Title]]` (ADR-071).
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
    let mut prev: Option<char> = None;
    let mut remaining = chars.as_slice();
    while let Some((&c, tail)) = remaining.split_first() {
        remaining = tail;
        let rest: String = tail.iter().collect();
        let needs = match c {
            '\\' | '`' | '*' | '~' => true,
            '_' => {
                !(prev.is_some_and(char::is_alphanumeric)
                    && tail.first().is_some_and(|c| c.is_alphanumeric()))
            }
            // Only when its own closing `]` is followed by `(`, making a link.
            '[' => rest
                .split_once(']')
                .is_some_and(|(_, after)| after.starts_with('(')),
            '<' => ["http://", "https://", "mailto:"]
                .iter()
                .any(|p| rest.starts_with(p)),
            _ => false,
        };
        if needs {
            out.push('\\');
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

/// A code span long enough to hold any backticks in `text`.
fn code_span(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.saturating_add(1));
    let pad = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{text}{pad}{fence}")
}
