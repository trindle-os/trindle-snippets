use super::view::{list_row, short};
use super::*;
use crate::sheet::CSS;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use toolkit::common::sheet::Layers;
use toolkit::toolkit::theme::Theme;

fn folder() -> Store {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-window")
        .join(format!("{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    let _ = fs::remove_dir_all(&dir);
    crate::snippets::tests::clear_old(dir.parent().unwrap());
    let s = Store { config: dir.join("config"), backups: dir.join("backups") };
    fs::create_dir_all(s.match_dir()).unwrap();
    fs::write(
        s.match_dir().join("base.yml"),
        "# mine\nmatches:\n  - trigger: \":addr\"\n    replace: \"1 Long Lane\\nAnytown\"\n  - triggers: [\":sig\", \":s\"]\n    replace: \"John Smith\"\n    word: true\n  - trigger: \":date\"\n    replace: \"{{d}}\"\n    vars: []\n",
    )
    .unwrap();
    fs::create_dir_all(s.match_dir().join("packages/greet")).unwrap();
    fs::write(s.match_dir().join("packages/greet/package.yml"), "matches:\n  - trigger: \":hello\"\n    replace: Hello\n").unwrap();
    s
}

fn text(store: Store) -> Snippets {
    Snippets::new(store, Sizes::from(&Layers::new(CSS, "")), Loop::new(), None, None, None)
}

fn typed(t: &mut Snippets, words: &str) {
    t.pane.borrow_mut().paste(words);
}

#[test]
fn the_search_finds_triggers_words_and_labels_and_counts_them() {
    let mut t = text(folder());
    assert_eq!(t.count(), "4 snippets");
    t.update(Msg::Search("SMITH".into()));
    assert_eq!(t.visible().len(), 1);
    assert_eq!(t.count(), "1 of 4 snippets");
    t.update(Msg::Search(":s".into()));
    assert_eq!(t.visible().iter().map(|e| e.triggers[0].as_str()).collect::<Vec<_>>(), vec![":sig"]);
    t.update(Msg::Search("nothing like it".into()));
    assert!(t.visible().is_empty());
}

#[test]
fn rows_say_what_is_special_about_a_snippet() {
    let t = text(folder());
    let rows: Vec<Item> = t.listing.entries.iter().map(|e| list_row(e, true, &short(&preview(e).split_whitespace().collect::<Vec<_>>().join(" "), 90), "")).collect();
    assert_eq!(rows[1], item(":sig").under("John Smith").detail("+1 more, base"));
    assert_eq!(rows[2], item(":date").under("- trigger: \":date\" replace: \"{{d}}\" vars: []").detail("advanced, base"));
    assert_eq!(rows[3], item(":hello").under("Hello").detail("package, greet"));
    assert_eq!(short("a b c", 90), "a b c");
    assert_eq!(short(&"x".repeat(100), 10), format!("{}\u{2026}", "x".repeat(9)));
}

#[test]
fn picking_a_snippet_fills_the_form_and_saving_changes_its_file() {
    let store = folder();
    let mut t = text(store.clone());
    t.update(Msg::Pick(1));
    assert_eq!(t.triggers, ":sig, :s");
    assert_eq!(t.pane.borrow().doc.text(), "John Smith");
    assert!(t.word && !t.case);
    t.update(Msg::Triggers(":sig".into()));
    t.update(Msg::Case(true));
    t.update(Msg::Save);
    assert_eq!(t.problems, Problems::default());
    assert_eq!(t.done.as_deref(), Some("Snippet saved."));
    let after = fs::read_to_string(store.match_dir().join("base.yml")).unwrap();
    assert!(after.contains("  - trigger: \":sig\"\n    replace: \"John Smith\"\n    word: true\n    propagate_case: true\n"), "{after}");
    assert!(after.starts_with("# mine\n"));
}

#[test]
fn a_new_snippet_is_checked_then_added() {
    let store = folder();
    let mut t = text(store.clone());
    t.update(Msg::New);
    assert!(t.drafting);
    t.update(Msg::Save);
    assert_eq!(t.problems.triggers.as_deref(), Some("Add at least one trigger."));
    t.update(Msg::Triggers(":tel".into()));
    t.update(Msg::Save);
    assert_eq!(t.problems.replace.as_deref(), Some("Add the replacement text."));
    typed(&mut t, "07700 900123");
    t.update(Msg::Save);
    assert_eq!(t.done.as_deref(), Some("Snippet added."));
    assert!(!t.drafting);
    assert_eq!(t.chosen().map(|e| e.triggers.clone()), Some(vec![":tel".to_string()]));
    assert_eq!(t.listing.entries.len(), 5);
}

#[test]
fn what_cannot_be_edited_cannot_be_saved_or_typed_in() {
    let store = folder();
    let before = fs::read_to_string(store.match_dir().join("base.yml")).unwrap();
    let mut t = text(store.clone());
    t.update(Msg::Pick(2));
    assert!(t.locked());
    t.update(Msg::Triggers(":changed".into()));
    assert_eq!(t.triggers, ":date");
    t.update(Msg::Save);
    t.update(Msg::Pick(3));
    assert!(t.locked());
    t.update(Msg::Delete);
    assert_eq!(fs::read_to_string(store.match_dir().join("base.yml")).unwrap(), before);
    assert!(store.match_dir().join("packages/greet/package.yml").exists());
}

#[test]
fn deleting_and_putting_back() {
    let store = folder();
    let mut t = text(store.clone());
    t.update(Msg::Pick(0));
    typed(&mut t, "Flat 2, ");
    t.update(Msg::Cancel);
    assert_eq!(t.pane.borrow().doc.text(), "1 Long Lane\nAnytown");
    t.update(Msg::Delete);
    assert_eq!(t.listing.entries.len(), 4);
    assert!(t.done.as_deref().is_some_and(|w| w.contains("again")));
    t.update(Msg::Delete);
    assert_eq!(t.done.as_deref(), Some("Snippet deleted. Press Restore to undo."));
    assert_eq!(t.listing.entries.len(), 3);
    assert!(t.selected.is_none());
    t.update(Msg::PutBack);
    assert_eq!(t.listing.entries.len(), 4);
    assert_eq!(t.chosen().map(|e| e.triggers.clone()), Some(vec![":addr".to_string()]));
}

#[test]
fn a_file_changed_elsewhere_is_read_again_and_the_form_kept() {
    let store = folder();
    let mut t = text(store.clone());
    t.update(Msg::Pick(1));
    typed(&mut t, "Dr ");
    fs::write(store.match_dir().join("more.yml"), "matches:\n  - trigger: x\n    replace: y\n").unwrap();
    t.handle(Do::Reload);
    assert_eq!(t.listing.entries.len(), 5);
    assert_eq!(t.pane.borrow().doc.text(), "Dr John Smith");
    assert_eq!(t.writable_files(), vec!["base.yml".to_string(), "more.yml".to_string()]);
}

#[test]
fn pictures_of_the_window() {
    let store = folder();
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("pictures");
    fs::create_dir_all(&out).unwrap();
    let states: Vec<(&str, fn(&mut Snippets))> = vec![
        ("nothing-picked", |_| {}),
        ("a-snippet", |t| t.update(Msg::Pick(1))),
        ("advanced", |t| t.update(Msg::Pick(2))),
        ("new-with-a-problem", |t| {
            t.update(Msg::New);
            t.update(Msg::Save);
        }),
        ("naming-a-file", |t| {
            t.update(Msg::New);
            t.update(Msg::NewFile);
            t.update(Msg::FileName("bad/name".into()));
            t.update(Msg::MakeFile);
        }),
    ];
    let mut seen: Vec<(String, Vec<u8>)> = Vec::new();
    for light in [false, true] {
        for (name, state) in &states {
            let mut t = text(store.clone());
            state(&mut t);
            let (w, h) = t.sizes.window;
            let mut ui = toolkit::toolkit::ui::Ui::new(t.view(), Theme::new(light, "#fbc711"));
            let mut pix = tiny_skia::Pixmap::new(w, h).unwrap();
            ui.paint(&mut pix.as_mut(), 1.0);
            let key = format!("{name}-{}", if light { "light" } else { "dark" });
            let first = &pix.data()[..4];
            assert!(pix.data().chunks(4).any(|p| p != first), "{key} is one flat colour");
            pix.save_png(out.join(format!("{key}.png"))).unwrap();
            seen.push((key, pix.data().to_vec()));
        }
    }
    for (i, (a, pa)) in seen.iter().enumerate() {
        for (b, pb) in &seen[i + 1..] {
            assert_ne!(pa, pb, "{a} and {b} were painted the same");
        }
    }
}

#[test]
fn a_new_snippet_can_go_in_a_new_file_made_by_name() {
    let mut t = text(folder());
    t.update(Msg::New);
    t.update(Msg::NewFile);
    t.update(Msg::FileName("bad/name".into()));
    t.update(Msg::MakeFile);
    assert!(t.problems.file.is_some());
    assert!(t.naming.is_some());
    t.update(Msg::FileName("Work Mail".into()));
    assert!(t.problems.file.is_none());
    t.update(Msg::MakeFile);
    assert!(t.naming.is_none() && t.problems.file.is_none());
    assert_eq!(t.writable_files()[t.file], "work-mail.yml");
    t.update(Msg::Triggers(":hi".into()));
    typed(&mut t, "Hello there");
    t.update(Msg::Save);
    assert!(t.problems.other.is_none() && t.problems.triggers.is_none(), "{:?}", t.problems);
    assert!(t.store.read_all().entries.iter().any(|e| e.triggers[0] == ":hi" && e.file == "work-mail.yml"));
    t.update(Msg::New);
    t.update(Msg::NewFile);
    t.update(Msg::FileName("work mail".into()));
    t.update(Msg::MakeFile);
    assert!(t.problems.file.is_some(), "a name already used is refused");
    t.update(Msg::NoFile);
    assert!(t.naming.is_none());
}

#[test]
fn search_finds_letters_whatever_their_case() {
    assert_eq!(super::view::found("Hello hello", "LL"), vec![2..4, 8..10]);
    assert_eq!(super::view::found("abc", ""), Vec::<std::ops::Range<usize>>::new());
    assert_eq!(super::view::found("caf\u{e9} bar", "\u{e9} b"), vec![3..7]);
}

#[test]
fn copy_gives_the_words_of_every_kind_of_snippet_or_says_why_not() {
    let store = folder();
    fs::write(
        store.match_dir().join("more.yml"),
        "matches:\n  - trigger: \":v\"\n    replace: \"Hi {{name}}\"\n    vars:\n      - name: name\n        type: echo\n        params:\n          echo: x\n  - trigger: \":f\"\n    form: \"[[a]]\"\n  - regex: \"r(\\\\d)\"\n    replace: \"R\"\n  - trigger: \":e\"\n    replace: \"\"\n",
    )
    .unwrap();
    let mut t = text(store);
    let ids: Vec<(String, bool, Option<String>)> = t.listing.entries.iter().map(|e| (e.id.clone(), e.advanced.is_some(), e.replace.clone())).collect();
    assert!(ids.len() >= 8, "{ids:?}");
    for (id, advanced, replace) in ids {
        t.update(Msg::Pick(t.visible().iter().position(|e| e.id == id).unwrap()));
        match (advanced, replace.as_deref()) {
            (false, Some(r)) if !r.is_empty() => assert_eq!(t.copy_words(), Ok(r.to_string()), "{id}"),
            _ => assert!(t.copy_words().is_err_and(|w| !w.is_empty()), "{id} must say why nothing is copied"),
        }
    }
}

#[test]
fn the_starter_snippet_is_tm_and_types_its_words() {
    let store = folder();
    let _ = fs::remove_dir_all(store.match_dir());
    store.ensure_skeleton().unwrap();
    let listing = store.read_all();
    let tm = listing.entries.iter().find(|e| e.triggers == [":tm".to_string()]).expect("the starter snippet is listed");
    assert_eq!(tm.replace.as_deref(), Some("Typed by Snippets"));
    assert!(crate::expand::render(&listing.entries).contains("\"trigger\":\":tm\""));
}

#[test]
fn saving_after_an_outside_edit_keeps_the_form_and_changes_nothing() {
    let store = folder();
    let mut t = text(store.clone());
    t.update(Msg::Pick(1));
    typed(&mut t, "Dr ");
    let path = store.match_dir().join("base.yml");
    let outside = fs::read_to_string(&path).unwrap().replace("John Smith", "Jane Doe");
    fs::write(&path, &outside).unwrap();
    t.handle(Do::Reload);
    t.update(Msg::Save);
    assert!(t.problems.other.as_deref().is_some_and(|w| w.contains("changed in its file")));
    assert_eq!(t.pane.borrow().doc.text(), "Dr John Smith");
    assert_eq!(fs::read_to_string(&path).unwrap(), outside);
    t.update(Msg::Delete);
    assert_eq!(fs::read_to_string(&path).unwrap(), outside);
}

#[test]
fn unsaved_words_are_not_dropped_by_picking_or_new_and_are_kept_on_closing() {
    let store = folder();
    let mut t = text(store.clone());
    t.update(Msg::Pick(1));
    typed(&mut t, "Dr ");
    t.update(Msg::Pick(0));
    assert_eq!(t.pane.borrow().doc.text(), "Dr John Smith");
    assert!(t.problems.other.as_deref().is_some_and(|w| w.contains("unsaved")));
    t.update(Msg::New);
    assert!(!t.drafting);
    t.keep_unsaved();
    let kept = fs::read_to_string(store.backups.join("unsaved.txt")).unwrap();
    assert!(kept.contains("Dr John Smith"));
    t.update(Msg::Cancel);
    t.update(Msg::Pick(0));
    assert_eq!(t.pane.borrow().doc.text(), "1 Long Lane\nAnytown");
}

#[test]
fn a_changed_form_whose_snippet_vanished_stays_as_a_draft() {
    let store = folder();
    let mut t = text(store.clone());
    t.update(Msg::Pick(0));
    typed(&mut t, "Flat 2, ");
    fs::write(store.match_dir().join("base.yml"), "matches: []\n").unwrap();
    t.handle(Do::Reload);
    assert!(t.drafting);
    assert_eq!(t.pane.borrow().doc.text(), "Flat 2, 1 Long Lane\nAnytown");
}

#[test]
fn an_import_with_commands_in_it_waits_for_a_yes_and_a_plain_one_does_not() {
    let store = folder();
    let mut t = text(store.clone());
    let from = store.backups.parent().unwrap().join("in.yml");
    fs::write(&from, "matches:\n  - trigger: a\n    replace: \"{{o}}\"\n    vars:\n      - name: o\n        type: shell\n").unwrap();
    t.chosen_file(Purpose::Import, Some(from.clone()));
    assert!(t.pending.is_some());
    assert!(!store.match_dir().join("in.yml").exists());
    t.update(Msg::Leave);
    assert!(t.pending.is_none() && !store.match_dir().join("in.yml").exists());
    t.chosen_file(Purpose::Import, Some(from));
    t.update(Msg::BringIn);
    assert!(store.match_dir().join("in.yml").exists());
    let plain = store.backups.parent().unwrap().join("plain.yml");
    fs::write(&plain, "matches:\n  - trigger: p\n    replace: q\n").unwrap();
    t.chosen_file(Purpose::Import, Some(plain));
    assert!(t.pending.is_none() && store.match_dir().join("plain.yml").exists());
}

#[test]
fn the_list_for_typing_is_written_where_told_and_a_failure_is_said() {
    let store = folder();
    let mut t = text(store.clone());
    let out = store.backups.parent().unwrap().join("home");
    fs::create_dir_all(&out).unwrap();
    t.publish_to = Some(out.clone());
    t.reload();
    assert!(fs::read_to_string(out.join(crate::expand::FILE)).unwrap().starts_with("{\"version\":1,"));
    t.publish_to = Some(out.join("missing/dir/blocked"));
    fs::write(out.join("missing"), "a file, not a folder").unwrap();
    t.reload();
    assert!(t.problems.other.as_deref().is_some_and(|w| w.contains("typing expansion")));
}

#[test]
fn the_search_follows_a_save_and_is_not_fooled_by_the_gap_between_words() {
    let store = folder();
    let mut t = text(store);
    t.update(Msg::Pick(1));
    t.update(Msg::Triggers(":sig".into()));
    typed(&mut t, "Zed ");
    t.update(Msg::Save);
    t.update(Msg::Search("zed john".into()));
    assert_eq!(t.visible().len(), 1);
    t.update(Msg::Search("szed".into()));
    assert!(t.visible().is_empty());
    t.update(Msg::Search("john smith".into()));
    assert_eq!(t.visible().len(), 1);
}

#[test]
fn unsaved_words_are_kept_as_typed_come_back_after_a_crash_and_the_watch_note_stays() {
    let store = folder();
    let mut t = text(store.clone());
    t.update(Msg::New);
    t.update(Msg::Triggers("zz#".into()));
    typed(&mut t, "kept");
    t.keep_unsaved();
    let mut again = text(store.clone());
    again.restore_unsaved();
    assert!(again.drafting && again.triggers == "zz#" && again.pane.borrow().doc.text() == "kept");
    assert!(!again.word && !again.case);
    t.update(Msg::Cancel);
    assert!(!store.backups.join("unsaved.txt").exists());
    t.handle(Do::Unwatched("not watched".into()));
    t.update(Msg::Pick(0));
    t.update(Msg::Save);
    assert_eq!(t.watch_note.as_deref(), Some("not watched"));
}

#[test]
fn unsaved_words_come_back_with_their_switches() {
    let store = folder();
    let mut t = text(store.clone());
    t.update(Msg::New);
    t.update(Msg::Triggers("zz#".into()));
    typed(&mut t, "kept");
    t.word = true;
    t.case = true;
    t.keep_unsaved();
    let mut again = text(store);
    again.restore_unsaved();
    assert!(again.word && again.case && again.triggers == "zz#" && again.pane.borrow().doc.text() == "kept");
}

#[test]
fn put_back_after_delete_keeps_everything_the_snippet_had() {
    let store = folder();
    let file = store.match_dir().join("adv.yml");
    let body = "matches:\n  - trigger: \":adv\"\n    replace: \"{{o}}\"\n    left_word: true\n    vars:\n      - name: o\n        type: echo\n        params:\n          echo: hi\n";
    fs::write(&file, body).unwrap();
    let (id, raw) = {
        let e = store.read_all().entries.into_iter().find(|e| e.triggers == vec![":adv".to_string()]).unwrap();
        (e.id, e.raw)
    };
    store.delete(&id, &raw).map(|written| store.restore("adv.yml", &written)).unwrap().unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap().trim_end(), body.trim_end());
}

#[test]
fn unsaved_words_are_written_after_typing_pauses_not_at_every_letter() {
    let store = folder();
    let events = Loop::new();
    let mut t = Snippets::new(store.clone(), Sizes::from(&Layers::new(CSS, "")), events.clone(), Some(events.post(|_: Do| {})), None, None);
    t.update(Msg::New);
    t.update(Msg::Triggers("zz#".into()));
    typed(&mut t, "kept");
    assert!(!store.backups.join("unsaved.txt").exists(), "written at a keystroke");
    t.handle(Do::KeepUnsaved);
    let kept = fs::read_to_string(store.backups.join("unsaved.txt")).unwrap();
    assert!(kept.contains("zz#") && kept.contains("kept"), "{kept}");
}
