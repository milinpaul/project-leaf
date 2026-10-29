//! Turning what the owner typed into a safe FTS5 query.
//!
//! FTS5 has its own query language (`AND`, `NEAR`, `*`, `^`, column filters, ...), so passing typed
//! text straight through would let a stray `"` or `-` fail the search or change its meaning. Every
//! piece of input is therefore quoted as an FTS5 string, which FTS5 only ever reads as words.

/// The FTS5 `MATCH` expression for `input`, or `None` when it holds nothing searchable (empty,
/// spaces, punctuation only).
///
/// Words and `"quoted phrases"` must all match, in any order. Each piece's last word matches as a
/// prefix, so results appear while a word is still being typed ("pro" finds "project"). An
/// unclosed quote runs to the end of the input.
#[must_use]
pub(crate) fn match_expression(input: &str) -> Option<String> {
    let terms: Vec<String> = pieces(input)
        .into_iter()
        .filter(|piece| piece.chars().any(char::is_alphanumeric))
        .map(|piece| format!("\"{}\"*", piece.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

/// Splits input into single words and quoted phrases (quotes removed).
fn pieces(input: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut rest = input;
    while let Some(start) = rest.find(|c: char| !c.is_whitespace()) {
        rest = &rest[start..];
        if let Some(after_quote) = rest.strip_prefix('"') {
            let end = after_quote.find('"').unwrap_or(after_quote.len());
            pieces.push(after_quote[..end].trim().to_owned());
            rest = after_quote.get(end + 1..).unwrap_or("");
        } else {
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '"')
                .unwrap_or(rest.len());
            pieces.push(rest[..end].to_owned());
            rest = &rest[end..];
        }
    }
    pieces
}

#[cfg(test)]
mod tests;
