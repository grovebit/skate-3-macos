//! List fields that a Lua script fills with a table.
//!
//! An empty Lua table has no array part, so mlua hands it to serde as an empty *map*; inside an
//! internally tagged enum such as [`crate::vm::Command`] (serde buffers the whole command first)
//! a plain `Vec<T>` then fails with "invalid type: map, expected a sequence". Every list field a
//! script can fill uses [`list`] or [`opt_list`] instead: a sequence reads as before, and a table
//! whose keys are exactly `1..n` (the empty table included) reads as the list in key order. Any
//! other table is still an error, and fields that are meant to be maps are not touched.
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::{fmt, marker::PhantomData};

/// A list field that is always present (missing, `nil` or `{}` = empty).
pub fn list<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    opt_list(d).map(Option::unwrap_or_default)
}

/// An optional list field: `nil` stays `None`, `{}` is `Some(empty)`.
pub fn opt_list<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<Vec<T>>, D::Error> {
    Option::<List<T>>::deserialize(d).map(|l| l.map(|l| l.0))
}

struct List<T>(Vec<T>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for List<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(ListVisitor(PhantomData)).map(List)
    }
}

struct ListVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for ListVisitor<T> {
    type Value = Vec<T>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a list (a table with keys 1..n, or {})")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
        let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(4096));
        while let Some(v) = seq.next_element()? {
            out.push(v);
        }
        Ok(out)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Vec<T>, A::Error> {
        let mut rows: Vec<(u64, T)> = Vec::new();
        while let Some(key) = map.next_key::<Key>()? {
            let Key::Index(i) = key else {
                return Err(de::Error::custom(format_args!("expected a list, found a table with the key {key}")));
            };
            rows.push((i, map.next_value()?));
        }
        rows.sort_by_key(|(i, _)| *i);
        for (n, (i, _)) in rows.iter().enumerate() {
            if *i != n as u64 + 1 {
                return Err(de::Error::custom(format_args!(
                    "expected a list, found a table with the key {i} (list keys run 1..n without gaps)"
                )));
            }
        }
        Ok(rows.into_iter().map(|(_, v)| v).collect())
    }
}

/// A table key: an integer list index (a Lua integer key; a JSON object's `"1"`), or anything else.
enum Key {
    Index(u64),
    Other(String),
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Key::Index(i) => write!(f, "{i}"),
            Key::Other(s) => write!(f, "{s:?}"),
        }
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct KeyVisitor;
        impl Visitor<'_> for KeyVisitor {
            type Value = Key;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a table key")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Key, E> {
                Ok(Key::Index(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Key, E> {
                Ok(u64::try_from(v).map_or_else(|_| Key::Other(v.to_string()), Key::Index))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Key, E> {
                Ok(if v.fract() == 0.0 && (0.0..9.0e15).contains(&v) { Key::Index(v as u64) } else { Key::Other(v.to_string()) })
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Key, E> {
                Ok(match v.parse::<u64>() {
                    Ok(i) if !v.starts_with('+') && (v == "0" || !v.starts_with('0')) => Key::Index(i),
                    _ => Key::Other(v.to_owned()),
                })
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Key, E> {
                Ok(Key::Other(v.to_string()))
            }
            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Key, E> {
                Ok(Key::Other(String::from_utf8_lossy(v).into_owned()))
            }
        }
        d.deserialize_any(KeyVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::{Lua, LuaSerdeExt};

    #[derive(Debug, serde::Deserialize, PartialEq)]
    #[serde(tag = "kind", deny_unknown_fields)]
    enum Tagged {
        T {
            #[serde(default, deserialize_with = "list")]
            xs: Vec<i32>,
            #[serde(default, deserialize_with = "opt_list")]
            o: Option<Vec<String>>,
        },
    }

    fn eval(src: &str) -> Result<Tagged, mlua::Error> {
        let lua = Lua::new();
        let v: mlua::Value = lua.load(src).eval().unwrap();
        lua.from_value(v)
    }

    #[test]
    fn lua_tables_read_as_lists() {
        let t = |xs: Vec<i32>, o: Option<Vec<&str>>| Tagged::T { xs, o: o.map(|o| o.into_iter().map(String::from).collect()) };
        assert_eq!(eval("return {kind='T'}").unwrap(), t(vec![], None));
        assert_eq!(eval("return {kind='T', xs={}, o={}}").unwrap(), t(vec![], Some(vec![])));
        assert_eq!(eval("return {kind='T', xs={3,1,2}, o={'a'}}").unwrap(), t(vec![3, 1, 2], Some(vec!["a"])));
        // Integer keys 1..n in any insertion order are a list.
        assert_eq!(eval("local x = {}; x[2] = 5; x[1] = 4; return {kind='T', xs=x}").unwrap(), t(vec![4, 5], None));
        // A gap, a zero key or a named key is not a list.
        for bad in ["{[2]=1}", "{[0]=1}", "{a=1}", "{[1.5]=1}", "{[-1]=1}"] {
            let e = eval(&format!("return {{kind='T', xs={bad}}}")).unwrap_err().to_string();
            assert!(e.contains("expected a list"), "{bad}: {e}");
        }
        assert!(eval("return {kind='T', xs=5}").is_err());
    }

    #[test]
    fn json_keeps_reading() {
        let j = |v| serde_json::from_value::<Tagged>(v);
        assert_eq!(j(serde_json::json!({"kind":"T","xs":[1,2],"o":null})).unwrap(), Tagged::T { xs: vec![1, 2], o: None });
        assert_eq!(j(serde_json::json!({"kind":"T","xs":{},"o":[]})).unwrap(), Tagged::T { xs: vec![], o: Some(vec![]) });
        assert_eq!(j(serde_json::json!({"kind":"T","xs":{"2":7,"1":6}})).unwrap(), Tagged::T { xs: vec![6, 7], o: None });
        assert!(j(serde_json::json!({"kind":"T","xs":{"01":1}})).is_err());
        assert!(j(serde_json::json!({"kind":"T","xs":"x"})).is_err());
    }
}
