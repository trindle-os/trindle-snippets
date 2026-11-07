//! Snippets's window: the list and search on the left, the chosen snippet's form on the right. Problems sit under their box; what was done is said in a quiet line that keeps its place.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use toolkit::pane::doc::Doc;
use toolkit::pane::editor::{Did, Look, Pane};
use toolkit::toolkit::bus::{Bus, Listen};
use toolkit::toolkit::chooser::{self, Ask};
use toolkit::toolkit::event::{Loop, Post};
use toolkit::toolkit::input::{Cursor, Input, Keysym, Mods, Typing};
use toolkit::toolkit::theme;
use toolkit::toolkit::ui::{App, Keyboard};
use toolkit::toolkit::widget::{
    button, Field, card, column, drawing, dropdown, entry, form_label, item, label, list, main_word, note, remove, row, scroll, space, switch, title,
    Item, Length, MenuRow, Node, Pointer, Tone,
};
use toolkit::toolkit::window::Window;

mod actions;
mod view;

use view::preview;

use crate::sheet::Sizes;
use crate::snippets::{parse_id, read_import, split_triggers, Entry, Fields, Incoming, Listing, Problem, Store};

const UNSAVED: &str = "You have changes that are not saved. Save them, or press Cancel to leave them.";

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const NEW_ICON: &str = "list-add-symbolic";
const COPY_ICON: &str = "edit-copy-symbolic";
const DELETE_ICON: &str = "user-trash-symbolic";
const CANCEL_ICON: &str = "window-close-symbolic";
const IMPORT_ICON: &str = "document-open-symbolic";
const EXPORT_ICON: &str = "send-to-symbolic";
const RELOAD_ICON: &str = "view-refresh-symbolic";

fn icon(name: &str) -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent()?.parent().map(|plugin| plugin.join("share/icons").join(format!("{name}.svg"))))
        .filter(|path| path.is_file())
        .map_or_else(|| name.to_string(), |path| path.to_string_lossy().into_owned())
}

const HELD: &str = "\u{a0}";

const NOT_HERE: &str = "Type a trigger such as :tm in any program to expand it, once Snippets is allowed to expand your short codes in its permissions. Or find a snippet and copy it.";

#[derive(Clone, Debug)]
pub enum Msg {
    Search(String),
    FocusSearch,
    Pick(usize),
    New,
    Triggers(String),
    Key(Input),
    Pointer(Pointer),
    Scroll(f32, f32, bool),
    Word(bool),
    Case(bool),
    File(usize),
    NewFile,
    FileName(String),
    MakeFile,
    NoFile,
    Save,
    Cancel,
    Delete,
    PutBack,
    Copy,
    Import,
    Export,
    Reload,
    BringIn,
    Leave,
}

pub enum Do {

    Reload,
    Chosen(Purpose, Option<PathBuf>),
    Paste(String),
    Restyle,

    Raise,

    Unwatched(String),

    Read(Result<Incoming, String>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Purpose {
    Import,
    Export,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Problems {
    pub triggers: Option<String>,
    pub replace: Option<String>,
    pub other: Option<String>,
    pub file: Option<String>,
}

pub struct Snippets {
    pub store: Store,
    pub listing: Listing,
    haystacks: Vec<String>,
    flats: Vec<String>,
    pub query: String,

    pub selected: Option<String>,

    opened: Option<(String, String)>,

    baseline: Fields,

    sure: bool,

    pending: Option<Incoming>,

    deleted: Option<(String, String)>,

    pub drafting: bool,
    pub triggers: String,
    pub pane: Rc<RefCell<Pane>>,
    pub word: bool,
    pub case: bool,

    pub file: usize,

    pub naming: Option<String>,
    pub problems: Problems,

    pub done: Option<String>,
    pub sizes: Sizes,

    pub publish_to: Option<PathBuf>,

    pub watch_note: Option<String>,
    pub window: Weak<Window>,
    events: Loop,
    asks: Option<Post<Do>>,
    bus: Option<Bus>,
    listen: Option<Listen>,

    at: Rc<Cell<(f32, f32)>>,

    search_turn: u32,
    triggers_turn: u32,
    file_turn: u32,
}

fn new_pane(text: &str, sizes: &Sizes) -> Rc<RefCell<Pane>> {
    let look = Look { font: None, size: sizes.replacement_size, wrap: true, numbers: false };
    Rc::new(RefCell::new(Pane::new(Doc::new(text.to_string()), look)))
}

impl Snippets {
    pub fn new(store: Store, sizes: Sizes, events: Loop, asks: Option<Post<Do>>, bus: Option<Bus>) -> Snippets {
        let mut t = Snippets {
            listing: Listing::default(),
            haystacks: Vec::new(),
            flats: Vec::new(),
            query: String::new(),
            selected: None,
            opened: None,
            baseline: Fields::default(),
            sure: false,
            pending: None,
            deleted: None,
            drafting: false,
            triggers: String::new(),
            pane: new_pane("", &sizes),
            word: false,
            case: false,
            file: 0,
            naming: None,
            problems: Problems::default(),
            done: None,
            publish_to: None,
            watch_note: None,
            window: Weak::new(),
            events,
            asks,
            bus,
            listen: None,
            at: Rc::new(Cell::new((0.0, 0.0))),
            search_turn: 1,
            triggers_turn: 0,
            file_turn: 0,
            store,
            sizes,
        };
        t.reload();
        t
    }

    fn read_listing(&mut self) {
        self.listing = self.store.read_all();
        self.haystacks = self.listing.entries.iter().map(|e| {
            let all = e.triggers.iter().map(String::as_str).chain(e.replace.as_deref()).chain(e.label.as_deref());
            all.collect::<Vec<_>>().join("\u{1}").to_lowercase()
        }).collect();
        self.flats = self.listing.entries.iter().map(|e| preview(e).split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    }

    pub fn reload(&mut self) {
        self.read_listing();
        self.publish();
        if let Some(id) = &self.selected {
            if self.entry(id).is_none() && !self.drafting {
                if self.dirty() {
                    self.selected = None;
                    self.opened = None;
                    self.drafting = true;
                    self.problems.other = Some("That snippet was removed from its file. Your changes are kept; Save adds it again.".into());
                } else {
                    self.clear_form();
                }
            }
        }
        self.file = self.file.min(self.writable_files().len().saturating_sub(1));
    }

    fn publish(&mut self) {
        let Some(folder) = &self.publish_to else { return };
        if let Err(why) = crate::expand::publish(&self.listing.entries, folder) {
            self.problems.other = Some(format!("Typing expansion could not be brought up to date: {why}."));
        }
    }

    fn entry(&self, id: &str) -> Option<&Entry> {
        self.listing.entries.iter().find(|e| e.id == id)
    }

    fn chosen(&self) -> Option<&Entry> {
        self.selected.as_deref().and_then(|id| self.entry(id))
    }

    pub fn writable_files(&self) -> Vec<String> {
        let files: Vec<String> = self.listing.files.iter().filter(|f| !f.read_only && f.error.is_none()).map(|f| f.rel.clone()).collect();
        if files.is_empty() { vec!["base.yml".into()] } else { files }
    }

    fn found_at(&self) -> Vec<usize> {
        let q = self.query.trim().to_lowercase();
        (0..self.listing.entries.len()).filter(|&i| q.is_empty() || self.haystacks[i].contains(&q)).collect()
    }

    pub fn visible(&self) -> Vec<&Entry> {
        self.found_at().into_iter().map(|i| &self.listing.entries[i]).collect()
    }

    pub fn count(&self) -> String {
        let (shown, all) = (self.visible().len(), self.listing.entries.len());
        match (all, shown == all) {
            (0, _) => String::new(),
            (1, true) => "1 snippet".into(),
            (_, true) => format!("{all} snippets"),
            _ => format!("{shown} of {all} snippets"),
        }
    }

    fn locked(&self) -> bool {
        !self.drafting && self.chosen().is_none_or(|e| e.read_only || e.advanced.is_some())
    }

    fn dirty(&self) -> bool {
        let now = self.fields();
        if self.drafting { !now.triggers.is_empty() || !now.replace.is_empty() } else { self.opened.is_some() && now != self.baseline }
    }

    pub fn keep_unsaved(&self) {
        let file = self.store.backups.join("unsaved.txt");
        if !self.dirty() {
            let _ = std::fs::remove_file(file);
            return;
        }
        let f = self.fields();
        let switches = format!("{}{}", if f.word { 'w' } else { '-' }, if f.propagate_case { 'c' } else { '-' });
        let text = format!("{}\n{switches}\n\n{}\n", f.triggers.join(", "), f.replace);
        let _ = std::fs::create_dir_all(&self.store.backups);
        let _ = toolkit::common::files::write_whole_private(&file, text.as_bytes());
    }

    pub fn restore_unsaved(&mut self) {
        let Ok(text) = std::fs::read_to_string(self.store.backups.join("unsaved.txt")) else { return };
        let Some((head, replace)) = text.split_once("\n\n") else { return };
        let (triggers, switches) = head.split_once('\n').unwrap_or((head, ""));
        self.start_draft();
        self.triggers = triggers.to_string();
        (self.word, self.case) = (switches.starts_with('w'), switches.ends_with('c'));
        self.pane = new_pane(replace.strip_suffix('\n').unwrap_or(replace), &self.sizes);
        self.done = Some("The words you had not saved last time are back.".into());
    }

    fn clear_form(&mut self) {
        self.selected = None;
        self.opened = None;
        self.sure = false;
        self.baseline = Fields::default();
        self.drafting = false;
        self.triggers.clear();
        self.pane = new_pane("", &self.sizes);
        self.word = false;
        self.case = false;
        self.problems = Problems::default();
        self.naming = None;
    }

    fn select(&mut self, id: Option<String>) {
        self.clear_form();
        let Some(entry) = id.as_deref().and_then(|id| self.entry(id)).cloned() else { return };
        self.triggers = entry.triggers.join(", ");
        self.pane = new_pane(entry.replace.as_deref().unwrap_or(""), &self.sizes);
        self.word = entry.word;
        self.case = entry.propagate_case;
        self.opened = Some((entry.id.clone(), entry.raw));
        self.baseline = self.fields();
        self.selected = Some(entry.id);
    }

    fn start_draft(&mut self) {
        self.clear_form();
        self.drafting = true;
        self.done = None;
        self.triggers_turn = self.triggers_turn.wrapping_add(1);
    }

    pub fn copy_words(&self) -> Result<String, &'static str> {
        if self.selected.as_deref().and_then(|id| self.entry(id)).is_some_and(|e| e.advanced.is_some()) {
            return Err("This snippet uses variables, forms or scripts, which only work where typing expands. There is no plain text to copy.");
        }
        let text = self.pane.borrow().doc.text().to_string();
        if text.is_empty() { Err("There is nothing in the replacement to copy.") } else { Ok(text) }
    }

    pub fn fields(&self) -> Fields {
        Fields { triggers: split_triggers(&self.triggers), replace: self.pane.borrow().doc.text().to_string(), word: self.word, propagate_case: self.case }
    }

    fn show_problem(&mut self, p: Problem) {
        self.problems = Problems::default();
        match p {
            Problem::Triggers(w) => self.problems.triggers = Some(w.into()),
            Problem::Replace(w) => self.problems.replace = Some(w.into()),
            Problem::Other(w) => self.problems.other = Some(w),
        }
    }
}

impl App for Snippets {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Search(q) => self.query = q,
            Msg::FocusSearch => self.search_turn = self.search_turn.wrapping_add(1),
            Msg::Pick(i) => self.pick(i),
            Msg::New => self.new_snippet(),
            Msg::Triggers(t) => self.set_triggers(t),
            Msg::Key(input) => self.typed(&input),
            Msg::Pointer(p) => self.pointed(p),
            Msg::Scroll(x, y, steps) => self.pane.borrow_mut().scroll(x, y, steps),
            Msg::Word(on) => {
                self.word = on;
                self.done = None;
            }
            Msg::Case(on) => {
                self.case = on;
                self.done = None;
            }
            Msg::File(i) => self.file = i,
            Msg::NewFile => {
                self.name_file(Some(String::new()));
                self.file_turn = self.file_turn.wrapping_add(1);
            }
            Msg::FileName(n) => self.name_file(Some(n)),
            Msg::NoFile => self.name_file(None),
            Msg::MakeFile => self.make_file(),
            Msg::Save => self.save(),
            Msg::Cancel => self.cancel(),
            Msg::Delete => self.delete(),
            Msg::PutBack => self.put_back(),
            Msg::Copy => self.copy(),
            Msg::Import => self.choose(Purpose::Import),
            Msg::BringIn => {
                if let Some(found) = self.pending.take() {
                    self.bring_in(found);
                }
            }
            Msg::Leave => {
                self.pending = None;
                self.done = Some("Nothing was imported.".into());
            }
            Msg::Export => self.choose(Purpose::Export),
            Msg::Reload => {
                self.reload();
                self.done = Some("Read again.".into());
            }
        }
    }

    fn view(&self) -> Node<Msg> {
        let s = &self.sizes;
        column(vec![
            row(vec![self.list_side(), self.form_side()]).gap(s.between).height(Length::Fill),
            row(vec![note(NOT_HERE), space(), note(format!("Snippets {VERSION}"))]).gap(s.buttons_gap),
        ])
        .gap(s.between)
        .padding(s.margin)
        .height(Length::Fill)
    }

    fn typing(&self) -> Option<Typing> {
        let pane = self.pane.borrow();
        if !pane.editing || self.locked() {
            return None;
        }
        let doc = &pane.doc;
        let line = doc.line_range(doc.line_of(doc.caret));
        let anchor = doc.anchor.clamp(line.start, line.end);
        let (x, y, w, h) = pane.caret_box;
        let (ox, oy) = pane.origin;
        Some(Typing {
            id: "replacement".into(),
            text: doc.text()[line.clone()].to_string(),
            caret: doc.caret - line.start,
            anchor: anchor - line.start,
            area: ((ox + x) as i32, (oy + y) as i32, w.ceil() as i32, h.ceil() as i32),
            secret: false,
            purpose: toolkit::toolkit::input::Purpose::Words,
        })
    }

    fn intercept_with(&self, input: &Input, at: Keyboard) -> Option<Msg> {
        match input {
            Input::PointerIn { x, y } | Input::PointerMove { x, y } | Input::Press { x, y, .. } => {
                self.at.set((*x, *y));
                None
            }
            Input::Scroll { x, y, steps } if self.pane.borrow().over => Some(Msg::Scroll(*x, *y, *steps)),
            Input::Key { sym, mods, .. } => {
                if mods.ctrl && !mods.alt && !mods.logo {
                    return match *sym {
                        Keysym::s | Keysym::S => Some(Msg::Save),
                        Keysym::n | Keysym::N => Some(Msg::New),
                        Keysym::f | Keysym::F => Some(Msg::FocusSearch),
                        _ => None,
                    };
                }
                let typing = at.text || self.pane.borrow().editing;
                (*sym == Keysym::slash && !typing && !mods.ctrl && !mods.alt).then_some(Msg::FocusSearch)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
