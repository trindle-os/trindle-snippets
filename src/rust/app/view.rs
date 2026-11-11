//! The window's parts, drawn from the state, and the words and rows they show.

use super::*;

impl Snippets {
    pub(super) fn list_side(&self) -> Node<Msg> {
        let s = &self.sizes;
        let at = self.found_at();
        let visible: Vec<&Entry> = at.iter().map(|&i| &self.listing.entries[i]).collect();
        let several = self.listing.files.len() > 1;
        let chosen = self.selected.as_deref().and_then(|id| visible.iter().position(|e| e.id == id));
        let rows: Vec<Item> = at.iter().map(|&i| list_row(&self.listing.entries[i], several, &short(&self.flats[i], s.preview_letters), &self.query)).collect();
        let body = if visible.is_empty() {
            let words = if self.listing.entries.is_empty() { t!("snippets-list-empty") } else { t!("snippets-list-no-match") };
            column(vec![note(words), space()]).height(Length::Fill)
        } else {
            scroll("snippets", list(rows, chosen, Msg::Pick).row_height(theme::sheet().rich_row).key("snippet-list"))
        };
        let unread: Vec<Node<Msg>> = self
            .listing
            .files
            .iter()
            .filter_map(|f| f.error.as_ref().map(|why| note(t!("snippets-file-left-alone", file = f.rel.clone(), why = why.clone())).tone(Tone::Problem)))
            .collect();
        let mut parts = vec![
            row(vec![
                entry(self.query.clone(), t!("snippets-search-hint"), Msg::Search).key("search").width(Length::Fill).grab(self.search_turn),
                button(icon(NEW_ICON), t!("snippets-add-hint")).on_press(Msg::New),
            ])
            .gap(s.buttons_gap),
            note(if self.listing.entries.is_empty() { HELD.to_string() } else { self.count() }),
            body,
        ];
        parts.extend(unread);
        if let Some(found) = &self.pending {
            parts.push(
                note(t!("snippets-risky", files = found.risky.join(", "))).tone(Tone::Problem),
            );
            parts.push(row(vec![space(), button(icon(CANCEL_ICON), t!("snippets-import-nothing")).on_press(Msg::Leave), main_word(t!("snippets-bring-in")).on_press(Msg::BringIn)]).gap(s.buttons_gap));
        }
        parts.push(
            row(vec![
                button(icon(IMPORT_ICON), t!("snippets-import-hint")).on_press(Msg::Import),
                button(icon(EXPORT_ICON), t!("snippets-export-hint")).on_press(Msg::Export),
                button(icon(RELOAD_ICON), t!("snippets-reload-hint")).on_press(Msg::Reload),
            ])
            .gap(s.buttons_gap),
        );
        card(column(parts).gap(s.form_gap).padding(s.inside).height(Length::Fill)).width(Length::Fixed(s.list_width)).height(Length::Fill)
    }

    pub(super) fn form_side(&self) -> Node<Msg> {
        let s = &self.sizes;
        let entry_now = self.chosen();
        let (heading, badge) = match (self.drafting, entry_now) {
            (true, _) => (t!("snippets-heading-new"), None),
            (false, Some(e)) if e.read_only => (t!("snippets-heading-snippet"), Some(t!("snippets-badge-package"))),
            (false, Some(e)) if e.advanced.is_some() => (t!("snippets-heading-snippet"), Some(t!("snippets-badge-readonly"))),
            (false, Some(_)) => (t!("snippets-heading-edit"), None),
            (false, None) => ("Snippets", None),
        };
        let mut parts = vec![row(vec![title(heading), space(), note(badge.unwrap_or(HELD))]).gap(s.buttons_gap)];
        if !self.drafting && entry_now.is_none() {
            parts.push(note(t!("snippets-pick-or-add")));
            if self.deleted.is_some() {
                parts.push(row(vec![space(), main_word(t!("snippets-put-back")).on_press(Msg::PutBack)]));
            }
            parts.push(space());
            parts.push(self.said_line());
            return card(column(parts).gap(s.form_gap).padding(s.inside).height(Length::Fill)).height(Length::Fill);
        }
        let locked = self.locked();
        parts.push(form_label(t!("snippets-triggers")));
        parts.push(entry(self.triggers.clone(), ":tm", Msg::Triggers).key("triggers").width(Length::Fill).off(locked).on_submit(Msg::Save).grab(self.triggers_turn));
        parts.push(under(self.problems.triggers.as_deref(), t!("snippets-triggers-about")));
        match entry_now.and_then(|e| e.advanced.clone().map(|why| (e.raw.clone(), why))).filter(|_| !self.drafting) {
            Some((raw, why)) => {
                parts.push(form_label(t!("snippets-raw-label")));
                parts.push(card(scroll("raw", label(raw).width(Length::Fill))).height(Length::Fill));
                parts.push(note(why));
            }
            None => {
                parts.push(row(vec![form_label(t!("snippets-replacement")), space(), button(icon(COPY_ICON), t!("snippets-copy-replacement")).on_press(Msg::Copy)]));
                parts.push(card(self.replacement(locked)).sunk().height(Length::Fill));
                let wont = crate::expand::wont_expand(&split_triggers(&self.triggers), self.pane.borrow().doc.text());
                parts.push(under(self.problems.replace.as_deref(), wont.unwrap_or(HELD)));
            }
        }
        parts.push(row(vec![label(t!("snippets-whole-words")), space(), switch(self.word, t!("snippets-whole-words-hint"), Msg::Word).off(locked)]).gap(s.buttons_gap));
        parts.push(row(vec![label(t!("snippets-match-case")), space(), switch(self.case, t!("snippets-match-case-hint"), Msg::Case).off(locked)]).gap(s.buttons_gap));
        parts.push(self.file_row(entry_now));
        let can_delete = !self.drafting && entry_now.is_some_and(|e| !e.read_only);
        parts.push(
            row(vec![
                remove(icon(DELETE_ICON), t!("snippets-delete-hint")).key("delete").on_press(Msg::Delete).off(!can_delete),
                space(),
                button(icon(CANCEL_ICON), t!("snippets-cancel-hint")).on_press(Msg::Cancel),
                main_word(t!("snippets-save")).on_press(Msg::Save).off(locked),
            ])
            .gap(s.buttons_gap),
        );
        parts.push(self.said_line());
        card(column(parts).gap(s.form_gap).padding(s.inside).height(Length::Fill)).height(Length::Fill)
    }

    fn file_row(&self, entry_now: Option<&Entry>) -> Node<Msg> {
        let s = &self.sizes;
        match (&self.naming, self.drafting) {
            (Some(name), true) => column(vec![
                row(vec![
                    label(t!("snippets-new-file")),
                    entry(name.clone(), "my-snippets", Msg::FileName).key("file-name").width(Length::Fill).on_submit(Msg::MakeFile).grab(self.file_turn),
                    button(icon(CANCEL_ICON), t!("snippets-no-new-file")).on_press(Msg::NoFile),
                    main_word(t!("snippets-make")).on_press(Msg::MakeFile),
                ])
                .gap(s.buttons_gap),
                under(self.problems.file.as_deref(), t!("snippets-file-name-about")),
            ])
            .gap(s.form_gap),
            (_, true) => {
                let files = dropdown(self.writable_files(), Some(self.file), t!("snippets-file-it-goes-in"), Msg::File);
                row(vec![label(t!("snippets-file")), space(), button(icon(NEW_ICON), t!("snippets-make-new-file-hint")).on_press(Msg::NewFile), files]).gap(s.buttons_gap)
            }
            _ => row(vec![label(t!("snippets-file")), space(), label(entry_now.map(|e| e.file.clone()).unwrap_or_default())]).gap(s.buttons_gap),
        }
    }

    pub(super) fn said_line(&self) -> Node<Msg> {
        match (&self.problems.other, &self.done, &self.watch_note) {
            (Some(p), ..) => note(p.clone()).tone(Tone::Problem),
            (None, Some(d), _) => note(d.clone()),
            (None, None, Some(w)) => note(w.clone()).tone(Tone::Problem),
            (None, None, None) => note(HELD),
        }
    }

    pub(super) fn replacement(&self, locked: bool) -> Node<Msg> {
        let paint = self.pane.clone();
        let place = self.pane.clone();
        let least = self.sizes.replacement_least;
        drawing((0.0, least), t!("snippets-replacement"), move |canvas, pen| paint.borrow_mut().paint(canvas, pen))
            .key("replacement")
            .menu(move |_, _| {
                let key = |sym| Msg::Key(Input::Key { sym, text: None, mods: Mods { ctrl: true, ..Mods::default() }, repeat: false });
                vec![
                    MenuRow::send("edit-cut-symbolic", t!("snippets-cut"), key(Keysym::x)).keys("Ctrl+X"),
                    MenuRow::send("edit-copy-symbolic", t!("snippets-copy"), key(Keysym::c)).keys("Ctrl+C"),
                    MenuRow::send("edit-paste-symbolic", t!("snippets-paste"), key(Keysym::v)).keys("Ctrl+V"),
                    MenuRow::send("edit-select-all-symbolic", t!("snippets-select-all"), key(Keysym::a)).keys("Ctrl+A"),
                ]
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .on_pointer(Msg::Pointer)
            .on_hover()
            .on_keys(move |input| match input {
                Input::Key { sym, mods, .. } => {
                    let leave = *sym == Keysym::Escape || *sym == Keysym::ISO_Left_Tab || (*sym == Keysym::Tab && (mods.ctrl || mods.alt));
                    (!leave).then(|| Msg::Key(input.clone()))
                }
                Input::Composed { .. } | Input::Focus(_) => Some(Msg::Key(input.clone())),
                _ => None,
            })
            .cursor_with(move |x, _| if x < place.borrow().text_left() { Cursor::Arrow } else { Cursor::Text })
            .off(locked)
    }
}

pub(super) fn under(problem: Option<&str>, otherwise: &str) -> Node<Msg> {
    match problem {
        Some(p) => note(p.to_string()).tone(Tone::Problem),
        None => note(otherwise.to_string()),
    }
}

pub fn short(flat: &str, most: usize) -> String {
    if flat.chars().nth(most).is_some() {
        format!("{}\u{2026}", flat.chars().take(most.saturating_sub(1)).collect::<String>())
    } else {
        flat.to_string()
    }
}

pub fn preview(e: &Entry) -> String {
    match &e.replace {
        Some(r) => r.clone(),
        None => e.raw.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join(" "),
    }
}

pub fn file_short(rel: &str) -> String {
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() > 2 && parts[0] == "packages" {
        return parts[1].to_string();
    }
    let name = parts.last().copied().unwrap_or(rel);
    name.strip_suffix(".yml").or_else(|| name.strip_suffix(".yaml")).unwrap_or(name).to_string()
}

pub(super) fn found(text: &str, q: &str) -> Vec<std::ops::Range<usize>> {
    let q: Vec<char> = q.chars().flat_map(char::to_lowercase).collect();
    let mut out = Vec::new();
    if q.is_empty() {
        return out;
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let mut j = i;
        let mut k = 0;
        while j < chars.len() && k < q.len() {
            let mut low = chars[j].1.to_lowercase();
            if low.any(|c| { let hit = q.get(k) == Some(&c); k += 1; !hit }) {
                break;
            }
            j += 1;
        }
        if k == q.len() && j > i {
            out.push(chars[i].0..chars.get(j).map_or(text.len(), |c| c.0));
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

pub fn list_row(e: &Entry, several: bool, line: &str, query: &str) -> Item {
    let mut marks = Vec::new();
    if e.triggers.len() > 1 {
        marks.push(t!("snippets-more", count = e.triggers.len() - 1));
    }
    if e.advanced.is_some() {
        marks.push(t!("snippets-mark-advanced").into());
    }
    if e.read_only {
        marks.push(t!("snippets-mark-package").into());
    }
    if several {
        marks.push(file_short(&e.file));
    }
    let q = query.trim();
    let top = e.triggers.first().cloned().unwrap_or_else(|| t!("snippets-no-trigger").into());
    let (top_marks, line_marks) = (found(&top, q), found(line, q));
    let mut row = item(top).marked(Field::Text, &top_marks).under(line.to_string()).marked(Field::Under, &line_marks);
    if !marks.is_empty() {
        row = row.detail(marks.join(", "));
    }
    row
}
