//! The program's own words: every file of share/lang/en is built in as the English every other language falls back to.

use toolkit::toolkit::host::{self, Host};

const ENGLISH: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/english.rs"));

struct Words;

impl Host for Words {
    fn words(&self) -> &'static [(&'static str, &'static str)] {
        ENGLISH
    }
}

static WORDS: Words = Words;

pub fn ready() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        host::set(&WORDS);
    });
}

macro_rules! t {
    ($($args:tt)*) => {{
        $crate::words::ready();
        toolkit::toolkit::t!($($args)*)
    }};
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    fn sources(dir: &Path, out: &mut String) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|x| x == "rs") && path.file_name().is_some_and(|n| n != "words.rs") {
                out.push_str(&std::fs::read_to_string(path).unwrap());
            }
        }
    }

    #[test]
    fn every_key_the_code_asks_for_is_in_the_word_file_and_every_word_is_used() {
        let mut code = String::new();
        sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rust"), &mut code);
        let used: BTreeSet<&str> = code.split("t!(\"").skip(1).filter_map(|s| s.split('"').next()).filter(|k| k.starts_with("snippets-")).collect();
        let words = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("share/lang/en/snippets.ftl")).unwrap();
        let held: BTreeSet<&str> = words.lines().filter(|l| l.starts_with("snippets-")).filter_map(|l| l.split(" =").next()).collect();
        assert_eq!(used.difference(&held).collect::<Vec<_>>(), Vec::<&&str>::new(), "asked for in the code, missing from the word file");
        assert_eq!(held.difference(&used).collect::<Vec<_>>(), Vec::<&&str>::new(), "in the word file, never used");
    }

    #[test]
    fn the_word_file_has_no_fault_the_toolkit_would_drop() {
        let words = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("share/lang/en/snippets.ftl")).unwrap();
        let parsed = toolkit::toolkit::lang::parse(&words);
        assert!(parsed.faults.is_empty(), "{:?}", parsed.faults);
        assert!(parsed.messages.len() > 100);
    }

    #[test]
    fn the_english_comes_back_for_a_key_with_and_without_arguments() {
        assert_eq!(t!("snippets-save"), "Save");
        assert_eq!(t!("snippets-count", count = 1), "1 snippet");
        assert_eq!(t!("snippets-count", count = 5), "5 snippets");
        assert_eq!(t!("snippets-count-found", shown = 2, all = 9), "2 of 9 snippets");
        assert_eq!(t!("snippets-imported", count = 1), "Imported 1 file.");
    }
}
