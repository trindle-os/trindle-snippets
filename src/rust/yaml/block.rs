//! A `|` or `>` block value.

use super::{blank, indent_of, strip_comment, Value};

pub(super) fn read_block(header: &str, below: &[&str], key: usize) -> (Value, usize) {
    let Some((chomp, given)) = block_header(header, key) else {
        return (Value::Odd("Its block has a header Snippets does not edit."), below.len());
    };

    let ind = given.or_else(|| below.iter().find(|l| !blank(l)).map(|l| indent_of(l)).filter(|&i| i > key));
    let reach = match ind {
        Some(ind) => below.iter().position(|l| !blank(l) && indent_of(l) < ind).unwrap_or(below.len()),
        None => below.iter().position(|l| !blank(l)).unwrap_or(below.len()),
    };
    let below = &below[..reach];
    let content_lines = below.iter().rposition(|l| !blank(l)).map(|p| p + 1).unwrap_or(0);
    let Some(ind) = ind else {
        return (Value::Str(String::new()), if chomp == '+' { below.len() } else { 0 });
    };
    let Some(rows) = block_rows(&below[..content_lines], ind) else {
        return (Value::Odd("Its block is indented in a way Snippets cannot follow."), content_lines);
    };
    let mut out = if header.starts_with('>') { fold(&rows) } else { rows.join("\n") };
    let used = match chomp {
        '-' => content_lines,
        '+' => {
            out.push('\n');
            out.extend(std::iter::repeat_n('\n', below.len() - content_lines));
            below.len()
        }
        _ => {
            out.push('\n');
            content_lines
        }
    };
    (Value::Str(out), used)
}

fn block_header(header: &str, key: usize) -> Option<(char, Option<usize>)> {
    let mut chomp = ' ';
    let mut given = None;
    for c in strip_comment(&header[1..]).trim().chars() {
        match c {
            '+' | '-' if chomp == ' ' => chomp = c,
            '1'..='9' if given.is_none() => given = c.to_digit(10).map(|d| key + d as usize),
            _ => return None,
        }
    }
    Some((chomp, given))
}

fn block_rows<'a>(lines: &[&'a str], ind: usize) -> Option<Vec<&'a str>> {
    let mut rows = Vec::new();
    for line in lines {
        if blank(line) && line.len() <= ind {
            rows.push("");
        } else if indent_of(line) < ind {
            return None;
        } else {
            rows.push(&line[ind..]);
        }
    }
    Some(rows)
}

fn fold(rows: &[&str]) -> String {
    let mut out = String::new();
    let mut pending = 0;
    let mut started = false;
    let mut prev_more = false;
    for row in rows {
        if row.is_empty() {
            pending += 1;
            continue;
        }
        let more = row.starts_with(' ') || row.starts_with('\t');
        if started {
            if pending == 0 {
                out.push(if more || prev_more { '\n' } else { ' ' });
            } else {
                let n = if more || prev_more { pending + 1 } else { pending };
                out.extend(std::iter::repeat_n('\n', n));
            }
        } else {
            out.extend(std::iter::repeat_n('\n', pending));
        }
        out.push_str(row);
        pending = 0;
        started = true;
        prev_more = more;
    }
    out
}
