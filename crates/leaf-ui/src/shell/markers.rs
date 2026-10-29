//! List markers by nesting depth, as in Notion and most writing apps: bullets go •, ◦, ▪ and numbers
//! go 1., a., i., then repeat, so nested lists are easy to tell apart.

/// The bullet for a bulleted item at `depth`.
pub(super) fn bullet(depth: usize) -> &'static str {
    ["•", "◦", "▪"][depth % 3]
}

/// The label for numbered item `n` (counted from 1) at `depth`.
pub(super) fn number(n: usize, depth: usize) -> String {
    match depth % 3 {
        0 => format!("{n}."),
        1 => format!("{}.", letters(n)),
        _ => format!("{}.", roman(n)),
    }
}

/// a, b, …, z, aa, ab, … like spreadsheet columns.
fn letters(mut n: usize) -> String {
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push(char::from(b'a' + u8::try_from(n % 26).unwrap_or(0)));
        n /= 26;
    }
    out.iter().rev().collect()
}

/// Lower-case Roman numerals; numbers past 3999 fall back to digits.
fn roman(n: usize) -> String {
    if n == 0 || n > 3999 {
        return n.to_string();
    }
    let table = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut rest = n;
    let mut out = String::new();
    for (value, numeral) in table {
        while rest >= value {
            out.push_str(numeral);
            rest -= value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{bullet, number};

    #[test]
    fn bullets_change_with_depth_and_repeat() {
        let bullets: Vec<&str> = (0..5).map(bullet).collect();
        assert_eq!(bullets, ["•", "◦", "▪", "•", "◦"]);
    }

    #[test]
    fn numbers_go_digits_letters_roman_by_depth() {
        assert_eq!(number(3, 0), "3.");
        assert_eq!(number(1, 1), "a.");
        assert_eq!(number(26, 1), "z.");
        assert_eq!(number(27, 1), "aa.");
        assert_eq!(number(4, 2), "iv.");
        assert_eq!(number(1994, 2), "mcmxciv.");
        assert_eq!(number(4000, 2), "4000.");
        assert_eq!(number(12, 3), "12.", "and around again");
    }
}
