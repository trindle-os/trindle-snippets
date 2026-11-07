//! The snippet folder: edits touch only the lines of the values changed, keep a copy first, and are read back to prove nothing else moved.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use toolkit::common::files;
use trindle_common::files as shared;

use crate::yaml::{self, Doc, Item, List, Value};

mod edit;
mod import;

use edit::*;
pub use import::{free_name, read_import, Incoming};
use import::*;

const SIMPLE: &[&str] = &["trigger", "triggers", "replace", "word", "left_word", "right_word", "propagate_case", "uppercase_style", "label"];

const BACKUPS_KEPT: usize = 20;

const TRIGGER_MOST: usize = 100;
const REPLACE_MOST: usize = 200_000;

const DEFAULT_CONFIG: &str = "# Settings for typing snippets, kept beside Snippets's snippets.\n# Anything set here applies to every program unless a more specific\n# file in this folder says otherwise.\n";
pub const DEFAULT_MATCHES: &str = "# Your text snippets live here.\n# Snippets edits this file for you, and leaves your comments alone.\n\nmatches:\n  - trigger: \":tm\"\n    replace: \"Typed by Snippets\"\n";

#[derive(Clone, Debug, PartialEq)]
pub struct FileInfo {

    pub rel: String,
    pub read_only: bool,

    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {

    pub id: String,
    pub file: String,
    pub index: usize,
    pub read_only: bool,

    pub advanced: Option<String>,
    pub triggers: Vec<String>,
    pub label: Option<String>,
    pub replace: Option<String>,
    pub word: bool,
    pub propagate_case: bool,

    pub picky: bool,

    pub raw: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listing {
    pub files: Vec<FileInfo>,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fields {
    pub triggers: Vec<String>,
    pub replace: String,
    pub word: bool,
    pub propagate_case: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    Triggers(&'static str),
    Replace(&'static str),
    Other(String),
}

impl Problem {
    pub fn words(&self) -> &str {
        match self {
            Problem::Triggers(w) | Problem::Replace(w) => w,
            Problem::Other(w) => w,
        }
    }
}

fn other(words: impl Into<String>) -> Problem {
    Problem::Other(words.into())
}

#[derive(Clone, Debug)]
pub struct Store {
    pub config: PathBuf,
    pub backups: PathBuf,
}

impl Store {

    pub fn usual() -> Option<Store> {
        let legacy = shared::home()?.join(".espanso");
        let config = if legacy.join("match").is_dir() { legacy } else { shared::config_home()?.join("espanso") };
        Some(Store { config, backups: shared::data_home()?.join("text").join("backups") })
    }

    pub fn match_dir(&self) -> PathBuf {
        self.config.join("match")
    }

    pub fn ensure_skeleton(&self) -> Result<(), String> {
        let make = |p: &Path| fs::create_dir_all(p).map_err(|e| format!("{} could not be made: {e}", p.display()));
        make(&self.config.join("config"))?;
        make(&self.match_dir())?;
        for (path, words) in [(self.config.join("config").join("default.yml"), DEFAULT_CONFIG), (self.match_dir().join("base.yml"), DEFAULT_MATCHES)] {
            if fs::symlink_metadata(&path).is_err() {
                write(&path, words).map_err(|e| e.words().to_string())?;
            }
        }
        Ok(())
    }

    pub fn files(&self) -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
            let Ok(read) = fs::read_dir(dir) else { return };
            let mut names: Vec<_> = read.flatten().collect();
            names.sort_by_key(|e| e.file_name());
            for e in names {
                let path = e.path();
                let Ok(kind) = e.file_type() else { continue };
                if kind.is_dir() && depth < 8 {
                    walk(&path, out, depth + 1);
                } else if (kind.is_file() || kind.is_symlink()) && is_yaml(&path) {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.match_dir(), &mut out, 0);
        out
    }

    pub fn read_all(&self) -> Listing {
        let root = self.match_dir();
        let mut listing = Listing::default();
        for path in self.files() {
            let rel = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy().into_owned();
            let read_only = is_package(&rel) || is_link(&path);
            match read_small(&path).and_then(|t| yaml::read(&t)) {
                Ok(doc) => {
                    let entries = entries_of(&doc, &rel, read_only);
                    listing.files.push(FileInfo { rel, read_only, error: None });
                    listing.entries.extend(entries);
                }
                Err(why) => listing.files.push(FileInfo { rel, read_only: true, error: Some(why) }),
            }
        }
        listing
    }

    fn writable(&self, rel: &str) -> Result<(PathBuf, Doc), Problem> {
        let path = inside(&self.match_dir(), rel).ok_or_else(|| other("That is not a file Snippets can write to."))?;
        if is_package(rel) {
            return Err(other("This came from an installed package, so it is read-only here."));
        }
        if is_link(&path) {
            return Err(other("This file is a link to somewhere else, so it is read-only here."));
        }
        let text = read_small(&path).map_err(Problem::Other)?;
        let doc = yaml::read(&text).map_err(|why| other(format!("{rel} could not be read. {why}")))?;
        Ok((path, doc))
    }

    pub fn create(&self, rel: &str, fields: &Fields) -> Result<String, Problem> {
        let fields = check(fields)?;
        let (path, doc) = self.writable(rel)?;
        let index = doc.items().len();

        let text = ["\n\n", "\n"]
            .into_iter()
            .map(|gap| line_ends(&doc.text, add_item(&doc, &fields, gap)))
            .find(|text| yaml::read(text).is_ok_and(|after| same_except(&doc, &after, None, Some(index)).is_ok() && matches_fields(&after, index, &fields, None).is_ok()))
            .ok_or_else(unsafe_change)?;
        self.save(&path, rel, &text)?;
        Ok(format!("{rel}#{index}"))
    }

    pub fn restore(&self, rel: &str, raw: &str) -> Result<String, Problem> {
        let (path, doc) = self.writable(rel)?;
        let index = doc.items().len();
        let keys = |doc: &yaml::Doc, at: usize| doc.items().get(at).map(|i| i.keys.iter().map(|k| (k.name.clone(), k.value.clone())).collect::<Vec<_>>());
        let was = yaml::read(&format!("matches:\n{raw}\n")).ok().and_then(|d| keys(&d, 0)).ok_or_else(unsafe_change)?;
        let text = ["\n\n", "\n"]
            .into_iter()
            .map(|gap| line_ends(&doc.text, add_raw(&doc, raw, gap)))
            .find(|text| yaml::read(text).is_ok_and(|after| same_except(&doc, &after, None, Some(index)).is_ok() && keys(&after, index).as_ref() == Some(&was)))
            .ok_or_else(unsafe_change)?;
        self.save(&path, rel, &text)?;
        Ok(format!("{rel}#{index}"))
    }

    pub fn update(&self, id: &str, raw: &str, fields: &Fields) -> Result<(), Problem> {
        let fields = check(fields)?;
        let (rel, index) = parse_id(id).ok_or_else(gone)?;
        let (path, doc) = self.writable(&rel)?;
        let item = doc.items().get(index).filter(|i| doc.raw(i) == raw).ok_or_else(changed)?;
        if describe(item).is_some() {
            return Err(other("Snippets does not write over a snippet it cannot show in full."));
        }
        let text = line_ends(&doc.text, change_item(&doc, item, &fields));
        if text == doc.text {
            return Ok(());
        }
        let after = yaml::read(&text).map_err(|_| unsafe_change())?;
        same_except(&doc, &after, Some(index), Some(index))?;
        matches_fields(&after, index, &fields, Some(item))?;
        self.save(&path, &rel, &text)
    }

    pub fn delete(&self, id: &str, raw: &str) -> Result<String, Problem> {
        let (rel, index) = parse_id(id).ok_or_else(gone)?;
        let (path, doc) = self.writable(&rel)?;
        let item = doc.items().get(index).filter(|i| doc.raw(i) == raw).ok_or_else(changed)?;
        let text = [true, false]
            .into_iter()
            .map(|tidy| line_ends(&doc.text, remove_item(&doc, item, tidy)))
            .find(|text| yaml::read(text).is_ok_and(|after| after.items().len() + 1 == doc.items().len() && same_except(&doc, &after, Some(index), None).is_ok()))
            .ok_or_else(unsafe_change)?;
        let written = doc.text[doc.lines[item.lines.start].start..item.span.end].to_string();
        self.save(&path, &rel, &text)?;
        Ok(written)
    }

    pub fn create_file(&self, name: &str) -> Result<String, String> {
        let name = name.trim();
        let cleaned = name.strip_suffix(".yml").or_else(|| name.strip_suffix(".yaml")).unwrap_or(name);
        let ok = !cleaned.is_empty() && cleaned.chars().count() <= 60 && cleaned.chars().all(|c| c.is_ascii_alphanumeric() || " _-".contains(c));
        if !ok {
            return Err("Use letters, numbers, spaces, dashes or underscores.".into());
        }
        let rel = format!("{}.yml", cleaned.split_whitespace().collect::<Vec<_>>().join("-").to_lowercase());
        let path = inside(&self.match_dir(), &rel).ok_or("That name cannot be used.")?;
        if fs::symlink_metadata(&path).is_ok() {
            return Err("There is already a file with that name.".into());
        }
        write(&path, &format!("# {cleaned}\n# Made by Snippets.\n\nmatches: []\n")).map_err(|e| e.words().to_string())?;
        Ok(rel)
    }

    fn save(&self, path: &Path, rel: &str, text: &str) -> Result<(), Problem> {
        self.backup(path, rel).map_err(|e| other(format!("Nothing was saved, because a copy of the file could not be kept: {e}")))?;
        write(path, text)
    }

    fn backup(&self, path: &Path, rel: &str) -> std::io::Result<()> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        fs::create_dir_all(&self.backups)?;
        fs::set_permissions(&self.backups, fs::Permissions::from_mode(0o700))?;
        let tail = format!("__{}", rel.replace('%', "%25").replace('/', "%2F"));
        files::write_whole_private(&self.backups.join(format!("{}{tail}", stamp())), &bytes)?;
        let mut kept: Vec<_> = fs::read_dir(&self.backups)?.flatten().map(|e| e.path()).filter(|p| is_copy_of(p, &tail)).collect();
        kept.sort();
        let extra = kept.len().saturating_sub(BACKUPS_KEPT);
        for old in &kept[..extra] {
            let _ = fs::remove_file(old);
        }
        Ok(())
    }

    pub fn export(&self, to: &Path) -> Result<(), String> {
        let root = self.match_dir();
        let mut packed = Vec::new();
        {
            let gz = flate2::write::GzEncoder::new(&mut packed, flate2::Compression::default());
            let mut tar = tar::Builder::new(gz);
            tar.follow_symlinks(false);
            tar.mode(tar::HeaderMode::Deterministic);
            tar.append_dir_all(".", &root).map_err(|e| format!("The snippets could not be packed: {e}"))?;
            let gz = tar.into_inner().map_err(|e| e.to_string())?;
            gz.finish().map_err(|e| e.to_string())?;
        }
        files::write_whole(to, &packed).map_err(|e| format!("{} could not be written: {e}", to.display()))
    }

    pub fn bring_in(&self, found: Vec<(String, Vec<u8>)>) -> Result<usize, String> {
        let dir = self.match_dir();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut brought = 0;
        for (name, bytes) in found {
            let free = free_name(&dir, &clean_name(&name)).ok_or("No free name was left for an imported file.")?;
            files::write_whole(&dir.join(&free), &bytes).map_err(|e| format!("{free} could not be written: {e}"))?;
            brought += 1;
        }
        Ok(brought)
    }
}

const STAMP_LEN: usize = 24;

fn is_copy_of(copy: &Path, tail: &str) -> bool {
    let name = copy.file_name().map_or(0, |n| n.len());
    name == STAMP_LEN + tail.len() && copy.to_string_lossy().ends_with(tail)
}

fn stamp() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let (secs, millis) = (now.as_secs() as i64, now.subsec_millis());
    let days = secs.div_euclid(86_400);
    let rest = secs.rem_euclid(86_400);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}-{:02}-{:02}-{millis:03}Z", rest / 3600, rest / 60 % 60, rest % 60)
}

fn is_yaml(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("yml") || e.eq_ignore_ascii_case("yaml"))
}

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

fn is_package(rel: &str) -> bool {
    Path::new(rel).components().next().is_some_and(|c| c.as_os_str() == "packages")
}

pub fn inside(root: &Path, rel: &str) -> Option<PathBuf> {
    let path = Path::new(rel);
    if rel.is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_))) || !is_yaml(path) {
        return None;
    }
    Some(root.join(path))
}

pub fn parse_id(id: &str) -> Option<(String, usize)> {
    let (rel, index) = id.rsplit_once('#')?;
    if rel.is_empty() {
        return None;
    }
    Some((rel.to_string(), index.parse().ok()?))
}

fn read_small(path: &Path) -> Result<String, String> {
    let mut bytes = Vec::new();
    fs::File::open(path).and_then(|f| f.take(FILE_MOST + 1).read_to_end(&mut bytes)).map_err(|e| format!("It could not be read: {e}"))?;
    if bytes.len() as u64 > FILE_MOST {
        return Err("It is too large to be snippets.".into());
    }
    String::from_utf8(bytes).map_err(|_| "It is not written in UTF-8.".into())
}

fn write(path: &Path, text: &str) -> Result<(), Problem> {
    files::write_whole(path, text.as_bytes()).map_err(|e| other(format!("It could not be saved: {e}")))
}

fn gone() -> Problem {
    other("That snippet is not there any more. Reload.")
}

fn changed() -> Problem {
    other("That snippet has changed in its file since it was shown. Reload.")
}

fn unsafe_change() -> Problem {
    other("Snippets could not save this without changing other snippets, so nothing was written.")
}

pub fn check(fields: &Fields) -> Result<Fields, Problem> {
    let triggers: Vec<String> = fields.triggers.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
    if triggers.is_empty() {
        return Err(Problem::Triggers("Give it at least one trigger."));
    }
    if triggers.iter().any(|t| t.chars().any(char::is_whitespace)) {
        return Err(Problem::Triggers("A trigger cannot hold spaces. Try :addr."));
    }
    if triggers.iter().any(|t| t.chars().count() > TRIGGER_MOST) {
        return Err(Problem::Triggers("That trigger is too long."));
    }
    if fields.replace.is_empty() {
        return Err(Problem::Replace("Give it something to turn into."));
    }
    if fields.replace.chars().count() > REPLACE_MOST {
        return Err(Problem::Replace("That is too long for a snippet."));
    }
    Ok(Fields { triggers, ..fields.clone() })
}

pub fn split_triggers(typed: &str) -> Vec<String> {
    typed.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

pub fn describe(item: &Item) -> Option<String> {
    if item.odd.is_some() {
        return Some("This snippet is not in a shape Snippets can edit.".into());
    }
    let unknown: Vec<&str> = item.keys.iter().map(|k| k.name.as_str()).filter(|n| !SIMPLE.contains(n)).collect();
    if !unknown.is_empty() {
        return Some(format!("This snippet uses {}, which Snippets shows but does not edit.", unknown.join(", ")));
    }
    if item.keys.iter().any(|k| matches!(k.value, Value::Odd(_))) {
        return Some("This snippet is written in a way Snippets shows but does not edit.".into());
    }
    if !matches!(item.get("replace"), Some(Value::Str(_))) {
        return Some("This snippet has no plain text to turn into.".into());
    }
    None
}

fn triggers_of(item: &Item) -> Vec<String> {
    if let Some(Value::Str(t)) = item.get("trigger") {
        return vec![t.clone()];
    }
    match item.get("triggers") {
        Some(Value::List(list)) => list.iter().filter_map(|v| if let Value::Str(s) = v { Some(s.clone()) } else { None }).collect(),
        _ => Vec::new(),
    }
}

fn entries_of(doc: &Doc, rel: &str, read_only: bool) -> Vec<Entry> {
    doc.items()
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let advanced = describe(item);
            let text = |name| if let Some(Value::Str(s)) = item.get(name) { Some(s.clone()) } else { None };
            Entry {
                id: format!("{rel}#{index}"),
                file: rel.to_string(),
                index,
                read_only,
                triggers: if item.odd.is_some() { Vec::new() } else { triggers_of(item) },
                label: text("label"),
                replace: if advanced.is_some() { None } else { text("replace") },
                word: item.get("word") == Some(&Value::Bool(true)),
                propagate_case: item.get("propagate_case") == Some(&Value::Bool(true)),
                picky: ["left_word", "right_word", "uppercase_style"].iter().any(|k| item.get(k).is_some()),
                raw: doc.raw(item).to_string(),
                advanced,
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests;
