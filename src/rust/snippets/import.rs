//! Reading snippet files and archives to bring in: what is read, how much, and under what names.

use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use super::*;

pub(super) const FILE_MOST: u64 = 8 * 1024 * 1024;
pub(super) const IMPORT_MOST: usize = 500;

pub(super) const TOTAL_MOST: u64 = 32 * 1024 * 1024;
pub(super) const INFLATED_MOST: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub struct Incoming {
    pub files: Vec<(String, Vec<u8>)>,

    pub risky: Vec<String>,
}

pub fn read_import(from: &Path) -> Result<Incoming, String> {
    let name = from.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let files: Vec<(String, Vec<u8>)> = if name.ends_with(".gz") || name.ends_with(".tgz") {
        unpack(from)?
    } else if is_yaml(from) {
        let mut bytes = Vec::new();
        fs::File::open(from).and_then(|f| f.take(FILE_MOST + 1).read_to_end(&mut bytes)).map_err(|e| format!("It could not be read: {e}"))?;
        if bytes.len() as u64 > FILE_MOST {
            return Err("That file is too large to be snippets.".into());
        }
        vec![(name, bytes)]
    } else {
        return Err("Choose a .yml, .yaml or .tar.gz file.".into());
    };
    if files.is_empty() {
        return Err("No snippet files were found in it.".into());
    }
    let risky = files.iter().filter(|(_, bytes)| runs_commands(bytes)).map(|(name, _)| name.clone()).collect();
    Ok(Incoming { files, risky })
}

pub(super) fn runs_commands(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let parsed: Result<Vec<serde_yaml::Value>, _> = serde_yaml::Deserializer::from_str(&text).map(serde_yaml::Value::deserialize).collect();
    match parsed {
        Ok(docs) => docs.iter().any(names_command),
        Err(_) => unescaped(&text).lines().any(|line| {
            let untagged: Vec<&str> = line.split_whitespace().filter(|w| !w.starts_with('!')).collect();
            let squeezed: String = untagged.concat().chars().filter(|c| !"\"'".contains(*c)).collect();
            squeezed.contains("type:shell") || squeezed.contains("type:script")
        }),
    }
}

fn names_command(value: &serde_yaml::Value) -> bool {
    use serde_yaml::Value::{Mapping, Sequence, Tagged};
    match value {
        Mapping(map) => map.iter().any(|(k, v)| (is_word(k, "type") && (is_word(v, "shell") || is_word(v, "script"))) || names_command(k) || names_command(v)),
        Sequence(items) => items.iter().any(names_command),
        Tagged(tagged) => names_command(&tagged.value),
        _ => false,
    }
}

fn is_word(value: &serde_yaml::Value, word: &str) -> bool {
    match value {
        serde_yaml::Value::String(s) => s.trim() == word,
        serde_yaml::Value::Tagged(tagged) => is_word(&tagged.value, word),
        _ => false,
    }
}

fn unescaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let digits = match chars.peek() {
            Some('x') => 2,
            Some('u') => 4,
            Some('U') => 8,
            Some('\n') => {
                while chars.next_if(|c| c.is_whitespace()).is_some() {}
                continue;
            }
            _ => 0,
        };
        if digits == 0 {
            out.extend(chars.next());
            continue;
        }
        chars.next();
        let hex: String = std::iter::from_fn(|| chars.next_if(char::is_ascii_hexdigit)).take(digits).collect();
        out.extend(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32));
    }
    out
}

pub(super) fn unpack(from: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let file = fs::File::open(from).map_err(|e| format!("It could not be opened: {e}"))?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file).take(INFLATED_MOST));
    let mut out = Vec::new();
    let mut total = 0u64;
    let too_big = || "That archive is too large, or damaged.".to_string();
    let entries = tar.entries().map_err(|_| "That archive could not be opened.".to_string())?;
    for entry in entries {
        let mut entry = entry.map_err(|_| too_big())?;
        if entry.header().entry_type() != tar::EntryType::Regular {
            continue;
        }
        let Some(name) = entry.path().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())) else { continue };
        if !is_yaml(Path::new(&name)) || name.starts_with('.') {
            continue;
        }
        if entry.size() > FILE_MOST {
            return Err(format!("{name} is too large to be snippets."));
        }
        total += entry.size();
        if total > TOTAL_MOST {
            return Err("That archive holds too much to be snippets.".into());
        }
        if out.len() >= IMPORT_MOST {
            return Err("That archive holds too many files.".into());
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|_| too_big())?;
        out.push((name, bytes));
    }
    Ok(out)
}

pub(super) fn clean_name(name: &str) -> String {
    let stem = name.strip_suffix(".yml").or_else(|| name.strip_suffix(".yaml")).unwrap_or(name);
    let stem: String = stem.chars().map(|c| if c.is_alphanumeric() || "_-.".contains(c) { c } else { '-' }).collect();
    let stem = stem.trim_matches(['.', '-']);
    format!("{}.yml", if stem.is_empty() { "imported" } else { stem })
}

pub fn free_name(dir: &Path, name: &str) -> Option<String> {
    let stem = name.strip_suffix(".yml").or_else(|| name.strip_suffix(".yaml")).unwrap_or(name);
    let mut candidate = name.to_string();
    for n in 0..100 {
        if fs::symlink_metadata(dir.join(&candidate)).is_err() {
            return Some(candidate);
        }
        candidate = if n == 0 { format!("{stem}-imported.yml") } else { format!("{stem}-imported-{n}.yml") };
    }
    None
}
