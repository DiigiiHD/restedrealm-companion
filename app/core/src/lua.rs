//! Read WoW SavedVariables as data. This module never executes Lua.
//!
//! It mirrors `companion/save_parser.py` rule for rule so that a save yields the
//! same values, and therefore the same record digests, in both implementations.

use crate::Error;

/// A table key. Python compares 1, 1.0 and True as equal dictionary keys, so
/// duplicate detection here does the same.
#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl Key {
    fn numeric(&self) -> Option<f64> {
        match self {
            Key::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Key::Int(i) => Some(*i as f64),
            Key::Float(f) => Some(*f),
            Key::Str(_) => None,
        }
    }

    fn same(&self, other: &Key) -> bool {
        match (self, other) {
            (Key::Str(a), Key::Str(b)) => a == b,
            (Key::Int(a), Key::Int(b)) => a == b,
            _ => match (self.numeric(), other.numeric()) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    /// A table whose keys are exactly 1..n.
    List(Vec<Value>),
    /// Any other table, in source order.
    Map(Vec<(Key, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(entries) => entries.iter().find_map(|(k, v)| match k {
                Key::Str(s) if s == key => Some(v),
                _ => None,
            }),
            _ => None,
        }
    }

    pub fn is_empty_map(&self) -> bool {
        matches!(self, Value::Map(entries) if entries.is_empty())
    }
}

/// Values in one save. A full 5,000-record save uses well under a million.
const MAX_NODES: usize = 4_000_000;
const MAX_DEPTH: usize = 64;

struct Parser<'a> {
    source: &'a str,
    at: usize,
    nodes: usize,
    records_span: Option<(usize, usize)>,
}

fn is_python_space(ch: char) -> bool {
    // Python's str.isspace() also counts the ASCII separators 0x1C to 0x1F.
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

impl<'a> Parser<'a> {
    fn fail<T>(&self, reason: &str) -> Result<T, Error> {
        Err(Error::SaveFormat(format!("{reason} at offset {}", self.at)))
    }

    fn bytes(&self) -> &'a [u8] {
        self.source.as_bytes()
    }

    fn peek(&self) -> Option<char> {
        self.source[self.at..].chars().next()
    }

    fn space(&mut self) {
        while let Some(ch) = self.peek() {
            if is_python_space(ch) {
                self.at += ch.len_utf8();
            } else if self.source[self.at..].starts_with("--") {
                match self.source[self.at..].find('\n') {
                    Some(end) => self.at += end + 1,
                    None => self.at = self.source.len(),
                }
            } else {
                break;
            }
        }
    }

    fn take(&mut self, expected: &str) -> Result<(), Error> {
        self.space();
        if !self.source[self.at..].starts_with(expected) {
            return self.fail(&format!("expected {expected:?}"));
        }
        self.at += expected.len();
        Ok(())
    }

    fn match_identifier(&self) -> Option<usize> {
        let bytes = self.bytes();
        if self.at < bytes.len() && is_ident_start(bytes[self.at]) {
            let mut end = self.at + 1;
            while end < bytes.len() && is_ident_char(bytes[end]) {
                end += 1;
            }
            Some(end)
        } else {
            None
        }
    }

    fn identifier(&mut self) -> Result<&'a str, Error> {
        self.space();
        match self.match_identifier() {
            Some(end) => {
                let word = &self.source[self.at..end];
                self.at = end;
                Ok(word)
            }
            None => self.fail("expected identifier"),
        }
    }

    fn quoted(&mut self) -> Result<String, Error> {
        let quote = self.bytes()[self.at];
        self.at += 1;
        let mut out = String::new();
        loop {
            let Some(ch) = self.peek() else {
                return self.fail("unfinished string");
            };
            self.at += ch.len_utf8();
            if ch as u32 == quote as u32 {
                return Ok(out);
            }
            if ch != '\\' {
                out.push(ch);
                continue;
            }
            let Some(esc) = self.peek() else {
                return self.fail("unfinished escape");
            };
            self.at += esc.len_utf8();
            if esc.is_ascii_digit() {
                // Up to three digits, read as a code point like the Python parser does.
                let start = self.at - 1;
                let bytes = self.bytes();
                while self.at < bytes.len().min(start + 3) && bytes[self.at].is_ascii_digit() {
                    self.at += 1;
                }
                let value: u32 = self.source[start..self.at].parse().unwrap_or(u32::MAX);
                if value > 255 {
                    return self.fail("invalid byte escape");
                }
                out.push(char::from_u32(value).expect("byte range is valid"));
                continue;
            }
            out.push(match esc {
                'a' => '\u{7}',
                'b' => '\u{8}',
                'f' => '\u{c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'v' => '\u{b}',
                '\\' => '\\',
                '"' => '"',
                '\'' => '\'',
                '\n' => '\n',
                _ => return self.fail("unsupported escape"),
            });
        }
    }

    fn number(&mut self) -> Result<Value, Error> {
        // -?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?
        let bytes = self.bytes();
        let start = self.at;
        let mut end = start;
        let digits = |mut i: usize| {
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            i
        };
        if end < bytes.len() && bytes[end] == b'-' {
            end += 1;
        }
        let whole = digits(end);
        if whole > end {
            end = whole;
            if end < bytes.len() && bytes[end] == b'.' {
                end = digits(end + 1);
            }
        } else if end < bytes.len() && bytes[end] == b'.' && digits(end + 1) > end + 1 {
            end = digits(end + 1);
        } else {
            return self.fail("bad number");
        }
        if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
            let mut exp = end + 1;
            if exp < bytes.len() && (bytes[exp] == b'+' || bytes[exp] == b'-') {
                exp += 1;
            }
            let exp_end = digits(exp);
            if exp_end > exp {
                end = exp_end;
            }
        }
        let text = &self.source[start..end];
        self.at = end;
        if text.contains(['.', 'e', 'E']) {
            let value: f64 = match text.parse() {
                Ok(v) => v,
                Err(_) => return self.fail("bad number"),
            };
            if !value.is_finite() {
                return self.fail("nonfinite number");
            }
            Ok(Value::Float(value))
        } else {
            match text.parse::<i64>() {
                Ok(v) => Ok(Value::Int(v)),
                Err(_) => self.fail("number out of range"),
            }
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, Error> {
        self.space();
        self.nodes += 1;
        if self.nodes > MAX_NODES || depth > MAX_DEPTH {
            return self.fail("save complexity limit exceeded");
        }
        let Some(ch) = self.peek() else {
            return self.fail("unexpected end");
        };
        match ch {
            '{' => self.table(depth + 1),
            '"' | '\'' => self.quoted().map(Value::Str),
            '-' | '.' | '0'..='9' => self.number(),
            _ => match self.identifier()? {
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                "nil" => Ok(Value::Nil),
                _ => self.fail("unexpected name"),
            },
        }
    }

    fn table(&mut self, depth: usize) -> Result<Value, Error> {
        self.take("{")?;
        let mut entries: Vec<(Key, Value)> = Vec::new();
        let mut next_index: i64 = 1;
        loop {
            self.space();
            let Some(ch) = self.peek() else {
                return self.fail("unfinished table");
            };
            if ch == '}' {
                self.at += 1;
                return Ok(normalize(entries));
            }
            let mut start = None;
            let (key, value) = if ch == '[' {
                self.at += 1;
                let key = self.value(depth)?;
                self.take("]")?;
                self.take("=")?;
                if depth == 1 && key == Value::Str("records".into()) {
                    self.space();
                    start = Some(self.at);
                }
                (key, self.value(depth)?)
            } else {
                let mark = self.at;
                let word_end = self.match_identifier();
                if let Some(end) = word_end {
                    self.at = end;
                    self.space();
                }
                if let Some(end) = word_end.filter(|_| self.peek() == Some('=')) {
                    let word = self.source[mark..end].to_string();
                    self.at += 1;
                    if depth == 1 && word == "records" {
                        self.space();
                        start = Some(self.at);
                    }
                    (Value::Str(word), self.value(depth)?)
                } else {
                    self.at = mark;
                    let key = Value::Int(next_index);
                    next_index += 1;
                    (key, self.value(depth)?)
                }
            };
            if let Some(start) = start {
                self.records_span = Some((start, self.at));
            }
            let key = match key {
                Value::Bool(b) => Key::Bool(b),
                Value::Int(i) => Key::Int(i),
                Value::Float(f) => Key::Float(f),
                Value::Str(s) => Key::Str(s),
                _ => return self.fail("invalid table key"),
            };
            if entries.iter().any(|(k, _)| k.same(&key)) {
                return self.fail("duplicate table key");
            }
            entries.push((key, value));
            self.space();
            match self.peek() {
                Some(',') | Some(';') => self.at += 1,
                Some('}') => {}
                _ => return self.fail("expected table separator"),
            }
        }
    }
}

fn normalize(entries: Vec<(Key, Value)>) -> Value {
    if !entries.is_empty() && entries.iter().all(|(k, _)| matches!(k, Key::Int(i) if *i > 0)) {
        let max = entries
            .iter()
            .map(|(k, _)| match k {
                Key::Int(i) => *i,
                _ => 0,
            })
            .max()
            .unwrap_or(0);
        if max == entries.len() as i64 {
            let mut slots: Vec<Option<Value>> = vec![None; entries.len()];
            for (k, v) in entries {
                if let Key::Int(i) = k {
                    slots[(i - 1) as usize] = Some(v);
                }
            }
            return Value::List(slots.into_iter().map(|v| v.expect("keys are 1..n")).collect());
        }
    }
    Value::Map(entries)
}

/// The parsed root table, plus the byte range of its `records` value in `source`.
pub struct Parsed {
    pub db: Value,
    pub records_span: Option<(usize, usize)>,
}

pub fn parse_save(source: &str) -> Result<Value, Error> {
    parse_save_with_span(source).map(|p| p.db)
}

pub fn parse_save_with_span(source: &str) -> Result<Parsed, Error> {
    let stripped = source.trim_start_matches('\u{feff}');
    let offset = source.len() - stripped.len();
    let mut parser = Parser { source: stripped, at: 0, nodes: 0, records_span: None };
    if parser.identifier()? != "RestedRealmCollectorDB" {
        return Err(Error::SaveFormat("not a RestedRealmCollector SavedVariables file".into()));
    }
    parser.take("=")?;
    let db = parser.value(0)?;
    parser.space();
    if parser.at != stripped.len() {
        return parser.fail("trailing code");
    }
    if !matches!(db, Value::Map(_)) {
        return Err(Error::SaveFormat("collector root must be a table".into()));
    }
    Ok(Parsed { db, records_span: parser.records_span.map(|(a, b)| (a + offset, b + offset)) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_data_types_and_escapes() {
        let db = parse_save(
            "RestedRealmCollectorDB = { [\"a\"] = {1, 2}, [\"b\"] = \"line\\ntext\", [\"c\"] = false, [\"d\"] = -2.5 } -- end",
        )
        .unwrap();
        assert_eq!(db.get("a"), Some(&Value::List(vec![Value::Int(1), Value::Int(2)])));
        assert_eq!(db.get("b"), Some(&Value::Str("line\ntext".into())));
        assert_eq!(db.get("c"), Some(&Value::Bool(false)));
        assert_eq!(db.get("d"), Some(&Value::Float(-2.5)));
    }

    #[test]
    fn code_is_rejected() {
        assert!(parse_save("RestedRealmCollectorDB = {} os.execute(\"whoami\")").is_err());
        assert!(parse_save("RestedRealmCollectorDB = { [\"x\"] = os.execute(\"whoami\") }").is_err());
        assert!(parse_save("Other = {}").is_err());
        assert!(parse_save("RestedRealmCollectorDB = { [\"records\"] = {").is_err());
    }

    #[test]
    fn duplicate_and_sparse_keys() {
        assert!(parse_save("RestedRealmCollectorDB = { [1] = 1, [1.0] = 2 }").is_err());
        assert!(parse_save("RestedRealmCollectorDB = { \"a\", [1] = \"b\" }").is_err());
        let db = parse_save("RestedRealmCollectorDB = { x = { [2] = \"b\", [3] = \"c\" } }").unwrap();
        assert!(matches!(db.get("x"), Some(Value::Map(e)) if e.len() == 2));
    }

    #[test]
    fn records_span_points_at_the_list() {
        let text = "\u{feff}RestedRealmCollectorDB = { [\"records\"] = { 1, 2 }, x = 1 }";
        let parsed = parse_save_with_span(text).unwrap();
        let (a, b) = parsed.records_span.unwrap();
        assert_eq!(&text[a..b], "{ 1, 2 }");
    }
}
