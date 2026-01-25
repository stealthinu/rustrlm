use pretty_assertions::assert_eq;

use rlm_runner::final_parser::{extract_final, extract_final_var_name, is_final};

#[test]
fn final_literal_double_quotes() {
    assert_eq!(extract_final(r#"FINAL("hello")"#), Some("hello".into()));
    assert!(is_final(r#"FINAL("hello")"#));
}

#[test]
fn final_literal_single_quotes() {
    assert_eq!(extract_final("FINAL('hello')"), Some("hello".into()));
}

#[test]
fn final_literal_triple_quotes_multiline() {
    let s = "prefix\nFINAL(\"\"\"a\nb\nc\"\"\")\nsuffix";
    assert_eq!(extract_final(s), Some("a\nb\nc".into()));
}

#[test]
fn final_var_name_extract() {
    assert_eq!(extract_final_var_name("FINAL_VAR(ans)"), Some("ans".into()));
    assert!(is_final("xxx FINAL_VAR(ans) yyy"));
}

#[test]
fn final_non_literal_is_not_extracted() {
    assert_eq!(extract_final("FINAL(ans)"), None);
}
