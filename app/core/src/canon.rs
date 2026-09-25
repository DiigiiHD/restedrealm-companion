//! Canonical JSON, byte-identical to Python's
//! `json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))`.
//!
//! Record digests are SHA-256 over this text, and the server and the pilot queue
//! already hold digests made by the Python companion, so the output must match
//! it exactly: key order, float spelling and string escaping.

use crate::lua::{Key, Value};
use crate::Error;
use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Python's `repr(float)`: shortest round-trip digits, exponent form below 1e-4
/// or from 1e16, otherwise fixed with at least one decimal.
pub fn python_float(x: f64) -> String {
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    // The fewest correctly rounded significant digits that read back as `x`.
    // Rust's shortest `{:e}` can pick a different 17-digit spelling than
    // Python, so round explicitly at each precision instead.
    let sci = (0..17)
        .map(|precision| format!("{x:.precision$e}"))
        .find(|text| text.parse::<f64>() == Ok(x))
        .unwrap_or_else(|| format!("{x:.16e}"));
    let (mantissa, exp) = sci.split_once('e').expect("exponent form");
    let exp: i32 = exp.parse().expect("integer exponent");
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if !(-4..16).contains(&exp) {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        out.push_str(&format!("{:02}", exp.abs()));
    } else {
        let point = exp + 1;
        if point <= 0 {
            out.push_str("0.");
            out.push_str(&"0".repeat((-point) as usize));
            out.push_str(&digits);
        } else if point as usize >= digits.len() {
            out.push_str(&digits);
            out.push_str(&"0".repeat(point as usize - digits.len()));
            out.push_str(".0");
        } else {
            out.push_str(&digits[..point as usize]);
            out.push('.');
            out.push_str(&digits[point as usize..]);
        }
    }
    out
}

pub fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn key_text(key: &Key) -> String {
    match key {
        Key::Str(s) => s.clone(),
        Key::Int(i) => i.to_string(),
        Key::Float(f) => python_float(*f),
        Key::Bool(b) => b.to_string(),
    }
}

fn key_number(key: &Key) -> f64 {
    match key {
        Key::Bool(b) => f64::from(u8::from(*b)),
        Key::Int(i) => *i as f64,
        Key::Float(f) => *f,
        Key::Str(_) => unreachable!("strings are sorted separately"),
    }
}

fn write_lua(out: &mut String, value: &Value) -> Result<(), Error> {
    match value {
        Value::Nil => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => out.push_str(&python_float(*f)),
        Value::Str(s) => write_string(out, s),
        Value::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_lua(out, item)?;
            }
            out.push(']');
        }
        Value::Map(entries) => {
            // Python sorts the original keys: strings by code point, numbers by
            // value, and refuses to compare a string with a number.
            let strings = entries.iter().filter(|(k, _)| matches!(k, Key::Str(_))).count();
            if strings != 0 && strings != entries.len() {
                return Err(Error::SaveFormat("table mixes text and number keys".into()));
            }
            let mut sorted: Vec<&(Key, Value)> = entries.iter().collect();
            if strings == 0 {
                sorted.sort_by(|a, b| key_number(&a.0).total_cmp(&key_number(&b.0)));
            } else {
                sorted.sort_by(|a, b| key_text(&a.0).cmp(&key_text(&b.0)));
            }
            out.push('{');
            for (i, (key, item)) in sorted.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, &key_text(key));
                out.push(':');
                write_lua(out, item)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// Canonical text of a parsed save value.
pub fn lua_to_canonical(value: &Value) -> Result<String, Error> {
    let mut out = String::new();
    write_lua(&mut out, value)?;
    Ok(out)
}

fn write_json(out: &mut String, value: &serde_json::Value) {
    use serde_json::Value as J;
    match value {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.push_str(&i.to_string());
            } else if let Some(u) = n.as_u64() {
                out.push_str(&u.to_string());
            } else {
                out.push_str(&python_float(n.as_f64().unwrap_or(0.0)));
            }
        }
        J::String(s) => write_string(out, s),
        J::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(out, item);
            }
            out.push(']');
        }
        J::Object(map) => {
            // serde_json's default map is ordered by key, which is code point order.
            out.push('{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_json(out, item);
            }
            out.push('}');
        }
    }
}

/// Canonical text of a JSON value, as Python re-dumps a loaded record.
pub fn json_to_canonical(value: &serde_json::Value) -> String {
    let mut out = String::new();
    write_json(&mut out, value);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_match_python_repr() {
        for (x, want) in [
            (1.5, "1.5"),
            (100.0, "100.0"),
            (-2.5, "-2.5"),
            (0.1, "0.1"),
            (1e-4, "0.0001"),
            (1e-5, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (1e16, "1e+16"),
            (1234567890123456.0, "1234567890123456.0"),
            (123456789012345678.0, "1.2345678901234568e+17"),
            (1e300, "1e+300"),
            (0.30000000000000004, "0.30000000000000004"),
            (-0.0, "-0.0"),
        ] {
            assert_eq!(python_float(x), want, "{x:e}");
        }
    }

    #[test]
    fn strings_escape_like_python() {
        let mut out = String::new();
        write_string(&mut out, "a\"b\\c\nd\u{1}é\u{7f}");
        assert_eq!(out, "\"a\\\"b\\\\c\\nd\\u0001é\u{7f}\"");
    }

    #[test]
    fn number_keys_sort_by_value() {
        let value = crate::lua::parse_save("RestedRealmCollectorDB = { [10] = 1, [2] = 2 }").unwrap();
        assert_eq!(lua_to_canonical(&value).unwrap(), "{\"2\":2,\"10\":1}");
        let mixed = crate::lua::parse_save("RestedRealmCollectorDB = { [10] = 1, a = 2 }").unwrap();
        assert!(lua_to_canonical(&mixed).is_err());
    }
}

#[cfg(test)]
mod python_fixture {
    /// `RRC_FLOAT_FIXTURE=file cargo test -- --ignored`, where each line is
    /// `<f64 bits> <Python repr>`, generated by Python on the same values.
    #[test]
    #[ignore]
    fn floats_match_python_fixture() {
        let path = std::env::var("RRC_FLOAT_FIXTURE").expect("RRC_FLOAT_FIXTURE");
        let text = std::fs::read_to_string(path).unwrap();
        let mut checked = 0;
        for line in text.lines() {
            let (bits, want) = line.split_once(' ').unwrap();
            let x = f64::from_bits(bits.parse().unwrap());
            assert_eq!(super::python_float(x), want, "bits {bits}");
            checked += 1;
        }
        assert!(checked > 0);
    }
}
