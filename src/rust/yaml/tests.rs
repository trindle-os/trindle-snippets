//! Tests for the YAML reader and writer.

use super::*;

fn items(text: &str) -> Vec<Item> {
    read(text).unwrap().items().to_vec()
}

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

#[test]
fn plain_quoted_and_block_values_are_read() {
    let text = "# mine\nmatches:\n  - trigger: \":a\"\n    replace: plain words # note\n\n  - trigger: ':b'\n    replace: |\n      line one\n\n      line three\n  - triggers: [\":c\", ':d', e]\n    replace: >-\n      folded\n      words\n    word: true\n";
    let it = items(text);
    assert_eq!(it.len(), 3);
    assert_eq!(it[0].get("trigger"), Some(&s(":a")));
    assert_eq!(it[0].get("replace"), Some(&s("plain words")));
    assert_eq!(it[1].get("replace"), Some(&s("line one\n\nline three\n")));
    assert_eq!(it[2].get("triggers"), Some(&Value::List(vec![s(":c"), s(":d"), s("e")])));
    assert_eq!(it[2].get("replace"), Some(&s("folded words")));
    assert_eq!(it[2].get("word"), Some(&Value::Bool(true)));
}

#[test]
fn spans_cover_each_snippet_and_value_exactly() {
    let text = "matches:\n- trigger: x\n  replace: \"y\"\n# between\n- trigger: z\n  replace: |\n    a\n    b\nglobal_vars: []\n";
    let doc = read(text).unwrap();
    let it = doc.items();
    assert_eq!(doc.raw(&it[0]), "- trigger: x\n  replace: \"y\"");
    assert_eq!(doc.raw(&it[1]), "- trigger: z\n  replace: |\n    a\n    b");
    let r = &it[1].keys[1];
    assert_eq!(&text[r.start..r.end], "replace: |\n    a\n    b");
}

#[test]
fn a_list_of_triggers_under_the_name_is_read() {
    let it = items("matches:\n  - triggers:\n      - \":one\"\n      - two\n    replace: x\n");
    assert_eq!(it[0].get("triggers"), Some(&Value::List(vec![s(":one"), s("two")])));
}

#[test]
fn what_cannot_be_followed_is_odd_not_wrong() {
    let it = items("matches:\n  - trigger: a\n    replace: &x b\n  - {trigger: c, replace: d}\n  - trigger: e\n    vars:\n      - name: f\n        type: date\n    replace: \"{{f}}\"\n");
    assert!(matches!(it[0].get("replace"), Some(Value::Odd(_))));
    assert!(it[1].odd.is_some());
    assert!(matches!(it[2].get("vars"), Some(Value::Odd(_))));
    assert_eq!(it[2].get("replace"), Some(&s("{{f}}")));
}

#[test]
fn numbers_nulls_and_bools_are_not_words() {
    assert_eq!(plain_value("12"), Value::Number("12".into()));
    assert_eq!(plain_value("1.5e3"), Value::Number("1.5e3".into()));
    assert_eq!(plain_value("~"), Value::Null);
    assert_eq!(plain_value("False"), Value::Bool(false));
    assert_eq!(plain_value("yes"), s("yes"));
    assert_eq!(plain_value(":sig"), s(":sig"));
    assert_eq!(plain_value("1.2.3"), s("1.2.3"));
}

#[test]
fn escapes_and_folds_in_quotes_are_undone() {
    let it = items("matches:\n  - trigger: \"a\\tb\\u00e9\\\"\"\n    replace: \"one\n      two\n\n      three\"\n  - trigger: 'it''s'\n    replace: x\n");
    assert_eq!(it[0].get("trigger"), Some(&s("a\tb\u{e9}\"")));
    assert_eq!(it[0].get("replace"), Some(&s("one two\nthree")));
    assert_eq!(it[1].get("trigger"), Some(&s("it's")));
}

#[test]
fn files_text_cannot_follow_say_why() {
    assert!(read("matches:\n\t- trigger: a\n").unwrap_err().contains("tabs"));
    assert!(read("  matches:\n    - trigger: a\n      replace: b\n").unwrap_err().contains("first line is indented"));
    assert!(read("# note\n\n  matches: []\n").unwrap_err().contains("first line is indented"));
    assert!(read("\tmatches: []\n").is_err());
    assert!(read("- a\n- b\n").is_err());
    assert!(read("matches: [{trigger: a, replace: b}]\n").is_err());
    assert!(read("a: 1\n---\nb: 2\n").is_err());
}

#[test]
fn empty_and_missing_lists_are_told_apart() {
    assert!(matches!(read("matches: []\n").unwrap().list, List::Empty { .. }));
    assert!(matches!(read("matches:\n").unwrap().list, List::Empty { .. }));
    assert!(matches!(read("global_vars: []\n").unwrap().list, List::Missing));
    assert!(matches!(read("").unwrap().list, List::Missing));
}

#[test]
fn what_is_written_reads_back_as_itself() {
    for v in ["plain", ":sig", "with \"quotes\" and \\", "two\nlines", "two\nlines\n", "kept\n\n\n", " leading\nspace", "tab\there", "\nstarts with a break", "a\r\nb", "é and 😀", "# not a comment", "x: y", "yes", "12"] {
        let text = format!("matches:\n  - trigger: \"t\"\n    replace: {}\n", write_str(v, 4));
        let it = items(&text);
        assert_eq!(it[0].get("replace"), Some(&s(v)), "{v:?} was written as {text:?}");
    }
}
