//! The list Trindle reads to expand triggers as you type: plain triggers and plain words only, written to Snippets's own home as expansions.json.

use std::path::Path;

use crate::snippets::Entry;

pub const FILE: &str = "expansions.json";
const MOST: usize = 2000;
const LONGEST_TRIGGER: usize = 32;
const LONGEST_REPLACE: usize = 4000;

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn fit(trigger: &str, replace: &str) -> bool {
    trigger.chars().count() >= 3
        && trigger.chars().count() <= LONGEST_TRIGGER
        && !trigger.chars().any(|c| c.is_whitespace() || c.is_control())
        && !replace.is_empty()
        && replace.len() <= LONGEST_REPLACE
        && !replace.contains("{{")
        && !replace.chars().any(char::is_control)
}

pub fn wont_expand(triggers: &[String], replace: &str) -> Option<&'static str> {
    if replace.is_empty() || triggers.is_empty() {
        return None;
    }
    if replace.chars().any(char::is_control) {
        return Some(t!("snippets-wont-line-break"));
    }
    if replace.contains("{{") {
        return Some(t!("snippets-wont-variable"));
    }
    if replace.len() > LONGEST_REPLACE {
        return Some(t!("snippets-wont-long"));
    }
    if !triggers.iter().any(|t| fit(t, replace)) {
        return Some(t!("snippets-wont-trigger"));
    }
    None
}

pub fn render(entries: &[Entry]) -> String {
    let mut out = String::from("{\"version\":1,\"snippets\":[");
    let mut n = 0;
    let mut seen = std::collections::HashSet::new();
    for e in entries.iter().filter(|e| e.advanced.is_none() && !e.picky) {
        let Some(replace) = e.replace.as_deref() else { continue };
        for t in e.triggers.iter().filter(|t| fit(t, replace) && seen.insert(t.as_str())) {
            if n == MOST {
                break;
            }
            if n > 0 {
                out.push(',');
            }
            out.push_str("{\"trigger\":");
            quote(t, &mut out);
            out.push_str(",\"replace\":");
            quote(replace, &mut out);
            out.push_str(&format!(",\"word\":{},\"case\":{}}}", e.word, e.propagate_case));
            n += 1;
        }
    }
    out.push_str("]}\n");
    out
}

const OFF_FILE: &str = "expand-off";

pub fn is_off(folder: &Path) -> bool {
    folder.join(OFF_FILE).exists()
}

pub fn set_on(folder: &Path, on: bool) -> std::io::Result<()> {
    if on {
        match std::fs::remove_file(folder.join(OFF_FILE)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    } else {
        toolkit::common::files::write_whole_private(&folder.join(OFF_FILE), b"")
    }
}

pub fn publish(entries: &[Entry], folder: &Path) -> std::io::Result<()> {
    let off = is_off(folder);
    let (headline, hidden) = if off { (t!("snippets-expansion-off"), "off") } else { (t!("snippets-expansion-on"), "on") };
    let mut ring = String::from("{\"headline\":");
    quote(headline, &mut ring);
    ring.push_str(&format!(",\"hidden\":[\"{hidden}\"]}}\n"));
    toolkit::common::files::write_whole_private(&folder.join(FILE), render(if off { &[] } else { entries }).as_bytes())?;
    toolkit::common::files::write_whole_private(&folder.join("ring.json"), ring.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(triggers: &[&str], replace: Option<&str>, advanced: bool) -> Entry {
        Entry {
            id: "base.yml#0".into(),
            file: "base.yml".into(),
            index: 0,
            read_only: false,
            advanced: advanced.then(|| "uses variables".to_string()),
            triggers: triggers.iter().map(|t| t.to_string()).collect(),
            label: None,
            replace: replace.map(String::from),
            word: false,
            propagate_case: false,
            picky: false,
            raw: String::new(),
        }
    }

    #[test]
    fn only_plain_snippets_are_offered_with_their_words_exactly() {
        let list = vec![
            entry(&["tn#", "tnn#"], Some("Line one \"two\"\\"), false),
            entry(&["ml#"], Some("Line one\nline two"), false),
            entry(&["ab"], Some("too short a trigger"), false),
            entry(&["has space"], Some("x"), false),
            entry(&[":v"], Some("Hi {{name}}"), true),
            entry(&["emp#"], Some(""), false),
            entry(&["frm#"], None, true),
        ];
        let text = render(&list);
        assert_eq!(
            text,
            "{\"version\":1,\"snippets\":[{\"trigger\":\"tn#\",\"replace\":\"Line one \\\"two\\\"\\\\\",\"word\":false,\"case\":false},{\"trigger\":\"tnn#\",\"replace\":\"Line one \\\"two\\\"\\\\\",\"word\":false,\"case\":false}]}\n"
        );
    }

    #[test]
    fn the_file_is_written_whole_and_replaced() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("expand-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        publish(&[entry(&["tn#"], Some("a"), false)], &dir).unwrap();
        publish(&[entry(&["xy#"], Some("b"), false)], &dir).unwrap();
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(text.contains("xy#") && !text.contains("tn#"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn switching_off_empties_the_list_and_on_brings_it_back() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("switch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let list = [entry(&["tn#"], Some("a"), false)];
        set_on(&dir, false).unwrap();
        publish(&list, &dir).unwrap();
        assert!(!std::fs::read_to_string(dir.join(FILE)).unwrap().contains("tn#"));
        assert!(std::fs::read_to_string(dir.join("ring.json")).unwrap().contains("is off"));
        set_on(&dir, true).unwrap();
        publish(&list, &dir).unwrap();
        assert!(std::fs::read_to_string(dir.join(FILE)).unwrap().contains("tn#"));
        assert!(std::fs::read_to_string(dir.join("ring.json")).unwrap().contains("is on"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn words_with_controls_picky_matching_or_a_repeated_trigger_are_left_out() {
        let mut picky = entry(&["pic#"], Some("x"), false);
        picky.picky = true;
        let list = [
            entry(&["esc#"], Some("a\u{1b}b"), false),
            entry(&["cr#"], Some("a\rb"), false),
            entry(&["tab#"], Some("a\tb\nc"), false),
            entry(&["ok#"], Some("fine"), false),
            picky,
            entry(&["dup#"], Some("first"), false),
            entry(&["dup#"], Some("second"), false),
        ];
        let text = render(&list);
        assert!(!text.contains("esc#") && !text.contains("cr#") && !text.contains("pic#"), "{text}");
        assert!(!text.contains("tab#") && text.contains("ok#") && text.contains("first") && !text.contains("second"), "{text}");
    }

    #[test]
    fn a_snippet_that_will_not_expand_says_why() {
        let t = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(wont_expand(&t(&[":tm"]), "ok").is_none());
        assert!(wont_expand(&t(&[":tm"]), "a\nb").unwrap().contains("line break"));
        assert!(wont_expand(&t(&["ab"]), "ok").unwrap().contains("3 to 32"));
        assert!(wont_expand(&t(&["ab", ":tm"]), "ok").is_none());
        assert!(wont_expand(&t(&[":tm"]), "{{x}}").unwrap().contains("variable"));
        assert!(wont_expand(&t(&[":tm"]), "").is_none());
    }

    #[test]
    fn the_published_files_are_for_this_person_alone() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("mode-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        publish(&[entry(&["tn#"], Some("a"), false)], &dir).unwrap();
        for name in [FILE, "ring.json"] {
            assert_eq!(std::fs::metadata(dir.join(name)).unwrap().permissions().mode() & 0o077, 0, "{name}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
