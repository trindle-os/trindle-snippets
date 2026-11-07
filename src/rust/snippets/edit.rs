//! The text of a snippet file with one snippet added, changed or taken out, and the checks that nothing else changed.

use super::*;

pub(super) fn key_text(name: &str, value: &Value, indent: usize) -> String {
    match value {
        Value::Str(s) => format!("{name}: {}", yaml::write_str(s, indent)),
        Value::Bool(b) => format!("{name}: {b}"),
        Value::List(list) => {
            let pad = " ".repeat(indent + 2);
            let mut out = format!("{name}:");
            for v in list {
                if let Value::Str(s) = v {
                    out.push_str(&format!("\n{pad}- {}", yaml::quote(s)));
                }
            }
            out
        }
        _ => format!("{name}: null"),
    }
}

pub(super) fn wanted(fields: &Fields) -> Vec<(&'static str, Value)> {
    let mut out = Vec::new();
    match fields.triggers.as_slice() {
        [one] => out.push(("trigger", Value::Str(one.clone()))),
        many => out.push(("triggers", Value::List(many.iter().cloned().map(Value::Str).collect()))),
    }
    out.push(("replace", Value::Str(fields.replace.clone())));
    if fields.word {
        out.push(("word", Value::Bool(true)));
    }
    if fields.propagate_case {
        out.push(("propagate_case", Value::Bool(true)));
    }
    out
}

pub(super) fn line_ends(old: &str, new: String) -> String {
    let crlf = old.matches("\r\n").count();
    if crlf > 0 && crlf == old.matches('\n').count() { new.replace("\r\n", "\n").replace('\n', "\r\n") } else { new }
}

pub(super) struct Edit {
    at: std::ops::Range<usize>,
    with: String,
}

pub(super) fn apply(text: &str, mut edits: Vec<Edit>) -> String {

    edits.sort_by(|a, b| a.at.start.cmp(&b.at.start).then(a.at.end.cmp(&b.at.end)));
    let mut merged: Vec<Edit> = Vec::new();
    for e in edits {
        match merged.last_mut() {
            Some(last) if last.with.is_empty() && e.with.is_empty() && e.at.start < last.at.end => last.at.end = last.at.end.max(e.at.end),
            _ => merged.push(e),
        }
    }
    let mut edits = merged;
    edits.sort_by(|a, b| b.at.start.cmp(&a.at.start).then(b.at.end.cmp(&a.at.end)));
    let mut out = text.to_string();
    for e in edits {
        out.replace_range(e.at, &e.with);
    }
    out
}

pub(super) fn after_break(text: &str, at: usize) -> usize {
    let rest = &text[at..];
    at + if rest.starts_with("\r\n") { 2 } else { usize::from(rest.starts_with('\n')) }
}

pub(super) fn item_end(item: &Item) -> usize {
    item.keys.iter().map(|k| k.end).fold(item.span.end, usize::max)
}

pub(super) fn add_item(doc: &Doc, fields: &Fields, gap: &str) -> String {
    add_lines(doc, gap, |dash| {
        let indent = dash + 2;
        wanted(fields)
            .iter()
            .enumerate()
            .map(|(i, (name, value))| {
                let lead = if i == 0 { format!("{}- ", " ".repeat(dash)) } else { " ".repeat(indent) };
                format!("{lead}{}", key_text(name, value, indent))
            })
            .collect::<Vec<_>>()
            .join("\n")
    })
}

pub(super) fn add_raw(doc: &Doc, raw: &str, gap: &str) -> String {
    let was = raw.len() - raw.trim_start_matches(' ').len();
    add_lines(doc, gap, |dash| {
        let moved = |l: &str| if l.trim().is_empty() { l.to_string() } else { format!("{}{}", " ".repeat(dash), l.get(was.min(l.len() - l.trim_start_matches(' ').len())..).unwrap_or(l)) };
        raw.lines().map(moved).collect::<Vec<_>>().join("\n")
    })
}

fn add_lines(doc: &Doc, gap: &str, lines: impl Fn(usize) -> String) -> String {
    match &doc.list {
        List::Items { indent, items, .. } => {
            let end = items.last().map(item_end).unwrap_or(doc.text.len());
            apply(&doc.text, vec![Edit { at: end..end, with: format!("{gap}{}", lines(*indent)) }])
        }
        List::Empty { line, after } => {
            let end = doc.lines[*line].end;
            let mut edits = vec![Edit { at: after.clone(), with: String::new() }];
            edits.push(Edit { at: end..end, with: format!("\n{}", lines(2)) });

            if after.start == end {
                edits.remove(0);
            }
            apply(&doc.text, edits)
        }
        List::Missing => {
            let mut text = doc.text.clone();
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&format!("matches:\n{}\n", lines(2)));
            text
        }
    }
}

pub(super) fn change_item(doc: &Doc, item: &Item, fields: &Fields) -> String {
    let indent = item.indent;
    let mut edits = Vec::new();
    let mut add = Vec::new();
    let mut gone = Vec::new();
    let position = |name: &str| item.keys.iter().position(|k| k.name == name);
    let replace = |edits: &mut Vec<Edit>, at: usize, name: &str, value: &Value| {
        let k = &item.keys[at];
        if k.name != name || &k.value != value {
            edits.push(Edit { at: k.start..k.end, with: key_text(name, value, indent) });
        }
    };
    for (name, value) in wanted(fields) {
        match name {
            "trigger" | "triggers" => {
                let other = if name == "trigger" { "triggers" } else { "trigger" };
                match (position(name), position(other)) {
                    (Some(s), o) => {
                        replace(&mut edits, s, name, &value);
                        gone.extend(o);
                    }
                    (None, Some(o)) => replace(&mut edits, o, name, &value),
                    (None, None) => add.push(key_text(name, &value, indent)),
                }
            }
            _ => match position(name) {
                Some(at) => replace(&mut edits, at, name, &value),
                None => add.push(key_text(name, &value, indent)),
            },
        }
    }
    for (flag, on) in [("word", fields.word), ("propagate_case", fields.propagate_case)] {
        if let (false, Some(at)) = (on, position(flag)) {
            gone.push(at);
        }
    }

    let lead = (0..item.keys.len()).take_while(|i| gone.contains(i)).count();
    if lead > 0 {
        if let Some(next) = item.keys.get(lead) {
            edits.push(Edit { at: item.keys[0].start..next.start, with: String::new() });
        }
    }
    for &at in gone.iter().filter(|&&at| at >= lead) {
        edits.push(Edit { at: key_range(doc, item, at), with: String::new() });
    }
    if !add.is_empty() {
        let end = item.keys.iter().enumerate().filter(|(i, _)| !gone.contains(i)).map(|(_, k)| k.end).max().unwrap_or(item.span.end);
        let pad = " ".repeat(indent);
        let with = add.iter().map(|a| ["\n", &pad, a].concat()).collect();
        edits.push(Edit { at: end..end, with });
    }
    apply(&doc.text, edits)
}

pub(super) fn key_range(doc: &Doc, item: &Item, at: usize) -> std::ops::Range<usize> {
    let key = &item.keys[at];
    let start = doc.lines[key.lines.start].start;
    if key.end < doc.text.len() {
        start..after_break(&doc.text, key.end)
    } else {
        start.saturating_sub(1)..key.end
    }
}

pub(super) fn remove_item(doc: &Doc, item: &Item, tidy: bool) -> String {
    let items = doc.items();
    if items.len() == 1 {
        if let List::Items { line, .. } = &doc.list {
            let key_line = doc.line(*line);
            let colon = key_line.find(':').map(|c| c + 1).unwrap_or(key_line.len());
            let from = doc.lines[*line].start + colon;
            let first = doc.lines[item.lines.start].start;
            let end = item_end(item);
            let stop = after_break(&doc.text, end);
            let first = if stop == end { first.saturating_sub(1).max(from) } else { first };
            return apply(&doc.text, vec![Edit { at: from..from, with: " []".into() }, Edit { at: first..stop, with: String::new() }]);
        }
    }
    let first = item.lines.start;
    let mut start = doc.lines[first].start;
    let end = item_end(item);
    let mut stop = after_break(&doc.text, end);
    let blank = |i: usize| doc.lines.get(i).is_some_and(|r| doc.text[r.clone()].trim().is_empty());
    let last_line = doc.lines.iter().position(|r| r.end >= end).unwrap_or(doc.lines.len() - 1);
    if tidy {
        if first > 0 && blank(first - 1) {
            start = doc.lines[first - 1].start;
        } else if blank(last_line + 1) {
            stop = doc.lines.get(last_line + 1).map(|r| after_break(&doc.text, r.end)).unwrap_or(stop);
        } else if end >= doc.text.len() && start > 0 {
            start -= 1;
        }
    }
    apply(&doc.text, vec![Edit { at: start..stop, with: String::new() }])
}

pub(super) fn same_except(before: &Doc, after: &Doc, changed: Option<usize>, new: Option<usize>) -> Result<(), Problem> {
    type Seen = (String, Vec<(String, Value)>, Option<&'static str>);
    let seen = |doc: &Doc, skip: Option<usize>| -> Vec<Seen> {
        doc.items()
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != skip)
            .map(|(_, it)| (doc.raw(it).to_string(), it.keys.iter().map(|k| (k.name.clone(), k.value.clone())).collect(), it.odd))
            .collect()
    };
    let (old, now) = (seen(before, changed), seen(after, new));
    if old != now {
        return Err(unsafe_change());
    }
    Ok(())
}

pub(super) fn matches_fields(after: &Doc, index: usize, fields: &Fields, before: Option<&Item>) -> Result<(), Problem> {
    let item = after.items().get(index).ok_or_else(unsafe_change)?;
    let ok = describe(item).is_none()
        && triggers_of(item) == fields.triggers
        && item.get("replace") == Some(&Value::Str(fields.replace.clone()))
        && (item.get("word") == Some(&Value::Bool(true))) == fields.word
        && (item.get("propagate_case") == Some(&Value::Bool(true))) == fields.propagate_case;
    let touched = ["trigger", "triggers", "replace", "word", "propagate_case"];
    let rest = |it: &Item| it.keys.iter().filter(|k| !touched.contains(&k.name.as_str())).map(|k| (k.name.clone(), k.value.clone())).collect::<Vec<_>>();
    let kept = before.is_none_or(|b| rest(b) == rest(item));
    if ok && kept { Ok(()) } else { Err(unsafe_change()) }
}
