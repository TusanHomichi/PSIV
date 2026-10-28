//! The pack's JSON text, byte for byte what the Python builder writes.
//!
//! Every pack JSON file is `json.dumps(payload, indent=2, sort_keys=True) +
//! "\n"`, UTF-8 (`psiv_tools/pack.py`, `_write_json`). One file,
//! `presentation/panels.json`, keeps insertion order instead (no `sort_keys`),
//! so [`Json::Object`] holds its pairs in order and the writer sorts them
//! only when asked.
//!
//! What has to match Python's encoder:
//!
//! - Layout: two-space indent, `", "`-free separators (`","` at line ends and
//!   `": "` after a key), `[]` and `{}` for empty containers.
//! - Sorting: Python sorts `str` keys by code point, which is the byte order
//!   of their UTF-8, so `str::cmp` is the same order. A Python dict with `int`
//!   keys sorts numerically before its keys become strings; no pack file has
//!   one (every object in a full pack was checked, 2026-10-28), and a section
//!   that needs one sorts its pairs itself and writes it unsorted.
//! - Strings: `ensure_ascii`, so everything outside `' '..='~'` is escaped:
//!   `\"`, `\\`, `\n`, `\r`, `\t`, `\b`, `\f`, and `\u00XX` (lowercase hex)
//!   for the rest, with a surrogate pair above U+FFFF. DEL (0x7F) is escaped.
//! - Floats: `float.__repr__`, the shortest digits that round-trip, in fixed
//!   notation when the decimal exponent is in `-4 < decpt <= 16` and as
//!   `d.ddde±XX` otherwise; `NaN`, `Infinity` and `-Infinity` as Python writes
//!   them (`allow_nan` is on by default).
//!
//! `rust/psiv-pack/tests/fixtures/harness.json` holds cases Python wrote
//! (`tools/pack_parity.py --write-fixtures`); `tests/harness.rs` holds this
//! writer to them.

use std::fmt::Write as _;

/// A JSON value with Python's number kinds and an ordered object.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// Python `int`; every pack integer fits.
    Int(i128),
    /// Python `float`.
    Float(f64),
    Str(String),
    Array(Vec<Json>),
    /// Pairs in insertion order; [`dumps`] sorts them when `sort_keys` is set.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// An empty object, for building one pair at a time with [`Json::with`].
    pub fn object() -> Self {
        Json::Object(Vec::new())
    }

    /// This object with `key` set to `value` (replacing an earlier value, as
    /// a Python dict assignment does, in the earlier key's position).
    pub fn with(mut self, key: impl Into<String>, value: impl Into<Json>) -> Self {
        self.insert(key, value);
        self
    }

    /// Set `key` on an object. Panics on a non-object: that is a bug in the
    /// section building it, not a property of the ROM.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Json>) {
        let Json::Object(pairs) = self else {
            panic!("Json::insert on a non-object");
        };
        let key = key.into();
        let value = value.into();
        match pairs.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => pairs.push((key, value)),
        }
    }
}

macro_rules! from_int {
    ($($t:ty),*) => {$(
        impl From<$t> for Json {
            fn from(value: $t) -> Self {
                Json::Int(i128::from(value))
            }
        }
    )*};
}
from_int!(i8, i16, i32, i64, u8, u16, u32, u64);

impl From<usize> for Json {
    fn from(value: usize) -> Self {
        Json::Int(value as i128)
    }
}

impl From<bool> for Json {
    fn from(value: bool) -> Self {
        Json::Bool(value)
    }
}

impl From<f64> for Json {
    fn from(value: f64) -> Self {
        Json::Float(value)
    }
}

impl From<&str> for Json {
    fn from(value: &str) -> Self {
        Json::Str(value.to_owned())
    }
}

impl From<String> for Json {
    fn from(value: String) -> Self {
        Json::Str(value)
    }
}

impl<T: Into<Json>> From<Vec<T>> for Json {
    fn from(values: Vec<T>) -> Self {
        Json::Array(values.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(value: Option<T>) -> Self {
        value.map_or(Json::Null, Into::into)
    }
}

/// A `serde_json` value converts as its text would parse in Python: an
/// integer stays an `int`, any other number is a `float`. Its objects are
/// already sorted (`serde_json::Map` is a `BTreeMap` here).
impl From<&serde_json::Value> for Json {
    fn from(value: &serde_json::Value) -> Self {
        use serde_json::Value;
        match value {
            Value::Null => Json::Null,
            Value::Bool(b) => Json::Bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Json::Int(i128::from(i))
                } else if let Some(u) = n.as_u64() {
                    Json::Int(i128::from(u))
                } else {
                    Json::Float(n.as_f64().unwrap_or(f64::NAN))
                }
            }
            Value::String(s) => Json::Str(s.clone()),
            Value::Array(items) => Json::Array(items.iter().map(Json::from).collect()),
            Value::Object(map) => Json::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), Json::from(v)))
                    .collect(),
            ),
        }
    }
}

impl From<serde_json::Value> for Json {
    fn from(value: serde_json::Value) -> Self {
        Json::from(&value)
    }
}

/// `json.dumps(value, indent=2, sort_keys=sort_keys)`.
pub fn dumps(value: &Json, sort_keys: bool) -> String {
    let mut out = String::new();
    write_value(&mut out, value, sort_keys, 0);
    out
}

/// A pack file's bytes: [`dumps`] sorted, plus the final newline.
pub fn pack_bytes(value: &Json) -> Vec<u8> {
    let mut text = dumps(value, true);
    text.push('\n');
    text.into_bytes()
}

fn indent(out: &mut String, level: usize) {
    out.push('\n');
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn write_value(out: &mut String, value: &Json, sort_keys: bool, level: usize) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Json::Float(f) => out.push_str(&float_repr(*f)),
        Json::Str(s) => write_str(out, s),
        Json::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                indent(out, level + 1);
                write_value(out, item, sort_keys, level + 1);
            }
            indent(out, level);
            out.push(']');
        }
        Json::Object(pairs) => {
            if pairs.is_empty() {
                out.push_str("{}");
                return;
            }
            let mut order: Vec<&(String, Json)> = pairs.iter().collect();
            if sort_keys {
                order.sort_by(|a, b| a.0.cmp(&b.0));
            }
            out.push('{');
            for (i, (key, item)) in order.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                indent(out, level + 1);
                write_str(out, key);
                out.push_str(": ");
                write_value(out, item, sort_keys, level + 1);
            }
            indent(out, level);
            out.push('}');
        }
    }
}

/// A string as `json.dumps` writes it with `ensure_ascii` on.
fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
}

/// Python's `float.__repr__` (`float_repr_style == 'short'`).
pub fn float_repr(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    // Rust's `{:e}` is the shortest round-trip digits, as Python's repr is:
    // "-1.5e-5", "1e16".
    let sci = format!("{x:e}");
    let (sign, sci) = match sci.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", sci.as_str()),
    };
    let (mantissa, exponent) = sci.split_once('e').expect("{:e} has an exponent");
    let exponent: i32 = exponent.parse().expect("{:e} exponent is an integer");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let decpt = exponent + 1;
    let mut out = String::from(sign);
    if -4 < decpt && decpt <= 16 {
        let n = digits.len() as i32;
        if decpt <= 0 {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-decpt) as usize));
            out.push_str(&digits);
        } else if decpt >= n {
            out.push_str(&digits);
            out.extend(std::iter::repeat_n('0', (decpt - n) as usize));
            out.push_str(".0");
        } else {
            out.push_str(&digits[..decpt as usize]);
            out.push('.');
            out.push_str(&digits[decpt as usize..]);
        }
    } else {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let _ = write!(
            out,
            "e{}{:02}",
            if exponent < 0 { '-' } else { '+' },
            exponent.abs()
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_python_indent_two() {
        let value = Json::object()
            .with("b", vec![1, 2])
            .with("a", Json::object())
            .with("c", Vec::<Json>::new());
        assert_eq!(
            dumps(&value, true),
            "{\n  \"a\": {},\n  \"b\": [\n    1,\n    2\n  ],\n  \"c\": []\n}"
        );
        assert_eq!(
            dumps(&value, false),
            "{\n  \"b\": [\n    1,\n    2\n  ],\n  \"a\": {},\n  \"c\": []\n}"
        );
    }

    #[test]
    fn insert_replaces_in_place() {
        let value = Json::object().with("x", 1).with("y", 2).with("x", 3);
        assert_eq!(dumps(&value, false), "{\n  \"x\": 3,\n  \"y\": 2\n}");
    }

    #[test]
    fn floats_follow_python_repr() {
        for (x, text) in [
            (1.0, "1.0"),
            (0.5, "0.5"),
            (1e-4, "0.0001"),
            (1e-5, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (1.2345e22, "1.2345e+22"),
            (1e100, "1e+100"),
            (-2.5, "-2.5"),
            (0.1 + 0.2, "0.30000000000000004"),
            (-0.0, "-0.0"),
        ] {
            assert_eq!(float_repr(x), text, "{x:e}");
        }
    }

    #[test]
    fn strings_escape_like_ensure_ascii() {
        let mut out = String::new();
        write_str(&mut out, "a\"\\\n\r\t\u{8}\u{c}\u{1}\u{7f}é\u{1F600}~ ");
        assert_eq!(
            out,
            "\"a\\\"\\\\\\n\\r\\t\\b\\f\\u0001\\u007f\\u00e9\\ud83d\\ude00~ \""
        );
    }
}
