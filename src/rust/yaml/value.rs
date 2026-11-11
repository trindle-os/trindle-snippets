//! Values after a name: quoted, plain, in a flow list or nested under it.

use super::block::read_block;
use super::{comment, content, indent_of, is_dash, strip_comment, Value};

pub fn read_value(inline: &str, below: &[&str], indent: usize) -> (Value, usize) {
    let words = strip_comment(inline).trim();
    let more = below.iter().rposition(|l| content(l)).map(|p| p + 1).unwrap_or(0);
    if words.is_empty() {
        if more == 0 {
            return (Value::Null, 0);
        }
        return read_nested(below, more, indent);
    }
    match words.chars().next().unwrap_or(' ') {
        '&' | '*' | '!' => (Value::Odd(t!("snippets-y-anchor")), more),
        '{' => (Value::Odd(t!("snippets-y-flow")), more),
        '[' => {
            if more > 0 {
                return (Value::Odd(t!("snippets-y-flow-list")), more);
            }
            (read_flow(words), 0)
        }
        '|' | '>' => read_block(words, below, indent),
        '"' | '\'' => read_quoted(inline.trim_start(), below),
        _ => read_plain(words, below, more),
    }
}

fn read_nested(below: &[&str], more: usize, indent: usize) -> (Value, usize) {
    let mut list = Vec::new();
    let mut dash = None;
    for line in &below[..more] {
        if !content(line) {
            continue;
        }
        let ind = indent_of(line);
        let body = &line[ind..];
        if !is_dash(body) || dash.is_some_and(|d| d != ind) || ind < indent {
            return (Value::Odd(t!("snippets-y-deep")), more);
        }
        dash = Some(ind);
        let (value, used) = read_value(&body[1..], &[], ind + 2);
        if used > 0 || matches!(value, Value::List(_)) {
            return (Value::Odd(t!("snippets-y-deep")), more);
        }
        list.push(value);
    }
    (Value::List(list), more)
}

fn read_flow(words: &str) -> Value {
    let Some(inner) = words.strip_prefix('[').and_then(|w| w.strip_suffix(']')) else {
        return Value::Odd(t!("snippets-y-list"));
    };
    let mut out = Vec::new();
    let mut rest = inner.trim();
    while !rest.is_empty() {
        let (value, used) = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => match quoted_end(rest, q) {
                Some(end) => (read_quoted(&rest[..end], &[]).0, end),
                None => return Value::Odd(t!("snippets-y-list")),
            },
            Some('[' | '{' | '&' | '*' | '!') => return Value::Odd(t!("snippets-y-list")),
            _ => {
                let end = rest.find(',').unwrap_or(rest.len());
                (plain_value(rest[..end].trim()), end)
            }
        };
        out.push(value);
        rest = rest[used..].trim_start();
        match rest.strip_prefix(',') {
            Some(r) => rest = r.trim_start(),
            None if rest.is_empty() => {}
            None => return Value::Odd(t!("snippets-y-list")),
        }
    }
    Value::List(out)
}

fn quoted_end(s: &str, q: char) -> Option<usize> {
    let mut chars = s.char_indices().skip(1).peekable();
    while let Some((k, c)) = chars.next() {
        if q == '"' && c == '\\' {
            chars.next();
        } else if c == q {
            if q == '\'' && chars.peek().is_some_and(|&(_, n)| n == '\'') {
                chars.next();
            } else {
                return Some(k + 1);
            }
        }
    }
    None
}

fn read_quoted(first: &str, below: &[&str]) -> (Value, usize) {
    let q = first.chars().next().unwrap_or('"');
    let mut whole = first.to_string();
    let mut used = 0;
    loop {
        if let Some(end) = quoted_end(&whole, q) {
            if content(&whole[end..]) {
                return (Value::Odd(t!("snippets-y-after-quote")), used);
            }
            let body = &whole[1..end - 1];
            return match unquote(body, q) {
                Some(s) => (Value::Str(s), used),
                None => (Value::Odd(t!("snippets-y-escape")), used),
            };
        }
        let Some(next) = below.get(used) else {
            return (Value::Odd(t!("snippets-y-open-quote")), used);
        };
        whole.push('\n');
        whole.push_str(next);
        used += 1;
    }
}

fn unquote(body: &str, q: char) -> Option<String> {
    let mut out = String::new();
    let chars: Vec<char> = body.chars().collect();
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if c == '\n' {
            k = fold_break(&chars, k, &mut out);
        } else if q == '\'' && c == '\'' {
            out.push('\'');
            k += 2;
        } else if q == '"' && c == '\\' {
            k = escape(&chars, k + 1, &mut out)?;
        } else {
            out.push(c);
            k += 1;
        }
    }
    Some(out)
}

fn fold_break(chars: &[char], mut k: usize, out: &mut String) -> usize {
    let kept = out.trim_end_matches([' ', '\t']).len();
    out.truncate(kept);
    let mut breaks = 0;
    k += 1;
    loop {
        while k < chars.len() && (chars[k] == ' ' || chars[k] == '\t') {
            k += 1;
        }
        if k < chars.len() && chars[k] == '\n' {
            breaks += 1;
            k += 1;
        } else {
            break;
        }
    }
    if breaks == 0 {
        out.push(' ');
    } else {
        out.extend(std::iter::repeat_n('\n', breaks));
    }
    k
}

fn escape(chars: &[char], at: usize, out: &mut String) -> Option<usize> {
    let mut k = at + 1;
    let hex = |len: usize, k: &mut usize| -> Option<char> {
        let s: String = chars.get(*k..*k + len)?.iter().collect();
        *k += len;
        char::from_u32(u32::from_str_radix(&s, 16).ok()?)
    };
    match *chars.get(at)? {
        '0' => out.push('\0'),
        'a' => out.push('\u{7}'),
        'b' => out.push('\u{8}'),
        't' | '\t' => out.push('\t'),
        'n' => out.push('\n'),
        'v' => out.push('\u{b}'),
        'f' => out.push('\u{c}'),
        'r' => out.push('\r'),
        'e' => out.push('\u{1b}'),
        ' ' => out.push(' '),
        '"' => out.push('"'),
        '/' => out.push('/'),
        '\\' => out.push('\\'),
        'N' => out.push('\u{85}'),
        '_' => out.push('\u{a0}'),
        'L' => out.push('\u{2028}'),
        'P' => out.push('\u{2029}'),
        'x' => out.push(hex(2, &mut k)?),
        'u' => out.push(hex(4, &mut k)?),
        'U' => out.push(hex(8, &mut k)?),
        '\n' => {
            while k < chars.len() && (chars[k] == ' ' || chars[k] == '\t') {
                k += 1;
            }
        }
        _ => return None,
    }
    Some(k)
}

fn read_plain(words: &str, below: &[&str], more: usize) -> (Value, usize) {
    if words.contains(": ") || words.ends_with(':') {
        return (Value::Odd(t!("snippets-y-colon")), more);
    }
    if more == 0 {
        return (plain_value(words), 0);
    }
    let mut out = words.to_string();
    let mut breaks = 0;
    for line in &below[..more] {
        if comment(line) {
            return (Value::Odd(t!("snippets-y-comment")), more);
        }
        let t = strip_comment(line).trim();
        if t.is_empty() {
            breaks += 1;
            continue;
        }
        if t.contains(": ") {
            return (Value::Odd(t!("snippets-y-colon")), more);
        }
        if breaks == 0 {
            out.push(' ');
        } else {
            out.extend(std::iter::repeat_n('\n', breaks));
        }
        breaks = 0;
        out.push_str(t);
    }
    (Value::Str(out), more)
}

pub fn plain_value(s: &str) -> Value {
    match s {
        "" | "~" | "null" | "Null" | "NULL" => return Value::Null,
        "true" | "True" | "TRUE" => return Value::Bool(true),
        "false" | "False" | "FALSE" => return Value::Bool(false),
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" | "-.inf" | "-.Inf" | "-.INF" | ".nan" | ".NaN" | ".NAN" => {
            return Value::Number(s.into())
        }
        _ => {}
    }
    let digits = s.strip_prefix(['-', '+']).unwrap_or(s);
    let int = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
    let radix = |p: &str, ok: fn(u8) -> bool| s.strip_prefix(p).is_some_and(|r| !r.is_empty() && r.bytes().all(ok));
    let hex = radix("0x", |b| b.is_ascii_hexdigit());
    let oct = radix("0o", |b| (b'0'..=b'7').contains(&b));
    if int || hex || oct || float(digits) {
        return Value::Number(s.into());
    }
    Value::Str(s.into())
}

fn float(s: &str) -> bool {
    let (mantissa, exp) = match s.find(['e', 'E']) {
        Some(at) => (&s[..at], Some(&s[at + 1..])),
        None => (s, None),
    };
    let (whole, part) = match mantissa.split_once('.') {
        Some((w, p)) => (w, Some(p)),
        None => (mantissa, None),
    };
    let digits = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok = digits(whole) && part.is_none_or(digits) && (whole.len() + part.map_or(0, str::len)) > 0;
    let exp_ok = exp.is_none_or(|e| {
        let e = e.strip_prefix(['-', '+']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    mantissa_ok && exp_ok && (part.is_some() || exp.is_some())
}
