use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(crate) fn clear_old(parent: &Path) {
    let old = std::time::Duration::from_secs(3600);
    for e in fs::read_dir(parent).into_iter().flatten().flatten() {
        if e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t.elapsed().is_ok_and(|age| age > old)) {
            let _ = fs::remove_dir_all(e.path());
        }
    }
}

fn store() -> Store {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-snippets")
        .join(format!("{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    let _ = fs::remove_dir_all(&dir);
    clear_old(dir.parent().unwrap());
    let s = Store { config: dir.join("config"), backups: dir.join("backups") };
    fs::create_dir_all(s.match_dir()).unwrap();
    s
}

fn put(s: &Store, rel: &str, text: &str) {
    let path = s.match_dir().join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn get(s: &Store, rel: &str) -> String {
    fs::read_to_string(s.match_dir().join(rel)).unwrap()
}

fn fields(triggers: &[&str], replace: &str, word: bool, case: bool) -> Fields {
    Fields { triggers: triggers.iter().map(|t| t.to_string()).collect(), replace: replace.into(), word, propagate_case: case }
}

fn real(text: &str) -> Vec<serde_yaml::Mapping> {
    let v: serde_yaml::Value = serde_yaml::from_str(text).unwrap_or_else(|e| panic!("not YAML ({e}):\n{text}"));
    v.get("matches").and_then(|m| m.as_sequence()).map(|s| s.iter().map(|i| i.as_mapping().unwrap().clone()).collect()).unwrap_or_default()
}

fn real_str(m: &serde_yaml::Mapping, key: &str) -> Option<String> {
    m.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

const HAND: &str = "# My snippets, written by hand.\nmatches:\n  # Greetings\n  - trigger: ':hi'   # short\n    replace: 'Hello there'\n    label: A greeting\n\n  - triggers: [\":addr\", \":home\"]\n    replace: |\n      1 Long Lane\n      Anytown\n    word: true\n\n  - trigger: \":date\"\n    replace: \"{{today}}\"\n    vars:\n      - name: today\n        type: date\n        params:\n          format: \"%d/%m/%Y\"\n\n# The end.\nglobal_vars: []\n";

#[test]
fn a_hand_written_file_is_listed_as_written() {
    let s = store();
    put(&s, "base.yml", HAND);
    let l = s.read_all();
    assert_eq!(l.files.len(), 1);
    assert_eq!(l.entries.len(), 3);
    let e = &l.entries;
    assert_eq!(e[0].triggers, vec![":hi"]);
    assert_eq!(e[0].replace.as_deref(), Some("Hello there"));
    assert_eq!(e[0].label.as_deref(), Some("A greeting"));
    assert_eq!(e[1].triggers, vec![":addr", ":home"]);
    assert_eq!(e[1].replace.as_deref(), Some("1 Long Lane\nAnytown\n"));
    assert!(e[1].word && !e[1].propagate_case);
    assert!(e[2].advanced.as_deref().unwrap().contains("vars"));
    assert_eq!(e[2].replace, None);
    assert!(e[2].raw.starts_with("- trigger: \":date\"") && e[2].raw.ends_with("format: \"%d/%m/%Y\""));
    assert_eq!(e[1].id, "base.yml#1");
}

#[test]
fn changing_one_value_leaves_every_other_line_as_it_was() {
    let s = store();
    put(&s, "base.yml", HAND);
    let e = s.read_all().entries;
    s.update(&e[0].id, &e[0].raw, &fields(&[":hi"], "Hello again", false, false)).unwrap();
    let after = get(&s, "base.yml");
    let changed: Vec<(&str, &str)> = HAND.lines().zip(after.lines()).filter(|(a, b)| a != b).collect();
    assert_eq!(changed, vec![("    replace: 'Hello there'", "    replace: \"Hello again\"")]);
    assert_eq!(HAND.lines().count(), after.lines().count());
    assert_eq!(real_str(&real(&after)[0], "replace").as_deref(), Some("Hello again"));
}

#[test]
fn saving_unchanged_values_writes_nothing() {
    let s = store();
    put(&s, "base.yml", HAND);
    let e = s.read_all().entries;
    s.update(&e[1].id, &e[1].raw, &fields(&[":addr", ":home"], "1 Long Lane\nAnytown\n", true, false)).unwrap();
    assert_eq!(get(&s, "base.yml"), HAND);
    assert!(!s.backups.exists(), "no copy is kept when nothing is written");
}

#[test]
fn triggers_turn_from_one_to_many_and_back() {
    let s = store();
    put(&s, "base.yml", HAND);
    let e = s.read_all().entries;
    s.update(&e[0].id, &e[0].raw, &fields(&[":hi", ":hey"], "Hello there", false, false)).unwrap();
    let r = real(&get(&s, "base.yml"));
    assert_eq!(r[0].get("triggers").unwrap().as_sequence().unwrap().len(), 2);
    assert!(r[0].get("trigger").is_none());
    assert_eq!(real_str(&r[0], "label").as_deref(), Some("A greeting"), "a value the form does not show stays");
    let e = s.read_all().entries;
    s.update(&e[1].id, &e[1].raw, &fields(&[":addr"], "1 Long Lane\nAnytown\n", true, false)).unwrap();
    let r = real(&get(&s, "base.yml"));
    assert_eq!(real_str(&r[1], "trigger").as_deref(), Some(":addr"));
    assert!(r[1].get("triggers").is_none());
}

#[test]
fn switches_add_and_take_out_their_lines() {
    let s = store();
    put(&s, "base.yml", "matches:\n  - word: true\n    propagate_case: true\n    trigger: \":a\"\n    replace: \"b\"\n");
    let e = s.read_all().entries;
    s.update(&e[0].id, &e[0].raw, &fields(&[":a"], "b", false, false)).unwrap();
    assert_eq!(get(&s, "base.yml"), "matches:\n  - trigger: \":a\"\n    replace: \"b\"\n");
    let e = s.read_all().entries;
    s.update(&e[0].id, &e[0].raw, &fields(&[":a"], "b", true, true)).unwrap();
    assert_eq!(get(&s, "base.yml"), "matches:\n  - trigger: \":a\"\n    replace: \"b\"\n    word: true\n    propagate_case: true\n");
    let e = s.read_all().entries;
    s.update(&e[0].id, &e[0].raw, &fields(&[":a"], "b", false, true)).unwrap();
    assert_eq!(get(&s, "base.yml"), "matches:\n  - trigger: \":a\"\n    replace: \"b\"\n    propagate_case: true\n");
}

#[test]
fn a_last_value_with_no_line_end_after_it_is_changed_and_taken_out() {
    let s = store();
    put(&s, "base.yml", "matches:\n- trigger: a\n  replace: b\n  word: true");
    let e = s.read_all().entries;
    s.update(&e[0].id, &e[0].raw, &fields(&["a"], "two\nlines", false, true)).unwrap();
    let after = get(&s, "base.yml");
    let r = real(&after);
    assert_eq!(real_str(&r[0], "replace").as_deref(), Some("two\nlines"));
    assert!(r[0].get("word").is_none());
    assert_eq!(r[0].get("propagate_case").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn new_snippets_go_at_the_end_whatever_the_list_was() {
    for (start, rel) in [(HAND, "a.yml"), ("matches: []\n", "b.yml"), ("# empty\nmatches:\n", "c.yml"), ("global_vars: []", "d.yml"), ("", "e.yml")] {
        let s = store();
        put(&s, rel, start);
        let before = real(start).len();
        let id = s.create(rel, &fields(&[":new", ":n"], "New words\nover two lines", true, false)).unwrap();
        assert_eq!(id, format!("{rel}#{before}"));
        let r = real(&get(&s, rel));
        assert_eq!(r.len(), before + 1, "{rel}");
        let last = r.last().unwrap();
        assert_eq!(real_str(last, "replace").as_deref(), Some("New words\nover two lines"));
        assert_eq!(last.get("word").and_then(|v| v.as_bool()), Some(true));
        let after = get(&s, rel);
        let mut lines = after.lines();
        assert!(start.lines().filter(|l| !l.starts_with("matches")).all(|l| lines.any(|a| a == l)), "{rel}: every line that was there stays, in order:\n{after}");
    }
}

#[test]
fn deleting_takes_out_only_that_snippet() {
    let s = store();
    put(&s, "base.yml", HAND);
    let e = s.read_all().entries;
    s.delete(&e[1].id, &e[1].raw).unwrap();
    let after = get(&s, "base.yml");
    let r = real(&after);
    assert_eq!(r.len(), 2);
    assert_eq!(real_str(&r[1], "trigger").as_deref(), Some(":date"));
    assert!(after.contains("# Greetings") && after.contains("# The end."));
    assert!(!after.contains("\n\n\n"), "no gap of two blank lines is left:\n{after}");
}

#[test]
fn deleting_the_only_snippet_leaves_an_empty_list() {
    let s = store();
    put(&s, "base.yml", "matches:\n  - trigger: a\n    replace: b\nother: 1\n");
    let e = s.read_all().entries;
    s.delete(&e[0].id, &e[0].raw).unwrap();
    assert_eq!(get(&s, "base.yml"), "matches: []\nother: 1\n");
}

#[test]
fn deleting_the_last_snippet_at_the_end_of_the_file() {
    let s = store();
    put(&s, "base.yml", "matches:\n  - trigger: a\n    replace: b\n  - trigger: c\n    replace: d");
    let e = s.read_all().entries;
    s.delete(&e[1].id, &e[1].raw).unwrap();
    assert_eq!(get(&s, "base.yml"), "matches:\n  - trigger: a\n    replace: b");
}

#[test]
fn what_text_cannot_show_is_never_written_over() {
    let s = store();
    put(&s, "base.yml", HAND);
    put(&s, "packages/hub/package.yml", "matches:\n  - trigger: p\n    replace: q\n");
    let l = s.read_all();
    let adv = &l.entries[2];
    assert!(s.update(&adv.id, &adv.raw, &fields(&[":date"], "x", false, false)).is_err());
    let pkg = l.entries.iter().find(|e| e.file.starts_with("packages")).unwrap();
    assert!(pkg.read_only);
    assert!(s.update(&pkg.id, &pkg.raw, &fields(&["p"], "z", false, false)).is_err());
    assert!(s.delete(&pkg.id, &pkg.raw).is_err());
    assert!(s.create("packages/hub/package.yml", &fields(&["p2"], "z", false, false)).is_err());
    assert_eq!(get(&s, "base.yml"), HAND);
}

#[test]
fn a_snippet_changed_in_its_file_since_it_was_listed_is_not_touched() {
    let s = store();
    put(&s, "base.yml", HAND);
    let e = s.read_all().entries;
    put(&s, "base.yml", &HAND.replace("Hello there", "Changed elsewhere"));
    let got = s.update(&e[0].id, &e[0].raw, &fields(&[":hi"], "Mine", false, false));
    assert!(matches!(got, Err(Problem::Other(w)) if w.contains("changed")));
    assert!(s.delete(&e[0].id, &e[0].raw).is_err());
}

#[test]
fn a_file_that_cannot_be_read_says_why_and_is_left_alone() {
    let s = store();
    put(&s, "bad.yml", "matches:\n\t- trigger: a\n");
    put(&s, "latin.yml", "");
    fs::write(s.match_dir().join("latin.yml"), b"matches:\n  - trigger: \xe9\n    replace: b\n").unwrap();
    let l = s.read_all();
    assert!(l.files.iter().all(|f| f.error.is_some() && f.read_only));
    assert!(s.create("bad.yml", &fields(&["x"], "y", false, false)).is_err());
}

#[test]
fn the_form_is_checked_in_plain_words() {
    assert_eq!(check(&fields(&[" ", ""], "x", false, false)), Err(Problem::Triggers("Add at least one trigger.")));
    assert!(matches!(check(&fields(&["a b"], "x", false, false)), Err(Problem::Triggers(_))));
    assert!(matches!(check(&fields(&[&"x".repeat(101)], "x", false, false)), Err(Problem::Triggers(_))));
    assert!(matches!(check(&fields(&[":a"], "", false, false)), Err(Problem::Replace(_))));
    assert!(matches!(check(&fields(&[":a"], &"x".repeat(200_001), false, false)), Err(Problem::Replace(_))));
    assert_eq!(check(&fields(&[" :a ", ":b"], "x", false, false)).unwrap().triggers, vec![":a", ":b"]);
    assert_eq!(split_triggers(" :a, :b ,,"), vec![":a", ":b"]);
}

#[test]
fn paths_stay_inside_the_snippet_folder() {
    let root = Path::new("/x/match");
    assert_eq!(inside(root, "a.yml"), Some(root.join("a.yml")));
    assert_eq!(inside(root, "sub/a.yaml"), Some(root.join("sub/a.yaml")));
    for bad in ["../a.yml", "/etc/a.yml", "a.txt", "", "sub/../../a.yml", "./a.yml"] {
        assert_eq!(inside(root, bad), None, "{bad}");
    }
    assert_eq!(parse_id("sub/a#b.yml#3"), Some(("sub/a#b.yml".into(), 3)));
    assert_eq!(parse_id("#3"), None);
    assert_eq!(parse_id("a.yml#x"), None);
}

#[test]
fn twenty_copies_are_kept_and_no_more() {
    let s = store();
    put(&s, "base.yml", "matches:\n  - trigger: a\n    replace: b\n");
    for n in 0..25 {
        let e = s.read_all().entries;
        s.update(&e[0].id, &e[0].raw, &fields(&["a"], &format!("b{n}"), false, false)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let kept: Vec<_> = fs::read_dir(&s.backups).unwrap().flatten().collect();
    assert_eq!(kept.len(), 20);
    assert!(kept.iter().all(|k| k.file_name().to_string_lossy().ends_with("__base.yml")));
}

#[test]
fn an_export_brings_everything_back_beside_what_is_there() {
    let s = store();
    put(&s, "base.yml", HAND);
    put(&s, "work/more.yml", "matches: []\n");
    let out = s.backups.parent().unwrap().join("out.tar.gz");
    s.export(&out).unwrap();
    let t = store();
    put(&t, "base.yml", "matches: []\n");
    assert_eq!(import(&t, &out).unwrap(), 2);
    assert_eq!(get(&t, "base.yml"), "matches: []\n", "nothing is written over");
    assert_eq!(get(&t, "base-imported.yml"), HAND);
    assert_eq!(get(&t, "more.yml"), "matches: []\n");
    assert_eq!(import(&t, &t.match_dir().join("more.yml")).unwrap(), 1);
    assert!(t.match_dir().join("more-imported.yml").exists());
    assert!(import(&t, Path::new("/nowhere/a.txt")).is_err());
}

#[test]
fn a_new_file_is_made_by_name_only_once() {
    let s = store();
    assert_eq!(s.create_file("Work Snippets"), Ok("work-snippets.yml".into()));
    assert!(s.create_file("work snippets").is_err());
    assert!(s.create_file("../x").is_err());
    assert_eq!(real(&get(&s, "work-snippets.yml")).len(), 0);
    s.create("work-snippets.yml", &fields(&[":w"], "work", false, false)).unwrap();
    assert_eq!(s.read_all().entries.len(), 1);
}

#[test]
fn the_starting_files_are_made_once() {
    let s = store();
    fs::remove_dir_all(s.match_dir()).unwrap();
    s.ensure_skeleton().unwrap();
    let l = s.read_all();
    assert_eq!(l.entries[0].triggers, vec![":tm"]);
    put(&s, "base.yml", "matches: []\n");
    s.ensure_skeleton().unwrap();
    assert_eq!(get(&s, "base.yml"), "matches: []\n");
}

#[test]
fn the_reader_agrees_with_a_full_yaml_reader() {
    let files = [
        HAND,
        "matches:\n- trigger: x\n  replace: >\n    folded\n    words\n\n    and more\n- trigger: \"q\\\"uote\"\n  replace: 'it''s'\n",
        "---\nmatches:\n  - triggers:\n    - a\n    - 'b'\n    replace: |-\n      kept\n        deeper\n      end\n  - trigger: num\n    replace: \"12\"\n",
        "matches:\n  - trigger: t\n    replace: \"one\n      two\\\n      three\"\n  - trigger: \"\\u00e9\\x41\"\n    replace: |+\n      keep\n\n  - trigger: z\n    replace: plain words\n      over lines\n",
    ];
    for text in files {
        let mine = yaml::read(text).unwrap();
        let theirs = real(text);
        assert_eq!(mine.items().len(), theirs.len());
        for (item, m) in mine.items().iter().zip(&theirs) {
            if describe(item).is_some() {
                continue;
            }
            assert_eq!(item.get("replace"), Some(&Value::Str(real_str(m, "replace").unwrap())), "{text}");
            let t: Vec<String> = match m.get("trigger") {
                Some(t) => vec![t.as_str().unwrap().into()],
                None => m.get("triggers").unwrap().as_sequence().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
            };
            assert_eq!(triggers_of(item), t);
        }
    }
}

#[test]
fn blocks_that_keep_blank_lines_keep_them() {
    let s = store();
    let start = "matches:\n  - trigger: a\n    replace: |+\n      kept\n\n  # about b\n  - trigger: b\n    replace: |\n      bee\n    # about word\n    word: true\n";
    put(&s, "base.yml", start);
    let e = s.read_all().entries;
    assert_eq!(e[0].replace.as_deref(), Some("kept\n\n"));
    assert_eq!(e[1].replace.as_deref(), Some("bee\n"));
    assert!(e[1].word);
    s.delete(&e[1].id, &e[1].raw).unwrap();
    let r = real(&get(&s, "base.yml"));
    assert_eq!(real_str(&r[0], "replace").as_deref(), Some("kept\n\n"));
    s.create("base.yml", &fields(&["c"], "sea", false, false)).unwrap();
    let r = real(&get(&s, "base.yml"));
    assert_eq!(real_str(&r[0], "replace").as_deref(), Some("kept\n\n"));
    assert_eq!(real_str(&r[1], "replace").as_deref(), Some("sea"));
}

#[test]
fn files_whose_names_differ_only_by_a_slash_keep_separate_copies() {
    let s = store();
    put(&s, "a/b.yml", "matches:\n  - trigger: a\n    replace: b\n");
    put(&s, "a_b.yml", "matches:\n  - trigger: c\n    replace: d\n");
    for n in 0..22 {
        let e = s.read_all().entries;
        let flat = e.iter().find(|e| e.file == "a_b.yml").unwrap();
        s.update(&flat.id, &flat.raw, &fields(&["c"], &format!("d{n}"), false, false)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let e = s.read_all().entries;
    let nested = e.iter().find(|e| e.file == "a/b.yml").unwrap();
    s.update(&nested.id, &nested.raw, &fields(&["a"], "z", false, false)).unwrap();
    let names: Vec<String> = fs::read_dir(&s.backups).unwrap().flatten().map(|k| k.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names.iter().filter(|n| n.ends_with("__a_b.yml")).count(), 20);
    assert_eq!(names.iter().filter(|n| n.ends_with("__a%2Fb.yml")).count(), 1);
}

#[test]
fn each_file_keeps_its_own_twenty_copies_privately() {
    let s = store();
    put(&s, "base.yml", "matches:\n  - trigger: a\n    replace: b\n");
    put(&s, "more.yml", "matches:\n  - trigger: c\n    replace: d\n");
    let e = s.read_all().entries;
    let more = e.iter().find(|e| e.file == "more.yml").unwrap().clone();
    s.update(&more.id, &more.raw, &fields(&["c"], "e", false, false)).unwrap();
    for n in 0..25 {
        let e = s.read_all().entries;
        let base = e.iter().find(|e| e.file == "base.yml").unwrap();
        s.update(&base.id, &base.raw, &fields(&["a"], &format!("b{n}"), false, false)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let names: Vec<String> = fs::read_dir(&s.backups).unwrap().flatten().map(|k| k.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names.iter().filter(|n| n.ends_with("__base.yml")).count(), 20);
    assert_eq!(names.iter().filter(|n| n.ends_with("__more.yml")).count(), 1);
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(fs::metadata(&s.backups).unwrap().permissions().mode() & 0o077, 0);
    let one = fs::read_dir(&s.backups).unwrap().flatten().next().unwrap();
    assert_eq!(one.metadata().unwrap().permissions().mode() & 0o077, 0);
}

#[test]
fn a_save_is_refused_when_its_copy_cannot_be_kept() {
    let s = store();
    put(&s, "base.yml", "matches:\n  - trigger: a\n    replace: b\n");
    fs::create_dir_all(s.backups.parent().unwrap()).unwrap();
    fs::write(&s.backups, "a file where the folder should be").unwrap();
    let e = s.read_all().entries;
    let why = s.update(&e[0].id, &e[0].raw, &fields(&["a"], "z", false, false)).unwrap_err();
    assert!(format!("{why:?}").contains("Nothing was saved"));
    assert_eq!(get(&s, "base.yml"), "matches:\n  - trigger: a\n    replace: b\n");
}

fn import(s: &Store, from: &Path) -> Result<usize, String> {
    s.bring_in(read_import(from)?.files)
}

fn archive(name: &str, files: &[(&str, Vec<u8>)]) -> PathBuf {
    let path = store().backups.parent().unwrap().join(name);
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(gz);
    for (n, bytes) in files {
        let mut h = tar::Header::new_gnu();
        h.set_size(bytes.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        tar.append_data(&mut h, n, &bytes[..]).unwrap();
    }
    fs::write(&path, tar.into_inner().unwrap().finish().unwrap()).unwrap();
    path
}

#[test]
fn an_import_with_too_much_in_it_is_refused_not_read_whole() {
    let big = vec![b'#'; 6 * 1024 * 1024];
    let many: Vec<(String, Vec<u8>)> = (0..6).map(|n| (format!("f{n}.yml"), big.clone())).collect();
    let refs: Vec<(&str, Vec<u8>)> = many.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    assert!(read_import(&archive("total.tar.gz", &refs)).unwrap_err().contains("too much"));
    let junk = vec![0u8; 70 * 1024 * 1024];
    assert!(read_import(&archive("inflated.tar.gz", &[("a.bin", junk), ("b.yml", b"matches: []\n".to_vec())])).is_err());
    let hundreds: Vec<(String, Vec<u8>)> = (0..501).map(|n| (format!("f{n}.yml"), b"matches: []\n".to_vec())).collect();
    let refs: Vec<(&str, Vec<u8>)> = hundreds.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    assert!(read_import(&archive("many.tar.gz", &refs)).unwrap_err().contains("too many"));
    let one = vec![b'#'; 9 * 1024 * 1024];
    assert!(read_import(&archive("one.tar.gz", &[("a.yml", one)])).unwrap_err().contains("too large"));
}

#[test]
fn files_with_shell_or_script_variables_are_named_before_anything_is_written() {
    let s = store();
    let path = archive(
        "risky.tar.gz",
        &[
            ("safe.yml", b"matches:\n  - trigger: a\n    replace: b\n".to_vec()),
            ("shell.yml", b"matches:\n  - trigger: a\n    replace: \"{{o}}\"\n    vars:\n      - name: o\n        type: shell\n".to_vec()),
            ("flow.yml", b"matches:\n  - {trigger: a, replace: x, vars: [{type: 'script'}]}\n".to_vec()),
            ("escaped.yml", b"matches:\n  - trigger: a\n    replace: x\n    vars:\n      - type: \"sh\\u0065ll\"\n".to_vec()),
            ("tagged.yml", b"matches:\n  - trigger: a\n    replace: x\n    vars:\n      - type: !!str shell\n".to_vec()),
            ("nested.yml", b"matches:\n  - trigger: a\n    replace: x\n    vars:\n      - name: v\n        type: \"scr\\x69pt\"\n".to_vec()),
            ("folded.yml", b"matches:\n  - trigger: a\n    replace: x\n    vars:\n      - type: \"sh\\\n          ell\"\n".to_vec()),
        ],
    );
    let found = read_import(&path).unwrap();
    assert_eq!(found.risky, ["shell.yml", "flow.yml", "escaped.yml", "tagged.yml", "nested.yml", "folded.yml"].map(String::from).to_vec());
    assert!(!s.match_dir().join("shell.yml").exists());
    assert!(runs_commands(b"matches: [\n  type: !!str shell"));
    assert!(!runs_commands(b"matches:\n  - trigger: shell\n    replace: type is script\n"));
}

#[test]
fn comments_survive_deleting_the_only_snippet_and_adding_to_an_empty_list() {
    let s = store();
    put(&s, "base.yml", "matches: # mine\n  # about this one\n  - trigger: a\n    replace: b\n  # after\n");
    let e = s.read_all().entries;
    s.delete(&e[0].id, &e[0].raw).unwrap();
    let after = get(&s, "base.yml");
    assert!(after.starts_with("matches: [] # mine\n"), "{after}");
    assert!(after.contains("# about this one") && after.contains("# after") && !after.contains("trigger"), "{after}");
    s.create("base.yml", &fields(&["c"], "d", false, false)).unwrap();
    let after = get(&s, "base.yml");
    assert!(after.starts_with("matches: # mine\n"), "{after}");
    assert!(after.contains("# about this one") && after.contains("trigger: c") || after.contains("trigger: \"c\""), "{after}");
    put(&s, "e.yml", "matches: [] # keep me\n");
    s.create("e.yml", &fields(&["x"], "y", false, false)).unwrap();
    let after = get(&s, "e.yml");
    assert!(after.starts_with("matches: # keep me\n  - trigger:"), "{after}");
}

#[test]
fn a_file_with_crlf_line_ends_keeps_them_whole_after_every_change() {
    let s = store();
    let crlf = "# mine\r\nmatches:\r\n  - trigger: a\r\n    replace: b\r\n  - trigger: c\r\n    replace: d\r\n    word: true\r\n";
    put(&s, "base.yml", crlf);
    let lone = |t: &str| t.matches('\n').count() == t.matches("\r\n").count();
    s.create("base.yml", &fields(&["e"], "f\ng", false, false)).unwrap();
    assert!(lone(&get(&s, "base.yml")), "{:?}", get(&s, "base.yml"));
    let e = s.read_all().entries;
    s.update(&e[1].id, &e[1].raw, &fields(&["c"], "d", false, false)).unwrap();
    let after = get(&s, "base.yml");
    assert!(lone(&after) && !after.contains("word: true"), "{after:?}");
    let e = s.read_all().entries;
    s.delete(&e[0].id, &e[0].raw).unwrap();
    let after = get(&s, "base.yml");
    assert!(lone(&after) && !after.contains("\r\r") && !after.contains("trigger: a"), "{after:?}");
    assert!(!after.contains("\r\n\r\n\r\n") && !after.contains("matches:\r\n\r\n"), "no blank line left: {after:?}");
}

#[test]
fn a_hostile_archive_gives_only_plain_files_by_their_last_name() {
    let s = store();
    let path = s.backups.parent().unwrap().join("hostile.tar.gz");
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(gz);
    let file = |tar: &mut tar::Builder<_>, name: &str, kind: tar::EntryType, body: &[u8], link: Option<&str>| {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(kind);
        h.set_size(body.len() as u64);
        h.set_mode(0o644);
        if let Some(l) = link {
            h.set_link_name(l).unwrap();
        }
        h.set_cksum();
        tar.append_data(&mut h, name, body).unwrap();
    };
    file(&mut tar, "link.yml", tar::EntryType::Symlink, b"", Some("/etc/passwd"));
    file(&mut tar, "hard.yml", tar::EntryType::Link, b"", Some("base.yml"));
    file(&mut tar, ".hidden.yml", tar::EntryType::Regular, b"matches: []\n", None);
    file(&mut tar, "notes.txt", tar::EntryType::Regular, b"x", None);
    file(&mut tar, "good.yml", tar::EntryType::Regular, b"matches: []\n", None);
    fs::write(&path, tar.into_inner().unwrap().finish().unwrap()).unwrap();
    assert_eq!(import(&s, &path).unwrap(), 1);
    let names: Vec<String> = fs::read_dir(s.match_dir()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["good.yml".to_string()]);
}

#[test]
fn an_archive_name_with_dots_cannot_land_outside_the_match_folder() {
    let s = store();
    let path = s.backups.parent().unwrap().join("dots.tar.gz");
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(gz);
    let mut h = tar::Header::new_old();
    h.as_old_mut().name[..14].copy_from_slice(b"../../evil.yml");
    h.set_size(12);
    h.set_mode(0o644);
    h.set_cksum();
    tar.append(&h, &b"matches: []\n"[..]).unwrap();
    fs::write(&path, tar.into_inner().unwrap().finish().unwrap()).unwrap();
    let result = import(&s, &path);
    let root = s.match_dir().parent().unwrap().parent().unwrap().to_path_buf();
    assert!(!root.join("evil.yml").exists() && !s.match_dir().parent().unwrap().join("evil.yml").exists());
    let names: Vec<String> = fs::read_dir(s.match_dir()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(names.iter().all(|n| n == "evil.yml"), "{names:?}");
    assert_eq!(result.is_ok(), !names.is_empty(), "{result:?} {names:?}");
}

#[test]
fn an_export_that_would_be_too_large_to_import_is_refused() {
    use std::io::Write;
    let mut held = Capped(Vec::new());
    assert!(held.write_all(&[0u8; 1024]).is_ok());
    assert!(held.write_all(&vec![0u8; TOTAL_MOST as usize]).is_err());
    assert_eq!(held.0.len(), 1024);
}

#[test]
fn a_plain_yml_over_the_size_limit_is_refused_not_read_whole() {
    let s = store();
    let path = s.backups.parent().unwrap().join("big.yml");
    fs::write(&path, vec![b'#'; 9 * 1024 * 1024]).unwrap();
    assert!(read_import(&path).unwrap_err().contains("too large"));
}

#[test]
fn a_file_that_starts_with_a_byte_order_mark_is_left_alone_and_says_why() {
    let s = store();
    put(&s, "bom.yml", "\u{feff}matches:\n  - trigger: a\n    replace: b\n");
    let listing = s.read_all();
    let file = listing.files.iter().find(|f| f.rel == "bom.yml").unwrap();
    assert!(file.error.as_deref().is_some_and(|e| e.contains("byte order mark")));
    assert!(listing.entries.is_empty());
}

#[test]
fn a_link_in_the_snippet_folder_is_exported_as_a_link_not_as_the_words_it_points_at() {
    let s = store();
    let outside = s.backups.parent().unwrap().join("outside.yml");
    fs::write(&outside, "matches:\n  - trigger: ':o'\n    replace: 'outside'\n").unwrap();
    std::os::unix::fs::symlink(&outside, s.match_dir().join("link.yml")).unwrap();
    let out = s.backups.parent().unwrap().join("links.tar.gz");
    s.export(&out).unwrap();
    let packed = flate2::read::GzDecoder::new(fs::File::open(&out).unwrap());
    let kinds: Vec<_> = tar::Archive::new(packed).entries().unwrap().flatten().map(|e| (e.path().unwrap().to_string_lossy().into_owned(), e.header().entry_type())).collect();
    assert!(kinds.iter().any(|(n, k)| n.ends_with("link.yml") && *k == tar::EntryType::Symlink), "{kinds:?}");
    let t = store();
    let _ = import(&t, &out);
    assert!(!fs::read_to_string(t.match_dir().join("link.yml")).unwrap_or_default().contains("outside"));
}

#[test]
fn a_link_in_the_snippet_folder_is_read_only_and_what_it_points_at_is_never_written() {
    let s = store();
    let outside = s.backups.parent().unwrap().join("target.yml");
    let text = "matches:\n  - trigger: ':o'\n    replace: 'outside'\n";
    fs::write(&outside, text).unwrap();
    std::os::unix::fs::symlink(&outside, s.match_dir().join("link.yml")).unwrap();
    let l = s.read_all();
    let e = l.entries.iter().find(|e| e.file == "link.yml").expect("the link is still listed");
    assert!(e.read_only && l.files.iter().all(|f| f.read_only));
    assert!(s.update(&e.id, &e.raw, &fields(&[":o"], "changed", false, false)).is_err());
    assert!(s.delete(&e.id, &e.raw).is_err());
    assert!(s.create("link.yml", &fields(&[":p"], "x", false, false)).is_err());
    assert_eq!(fs::read_to_string(&outside).unwrap(), text);
}

#[test]
fn backups_of_an_earlier_version_move_to_the_new_folder_once() {
    let s = store();
    let base = s.backups.parent().unwrap().join("move-backups");
    let _ = fs::remove_dir_all(&base);
    let (old, new) = (base.join("text/backups"), base.join("snippets/backups"));
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("a__base.yml"), "kept").unwrap();
    assert_eq!(keep_backups_here(&old, new.clone()), new);
    assert_eq!(fs::read_to_string(new.join("a__base.yml")).unwrap(), "kept");
    assert!(!old.exists());
    assert_eq!(keep_backups_here(&old, new.clone()), new);
    let blocked = base.join("file-in-the-way");
    fs::create_dir_all(&old).unwrap();
    fs::write(&blocked, "x").unwrap();
    assert_eq!(keep_backups_here(&old, blocked.join("backups")), old);
}
