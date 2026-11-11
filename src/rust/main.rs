//! Snippets: Trindle's snippet keeper. It lists the text snippets in the

#[macro_use]
mod words;
mod app;
mod expand;
mod sheet;
mod snippets;
mod yaml;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::{Rc, Weak};
use std::time::Duration;

use toolkit::toolkit::bus::{Bus, Served};
use toolkit::toolkit::once;
use toolkit::toolkit::event::{Flow, Loop, Post};
use toolkit::toolkit::ui::Shown;
use toolkit::toolkit::watch::Watch;
use toolkit::toolkit::window::{Display, Kind};

use app::{Do, Snippets};

const APP_ID: &str = "org.trindle.Snippets";
const PATH: &str = "/org/trindle/Snippets";

const SETTLE: Duration = Duration::from_millis(400);

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("Snippets {}", app::VERSION);
        return ExitCode::SUCCESS;
    }
    let Some(store) = snippets::Store::usual() else {
        eprintln!("snippets: HOME is not set to a full path, so there is nowhere to keep snippets");
        return ExitCode::FAILURE;
    };
    if let Some(code) = expansion_switch(&store) {
        return code;
    }
    run(store)
}

fn expansion_switch(store: &snippets::Store) -> Option<ExitCode> {
    match switch_asked(std::env::args().skip(1))? {
        Some(on) => Some(switch_expansion(store, on)),
        None => {
            eprintln!("snippets: --expand needs on or off");
            Some(ExitCode::FAILURE)
        }
    }
}

struct Held {
    shown: Rc<Shown<Snippets>>,
    _display: Rc<Display>,
    _raise: Option<Served>,
}

fn run(store: snippets::Store) -> ExitCode {
    let skeleton = store.ensure_skeleton();
    let events = Loop::new();
    let gone = events.clone();
    let Some(display) = Display::connect(&events, move || gone.quit()) else {
        eprintln!("snippets: there is no screen to show a window on");
        return ExitCode::FAILURE;
    };
    let bus = Bus::session(&events).ok();
    let (held, failed) = (Rc::new(RefCell::new(None)), Rc::new(Cell::new(false)));
    let (ev, b, h, f) = (events.clone(), bus.clone(), held.clone(), failed.clone());
    let first = move || match open(&store, &ev, display, &b, skeleton) {
        Some(opened) => *h.borrow_mut() = Some(opened),
        None => {
            f.set(true);
            ev.quit();
        }
    };
    let quit = events.clone();
    match &bus {
        Some(bus) => once::claim(bus, &events, APP_ID, PATH, first, move || quit.quit()),
        None => first(),
    }
    events.run();
    if let Some(held) = held.borrow().as_ref() {
        held.shown.change(|t| t.keep_unsaved());
    }
    if failed.get() { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

fn open(store: &snippets::Store, events: &Loop, display: Rc<Display>, bus: &Option<Bus>, skeleton: Result<(), String>) -> Option<Held> {
    let slot: Rc<RefCell<Weak<Shown<Snippets>>>> = Rc::new(RefCell::new(Weak::new()));
    let asks = route_asks(events, &slot);
    let (text, size) = new_window(store, events, &asks, bus, skeleton);
    let kind = Kind::Window { title: "Snippets".into(), app_id: APP_ID.into() };
    let Some(shown) = Shown::open(events, &display, kind, size, text) else {
        eprintln!("snippets: the window could not be opened");
        return None;
    };
    let shown = Rc::new(shown);
    wire(events, &shown, &slot, &asks, store);
    let raise = bus.as_ref().and_then(|b| once::serve_window(b, PATH, &shown.window).ok());
    Some(Held { shown, _display: display, _raise: raise })
}

fn wire(events: &Loop, shown: &Rc<Shown<Snippets>>, slot: &Rc<RefCell<Weak<Shown<Snippets>>>>, asks: &Post<Do>, store: &snippets::Store) {
    *slot.borrow_mut() = Rc::downgrade(shown);
    let w = Rc::downgrade(&shown.window);
    shown.change(|t| t.window = w);
    let quit = events.clone();
    shown.window.on_close(move || quit.quit());
    quit_on_signals(events);
    if !watch_snippets(events, store.match_dir(), asks.clone()) {
        shown.change(|t| t.watch_note = Some(not_watched().into()));
    }
    watch_look(events, asks.clone());
}

fn route_asks(events: &Loop, slot: &Rc<RefCell<Weak<Shown<Snippets>>>>) -> Post<Do> {
    let s = slot.clone();
    events.post(move |ask: Do| {
        let Some(shown) = s.borrow().upgrade() else { return };
        match ask {
            Do::Restyle => shown.restyle(),
            ask => shown.change(|t| t.handle(ask)),
        }
    })
}

fn new_window(store: &snippets::Store, events: &Loop, asks: &Post<Do>, bus: &Option<Bus>, skeleton: Result<(), String>) -> (Snippets, (u32, u32)) {
    let sizes = sheet::Sizes::read();
    let size = sizes.window;
    let mut text = Snippets::new(store.clone(), sizes, events.clone(), Some(asks.clone()), bus.clone(), trindle_common::files::home());
    text.restore_unsaved();
    if let Err(why) = skeleton {
        text.problems.other = Some(why);
    }
    (text, size)
}

fn quit_on_signals(events: &Loop) {
    for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        let quit = events.clone();
        events.on_signal(signal, move || {
            quit.quit();
            Flow::Stop
        });
    }
}

fn switch_asked(mut args: impl Iterator<Item = String>) -> Option<Option<bool>> {
    if args.next().as_deref() != Some("--expand") {
        return None;
    }
    Some(match args.next().as_deref() {
        Some("on") => Some(true),
        Some("off") => Some(false),
        _ => None,
    })
}

fn switch_expansion(store: &snippets::Store, on: bool) -> ExitCode {
    let Some(home) = trindle_common::files::home() else { return ExitCode::FAILURE };
    let done = expand::set_on(&home, on).and_then(|()| expand::publish(&store.read_all().entries, &home));
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("snippets: {why}");
            ExitCode::FAILURE
        }
    }
}

const FOLDERS_MOST: usize = 256;

fn folders(root: &Path) -> (Vec<PathBuf>, bool) {
    let mut out = vec![root.to_path_buf()];
    let mut i = 0;
    while i < out.len() && out.len() < FOLDERS_MOST {
        if let Ok(read) = std::fs::read_dir(&out[i]) {
            out.extend(read.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()));
        }
        i += 1;
    }
    let more = i < out.len() || out.len() > FOLDERS_MOST;
    out.truncate(FOLDERS_MOST);
    (out, more)
}

fn watch_all(watch: &mut Watch, root: &Path) -> bool {
    let (folders, more) = folders(root);
    folders.iter().fold(!more, |all, f| watch.folder(f).is_ok() && all)
}

fn not_watched() -> &'static str {
    t!("snippets-not-watched")
}

fn watch_snippets(events: &Loop, root: PathBuf, asks: Post<Do>) -> bool {
    let Ok(mut watch) = Watch::new() else { return false };
    let complete = watch_all(&mut watch, &root);
    let watch = Rc::new(RefCell::new(watch));
    let waiting = Rc::new(Cell::new(false));
    let ev = events.clone();
    let fd = watch.borrow().fd();
    events.watch(fd, move |hung_up| {
        if hung_up {
            asks.send(Do::Unwatched(not_watched().into()));
            return Flow::Stop;
        }
        let changed = watch.borrow().changed();
        if changed.is_empty() || waiting.replace(true) {
            return Flow::Continue;
        }
        let (asks, waiting, watch, root) = (asks.clone(), waiting.clone(), watch.clone(), root.clone());
        ev.after(SETTLE, move || {
            waiting.set(false);

            if !watch_all(&mut watch.borrow_mut(), &root) {
                asks.send(Do::Unwatched(not_watched().into()));
            }
            asks.send(Do::Reload);
        });
        Flow::Continue
    });
    complete
}

fn watch_look(events: &Loop, asks: Post<Do>) {
    let Ok(mut watch) = Watch::new() else { return };
    let folder = toolkit::common::files::trindle_config();
    if watch.folder(&folder).is_err() {
        return;
    }
    events.watch(watch.fd(), move |hung_up| {
        if hung_up {
            return Flow::Stop;
        }
        if !watch.changed().is_empty() && !asks.send(Do::Restyle) {
            return Flow::Stop;
        }
        Flow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_corner_or_button_size_is_written_here() {
        let mut files = vec![];
        let mut dirs = vec![std::path::PathBuf::from("src/rust"), "toolkit-link/src".into()];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() { dirs.push(p) } else if p.extension().is_some_and(|x| x == "rs") { files.push(p) }
            }
        }
        let banned = ["panel_radius", "control_radius", "row_radius", "float_radius", "ring_corner", "border-radius", "button-height", "rounded_rect", "RoundRect"];
        for f in files.iter().filter(|f| !f.ends_with("main.rs")) {
            let text = std::fs::read_to_string(f).unwrap();
            for b in banned {
                assert!(!text.contains(b), "{} writes {b}; the sheet decides it", f.display());
            }
        }
    }

    fn asked(words: &[&str]) -> Option<Option<bool>> {
        switch_asked(words.iter().map(|w| w.to_string()))
    }

    #[test]
    fn the_expand_switch_takes_only_on_or_off() {
        assert_eq!(asked(&[]), None);
        assert_eq!(asked(&["--other"]), None);
        assert_eq!(asked(&["--expand", "on"]), Some(Some(true)));
        assert_eq!(asked(&["--expand", "off"]), Some(Some(false)));
        assert_eq!(asked(&["--expand"]), Some(None));
        assert_eq!(asked(&["--expand", "ON"]), Some(None));
    }

    #[test]
    fn a_folder_tree_over_the_limit_is_cut_and_said_to_be() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("watched-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for n in 0..300 {
            std::fs::create_dir_all(root.join(format!("d{n}"))).unwrap();
        }
        let (all, more) = folders(&root);
        assert!(more && all.len() == FOLDERS_MOST);
        std::fs::remove_dir_all(&root).unwrap();
        let small = root.with_file_name(format!("watched-small-{}", std::process::id()));
        std::fs::create_dir_all(small.join("a/b")).unwrap();
        let (few, more) = folders(&small);
        std::fs::remove_dir_all(&small).unwrap();
        assert!(!more && few.len() == 3);
        let exact = root.with_file_name(format!("watched-exact-{}", std::process::id()));
        for n in 0..FOLDERS_MOST - 1 {
            std::fs::create_dir_all(exact.join(format!("d{n}"))).unwrap();
        }
        let (all, more) = folders(&exact);
        std::fs::remove_dir_all(&exact).unwrap();
        assert!(more && all.len() == FOLDERS_MOST, "folders below the last one are not scanned, so more is said");
    }
}
