//! Reads only the YAML shape of snippet files, keeping where each snippet and value sits in the text; anything else is marked odd and never written over.

use std::ops::Range;

mod block;
mod value;
#[cfg(test)]
mod tests;

use value::read_value;
#[cfg(test)]
use value::plain_value;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    Str(String),
    List(Vec<Value>),

    Odd(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub name: String,

    pub start: usize,

    pub end: usize,

    pub lines: Range<usize>,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {

    pub lines: Range<usize>,

    pub span: Range<usize>,

    pub indent: usize,
    pub keys: Vec<Key>,

    pub odd: Option<&'static str>,
}

impl Item {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.keys.iter().find(|k| k.name == name).map(|k| &k.value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum List {

    Missing,

    Empty { line: usize, after: Range<usize> },

    Items { line: usize, indent: usize, items: Vec<Item> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Doc {
    pub text: String,

    pub lines: Vec<Range<usize>>,
    pub list: List,
}

impl Doc {
    pub fn items(&self) -> &[Item] {
        match &self.list {
            List::Items { items, .. } => items,
            _ => &[],
        }
    }

    pub fn line(&self, i: usize) -> &str {
        &self.text[self.lines[i].clone()]
    }

    pub fn raw(&self, item: &Item) -> &str {
        &self.text[item.span.clone()]
    }
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

fn content(line: &str) -> bool {
    !blank(line) && !comment(line)
}

pub fn read(text: &str) -> Result<Doc, String> {
    let mut lines = Vec::new();
    let mut at = 0;
    for piece in text.split_inclusive('\n') {
        let body = piece.strip_suffix('\n').unwrap_or(piece);
        let body = body.strip_suffix('\r').unwrap_or(body);
        lines.push(at..at + body.len());
        at += piece.len();
    }
    let doc = Doc { text: text.to_string(), lines, list: List::Missing };
    let list = read_list(&doc)?;
    Ok(Doc { list, ..doc })
}

fn read_list(doc: &Doc) -> Result<List, String> {
    let mut found: Option<List> = None;
    let mut seen_content = false;
    let mut i = 0;
    while i < doc.lines.len() {
        let line = doc.line(i);
        refuse_unreadable(i, line)?;
        if !content(line) {
            i += 1;
            continue;
        }
        if document_marker(line, seen_content)? {
            i += 1;
            continue;
        }
        let first_content = !seen_content;
        seen_content = true;
        if indent_of(line) > 0 {
            if first_content {
                return Err(t!("snippets-y-first-indent").into());
            }

            i += 1;
            continue;
        }
        let Some((name, after)) = key_at(line, 0) else {
            return Err(t!("snippets-y-not-named").into());
        };
        if name != "matches" {
            i = skip_value(doc, i, 0);
            continue;
        }
        if found.is_some() {
            return Err(t!("snippets-y-matches-twice").into());
        }
        let (list, next) = read_matches(doc, i, after)?;
        found = Some(list);
        i = next;
    }
    Ok(found.unwrap_or(List::Missing))
}

fn refuse_unreadable(i: usize, line: &str) -> Result<(), String> {
    if i == 0 && line.starts_with('\u{feff}') {
        return Err(t!("snippets-y-bom").into());
    }
    if line.starts_with('\t') && !blank(line) {
        return Err(t!("snippets-y-tabs").into());
    }
    Ok(())
}

fn document_marker(line: &str, seen_content: bool) -> Result<bool, String> {
    if line.starts_with('%') {
        return Err(t!("snippets-y-directive").into());
    }
    let trimmed = line.trim_end();
    if trimmed == "---" || trimmed.starts_with("--- #") {
        if seen_content {
            return Err(t!("snippets-y-documents").into());
        }
        return Ok(true);
    }
    if trimmed == "..." {
        return Err(t!("snippets-y-documents").into());
    }
    Ok(false)
}

fn read_matches(doc: &Doc, i: usize, after: usize) -> Result<(List, usize), String> {
    let value = strip_comment(&doc.line(i)[after..]);
    let rest = value.trim();
    if !(rest == "[]" || rest.is_empty() || rest == "~" || rest == "null") {
        return Err(t!("snippets-y-one-line").into());
    }
    let end = skip_value(doc, i, 0);
    if rest.is_empty() && (i + 1..end).any(|j| content(doc.line(j))) {
        return read_items(doc, i);
    }
    let start = doc.lines[i].start + after;
    Ok((List::Empty { line: i, after: start..start + value.trim_end().len() }, end))
}

fn skip_value(doc: &Doc, i: usize, indent: usize) -> usize {
    let mut j = i + 1;
    while j < doc.lines.len() {
        let line = doc.line(j);
        if content(line) && indent_of(line) <= indent && !(indent == 0 && is_dash(line)) {
            break;
        }
        j += 1;
    }
    j
}

fn key_at(line: &str, at: usize) -> Option<(String, usize)> {
    let rest = line.get(at..)?;
    let (name, used) = if let Some(q @ ('"' | '\'')) = rest.chars().next() {
        let close = rest[1..].find(q)? + 1;
        (rest[1..close].to_string(), close + 1)
    } else {
        let mut end = None;
        for (k, c) in rest.char_indices() {
            if c == ':' && rest[k + 1..].chars().next().is_none_or(|n| n == ' ') {
                end = Some(k);
                break;
            }
            if c == '#' && k > 0 && rest[..k].ends_with(' ') {
                return None;
            }
        }
        let end = end?;
        let name = rest[..end].trim_end();
        let first = name.chars().next()?;
        if "-?:,[]{}#&*!|>'\"%@`".contains(first) {
            return None;
        }
        (name.to_string(), end)
    };
    let after = rest.get(used..)?;
    if !after.starts_with(':') || after[1..].chars().next().is_some_and(|c| c != ' ') {
        return None;
    }
    Some((name, at + used + 1))
}

fn strip_comment(s: &str) -> &str {
    let mut quote = None;
    let bytes = s.as_bytes();
    for (k, c) in s.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') if k == 0 || bytes[k - 1] == b' ' || bytes[k - 1] == b'[' || bytes[k - 1] == b',' => quote = Some(c),
            (Some(q), c) if c == q => {
                if !(q == '"' && k > 0 && bytes[k - 1] == b'\\' && !escaped_backslash(&s[..k])) {
                    quote = None;
                }
            }
            (None, '#') if k == 0 || bytes[k - 1] == b' ' => return &s[..k],
            _ => {}
        }
    }
    s
}

fn escaped_backslash(s: &str) -> bool {
    let run = s.len() - s.trim_end_matches('\\').len();
    run % 2 == 0
}

fn read_items(doc: &Doc, key_line: usize) -> Result<(List, usize), String> {
    let n = doc.lines.len();
    let mut i = key_line + 1;
    while i < n && !content(doc.line(i)) {
        i += 1;
    }
    let first = doc.line(i);
    let dash = indent_of(first);
    if !is_dash(&first[dash..]) {
        return Err(t!("snippets-y-matches-not-list").into());
    }
    let mut items = Vec::new();
    while i < n {
        let line = doc.line(i);
        if !content(line) {
            i += 1;
            continue;
        }
        let ind = indent_of(line);
        if ind == dash && is_dash(&line[dash..]) {
            let (item, next) = read_item(doc, i, dash);
            items.push(item);
            i = next;
            continue;
        }
        if ind == 0 {
            break;
        }
        return Err(t!("snippets-y-line-indent", line = i + 1));
    }
    Ok((List::Items { line: key_line, indent: dash, items }, i))
}

fn is_dash(s: &str) -> bool {
    s == "-" || s.starts_with("- ") || s.trim_end() == "-"
}

fn read_item(doc: &Doc, i: usize, dash: usize) -> (Item, usize) {

    let mut end = i + 1;
    let mut last = i;
    while end < doc.lines.len() {
        let line = doc.line(end);
        if content(line) && indent_of(line) <= dash {
            break;
        }
        if content(line) {
            last = end;
        }
        end += 1;
    }
    let mut item = Item {
        lines: i..last + 1,
        span: doc.lines[i].start + dash..doc.lines[last].end,
        indent: dash + 2,
        keys: Vec::new(),
        odd: None,
    };
    let after_dash = &doc.line(i)[dash + 1..];
    let first = if content(after_dash) {
        Some((i, dash + 1 + indent_of(after_dash)))
    } else {
        (i + 1..=last).find(|&j| content(doc.line(j))).map(|j| (j, indent_of(doc.line(j))))
    };
    let Some((k, indent)) = first else {
        item.odd = Some(t!("snippets-y-empty"));
        return (item, last + 1);
    };
    item.indent = indent;
    read_keys(doc, &mut item, (i, k, end, last), indent);
    (item, last + 1)
}

fn read_keys(doc: &Doc, item: &mut Item, (i, mut k, end, last): (usize, usize, usize, usize), indent: usize) {
    while k <= last {
        let line = doc.line(k);
        if !content(line) {
            k += 1;
            continue;
        }
        if indent_of(line) != indent && k != i {
            item.odd = Some(t!("snippets-y-item-indent"));
            break;
        }
        let Some((name, after)) = key_at(line, indent) else {
            item.odd = Some(t!("snippets-y-item-not-named"));
            break;
        };
        if item.keys.iter().any(|key| key.name == name) {
            item.odd = Some(t!("snippets-y-item-twice"));
        }

        let under = (k + 1..end).find(|&j| content(doc.line(j)) && indent_of(doc.line(j)) <= indent).unwrap_or(end);

        let below: Vec<&str> = (k + 1..under).map(|j| doc.line(j)).collect();
        let (value, used) = read_value(&line[after..], &below, indent);
        let value_last = k + used;
        item.keys.push(Key {
            name,
            start: doc.lines[k].start + indent,
            end: doc.lines[value_last].end,
            lines: k..value_last + 1,
            value,
        });
        k = under;
    }
}

pub fn write_str(s: &str, indent: usize) -> String {
    let block_safe = s.contains('\n')
        && !s.chars().any(|c| (c.is_control() && c != '\n' && c != '\t') || c == '\u{feff}' || c == '\u{85}' || c == '\u{2028}' || c == '\u{2029}')
        && !s.starts_with('\n');
    if !block_safe {
        return quote(s);
    }
    let body = s.trim_end_matches('\n');
    let trailing = s.len() - body.len();
    let chomp = match trailing {
        0 => "-",
        1 => "",
        _ => "+",
    };
    let first = body.lines().next().unwrap_or("");
    let lead = if first.starts_with(' ') || first.starts_with('\t') { "2" } else { "" };
    let pad = " ".repeat(indent + 2);
    let mut out = format!("|{lead}{chomp}");
    for line in body.split('\n') {
        out.push('\n');
        if !line.is_empty() {
            out.push_str(&pad);
            out.push_str(line);
        }
    }
    for _ in 1..trailing {
        out.push('\n');
    }
    out
}

pub fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{feff}' | '\u{85}' | '\u{2028}' | '\u{2029}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
