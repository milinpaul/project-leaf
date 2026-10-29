use super::match_expression;

#[test]
fn words_become_quoted_prefix_terms() {
    assert_eq!(match_expression("pro plan").as_deref(), Some(r#""pro"* "plan"*"#));
}

#[test]
fn quoted_phrases_stay_together() {
    assert_eq!(
        match_expression(r#"say "hello wor" now"#).as_deref(),
        Some(r#""say"* "hello wor"* "now"*"#)
    );
}

#[test]
fn an_unclosed_quote_runs_to_the_end() {
    assert_eq!(match_expression(r#"a "b c"#).as_deref(), Some(r#""a"* "b c"*"#));
}

#[test]
fn query_syntax_is_quoted_not_obeyed() {
    assert_eq!(
        match_expression("NOT x OR y*").as_deref(),
        Some(r#""NOT"* "x"* "OR"* "y*"*"#)
    );
    assert_eq!(match_expression(r#"it"s"#).as_deref(), Some(r#""it"* "s"*"#));
    assert_eq!(match_expression("title:x").as_deref(), Some(r#""title:x"*"#));
}

#[test]
fn nothing_searchable_gives_none() {
    for input in ["", "   ", "\"\"", "- * ^", "\" \""] {
        assert_eq!(match_expression(input), None, "{input:?}");
    }
}
