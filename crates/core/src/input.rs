// SPDX-License-Identifier: Apache-2.0
//! A common input boundary for all adapters. Ambiguous duplicate keys are errors.
use anyhow::{Result, ensure};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::{collections::HashSet, fmt};

pub fn parse(input: &[u8]) -> Result<Value> {
    ensure!(input.len() <= crate::MAX_INPUT_BYTES, "input too large");
    let mut parser = serde_json::Deserializer::from_slice(input);
    Unique::deserialize(&mut parser)?;
    parser.end()?;
    Ok(serde_json::from_slice(input)?)
}
struct Unique;
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(Self)
    }
}
impl<'de> Visitor<'de> for Unique {
    type Value = Self;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unambiguous JSON")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut m: M) -> Result<Self, M::Error> {
        let mut keys = HashSet::new();
        while let Some(key) = m.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON object property"));
            }
            m.next_value::<Unique>()?;
        }
        Ok(Self)
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Self, S::Error> {
        while seq.next_element::<Unique>()?.is_some() {}
        Ok(Self)
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<Self, E> {
        Ok(Self)
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self, E> {
        Ok(Self)
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self, E> {
        Ok(Self)
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self, E> {
        Ok(Self)
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self, E> {
        Ok(Self)
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self, E> {
        Ok(Self)
    }
}
