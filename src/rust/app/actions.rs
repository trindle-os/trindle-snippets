//! What the window does when asked: make a file, save, delete, put back, choose files, bring in an import.

use super::*;

impl Snippets {

    pub(super) fn make_file(&mut self) {
        let Some(name) = self.naming.clone() else { return };
        match self.store.create_file(&name) {
            Ok(rel) => {
                self.reload();
                let files = self.writable_files();
                if let Some(i) = files.iter().position(|f| *f == rel) {
                    self.file = i;
                }
                self.naming = None;
                self.problems.file = None;
            }
            Err(why) => self.problems.file = Some(why),
        }
    }

    pub fn save(&mut self) {
        if self.locked() {
            return;
        }
        self.sure = false;
        self.problems = Problems::default();
        let fields = self.fields();
        let result = if self.drafting {
            let files = self.writable_files();
            let rel = files.get(self.file).cloned().unwrap_or_else(|| "base.yml".into());
            self.store.create(&rel, &fields).map(Some)
        } else {
            let Some((id, raw)) = self.opened.clone() else { return };
            self.store.update(&id, &raw, &fields).map(|()| Some(id))
        };
        match result {
            Ok(id) => {
                let added = self.drafting;
                self.drafting = false;
                self.read_listing();
                self.publish();
                self.select(id);
                self.done = Some(if added { "Snippet added." } else { "Snippet saved." }.into());
                self.keep_unsaved();
            }
            Err(p) => self.show_problem(p),
        }
    }

    pub fn delete(&mut self) {
        let Some((id, raw)) = self.opened.clone() else { return };
        if !self.sure {
            self.sure = true;
            self.done = Some("Press delete again to remove this snippet. Cancel keeps it.".into());
            return;
        }
        match self.store.delete(&id, &raw) {
            Ok(written) => {
                self.deleted = Some((parse_id(&id).map(|(rel, _)| rel).unwrap_or_default(), written));
                self.read_listing();
                self.publish();
                self.clear_form();
                self.done = Some("Snippet deleted. Press put back to undo.".into());
            }
            Err(p) => self.show_problem(p),
        }
    }

    pub(super) fn put_back(&mut self) {
        let Some((rel, written)) = self.deleted.clone() else { return };
        match self.store.restore(&rel, &written) {
            Ok(id) => {
                self.deleted = None;
                self.read_listing();
                self.publish();
                self.select(Some(id));
                self.done = Some("Snippet put back.".into());
            }
            Err(p) => self.show_problem(p),
        }
    }

    pub(super) fn choose(&mut self, purpose: Purpose) {
        self.problems = Problems::default();
        let (Some(bus), Some(asks)) = (self.bus.clone(), self.asks.clone()) else {
            self.problems.other = Some("Files cannot be chosen here.".into());
            return;
        };
        let home = trindle_common::files::home();
        self.listen = match purpose {
            Purpose::Import => {
                let filters = vec![("Snippet files and archives".into(), vec!["*.yml".into(), "*.yaml".into(), "*.tar.gz".into(), "*.tgz".into()])];
                chooser::open(&bus, &self.events, Ask { title: "Import snippets".into(), folder: home, name: None, filters }, move |p| {
                    asks.send(Do::Chosen(purpose, p));
                })
            }
            Purpose::Export => {
                let filters = vec![("Snippet archive".into(), vec!["*.tar.gz".into()])];
                let ask = Ask { title: "Export snippets".into(), folder: home, name: Some("my-snippets.tar.gz".into()), filters };
                chooser::save(&bus, &self.events, ask, move |p| {
                    asks.send(Do::Chosen(purpose, p));
                })
            }
        };
        if self.listen.is_none() {
            self.problems.other = Some("The window for choosing files could not be opened.".into());
        }
    }

    pub fn chosen_file(&mut self, purpose: Purpose, path: Option<PathBuf>) {
        self.listen = None;
        let Some(path) = path else {
            if chooser::failed() {
                self.problems.other = Some(chooser::did_not_answer().into());
            }
            return;
        };
        match purpose {
            Purpose::Export => match self.store.export(&path) {
                Ok(()) => self.done = Some("Snippets exported.".into()),
                Err(why) => self.problems.other = Some(why),
            },
            Purpose::Import => {
                self.done = Some("Reading the file.".into());
                match self.asks.clone() {
                    Some(asks) => {
                        std::thread::spawn(move || {
                            let _ = asks.send(Do::Read(read_import(&path)));
                        });
                    }
                    None => self.read(read_import(&path)),
                }
            }
        }
    }

    pub fn read(&mut self, found: Result<Incoming, String>) {
        match found {
            Err(why) => {
                self.done = None;
                self.problems.other = Some(why);
            }
            Ok(found) if found.risky.is_empty() => self.bring_in(found),
            Ok(found) => {
                self.done = None;
                self.pending = Some(found);
            }
        }
    }

    pub(super) fn bring_in(&mut self, found: Incoming) {
        match self.store.bring_in(found.files) {
            Ok(n) => {
                self.reload();
                self.done = Some(if n == 1 { "Imported 1 file.".into() } else { format!("Imported {n} files.") });
            }
            Err(why) => self.problems.other = Some(why),
        }
    }

    pub fn handle(&mut self, ask: Do) {
        match ask {
            Do::Reload => self.reload(),
            Do::Chosen(purpose, path) => self.chosen_file(purpose, path),
            Do::Read(found) => self.read(found),
            Do::Unwatched(why) => self.watch_note = Some(why),
            Do::Paste(text) => {
                let mut pane = self.pane.borrow_mut();
                if pane.editing && !self.locked() {
                    pane.paste(&text);
                }
            }
            Do::Restyle | Do::Raise => {}
        }
    }

    pub(super) fn edited(&mut self, did: Did) {
        match did {
            Did::Copy(s) => {
                if let Some(w) = self.window.upgrade() {
                    w.copy(&s);
                }
            }
            Did::Paste => {
                if let (Some(w), Some(asks)) = (self.window.upgrade(), self.asks.clone()) {
                    w.paste(move |t| {
                        asks.send(Do::Paste(t));
                    });
                }
            }
            Did::Edited => {
                self.problems.replace = None;
                self.done = None;
                self.keep_unsaved();
            }
            Did::Moved | Did::Nothing => {}
        }
    }

    pub(super) fn pick(&mut self, i: usize) {
        let id = self.visible().get(i).map(|e| e.id.clone());
        if id != self.selected && self.dirty() {
            self.problems.other = Some(UNSAVED.into());
            return;
        }
        self.done = None;
        self.select(id);
    }

    pub(super) fn new_snippet(&mut self) {
        if self.dirty() {
            self.problems.other = Some(UNSAVED.into());
        } else {
            self.start_draft();
        }
    }

    pub(super) fn set_triggers(&mut self, typed: String) {
        if !self.locked() {
            self.triggers = typed;
            self.problems.triggers = None;
            self.done = None;
            self.keep_unsaved();
        }
    }

    pub(super) fn typed(&mut self, input: &Input) {
        if self.locked() && !matches!(input, Input::Focus(_)) {
            return;
        }
        let did = self.pane.borrow_mut().key(input);
        if let Some(did) = did {
            self.edited(did);
        }
    }

    pub(super) fn pointed(&mut self, p: Pointer) {
        if let Pointer::Down(x, y) | Pointer::Move(x, y) | Pointer::Hover(x, y) | Pointer::Up(x, y) = p {
            let at = self.at.get();
            self.pane.borrow_mut().origin = (at.0 - x, at.1 - y);
        }
        let did = self.pane.borrow_mut().pointer(p);
        self.edited(did);
    }

    pub(super) fn name_file(&mut self, name: Option<String>) {
        self.naming = name;
        self.problems.file = None;
    }

    pub(super) fn cancel(&mut self) {
        self.done = None;
        if self.drafting {
            self.clear_form();
        } else {
            self.select(self.selected.clone());
        }
        self.keep_unsaved();
    }

    pub(super) fn copy(&mut self) {
        match self.copy_words() {
            Ok(text) => {
                if let Some(w) = self.window.upgrade() {
                    w.copy(&text);
                    self.done = Some("Copied. Paste it where you need it.".into());
                }
            }
            Err(why) => self.problems.other = Some(why.into()),
        }
    }
}
