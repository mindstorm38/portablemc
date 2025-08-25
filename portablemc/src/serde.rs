//! Common serde extensions and custom types.

use std::ops::{Deref, DerefMut};
use std::fmt::Write;

use regex::Regex;


/// A regular expression serialized and deserialized to/from its string representation. 
#[derive(Debug, Clone)]
pub struct RegexString(pub Regex);

impl Deref for RegexString {
    type Target = Regex;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RegexString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl serde::Serialize for RegexString {
    
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        serializer.serialize_str(self.0.as_str())
    }

}

impl<'de> serde::Deserialize<'de> for RegexString {

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {

            type Value = RegexString;
        
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a string regex")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error, 
            {
                Regex::new(v)
                    .map(RegexString)
                    .map_err(|e| E::custom(e))
            }

        }

        deserializer.deserialize_str(Visitor)

    }

}


/// A hexadecimal, lower case, formatted bytes string.
#[derive(Debug, Clone)]
pub struct HexString<const N: usize>(pub [u8; N]);

impl<const N: usize> Deref for HexString<N> {
    type Target = [u8; N];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> DerefMut for HexString<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const N: usize> serde::Serialize for HexString<N> {

    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        let mut buf = String::new();
        for b in self.0 {
            write!(buf, "{b:02x}").unwrap();
        }
        serializer.serialize_str(&buf)
    }

}

impl<'de, const N: usize> serde::Deserialize<'de> for HexString<N> {

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {

        struct Visitor<const N: usize>;
        impl<'de, const N: usize> serde::de::Visitor<'de> for Visitor<N> {

            type Value = HexString<N>;
        
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a bytes string ({} hex characters)", N * 2)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error, 
            {
                parse_hex_bytes::<N>(v)
                    .map(HexString)
                    .ok_or_else(|| E::custom(format_args!("invalid bytes string ({} hex characters)", N * 2)))
            }

        }

        deserializer.deserialize_str(Visitor)

    }

}

/// Parse the given hex bytes string into the given destination slice, returning none if 
/// the input string cannot be parsed, is too short or too long.
pub fn parse_hex_bytes<const LEN: usize>(mut string: &str) -> Option<[u8; LEN]> {
    
    let mut dst = [0; LEN];
    for dst in &mut dst {
        if string.is_char_boundary(2) {

            let (num, rem) = string.split_at(2);
            string = rem;

            *dst = u8::from_str_radix(num, 16).ok()?;

        } else {
            return None;
        }
    }

    // Only successful if no string remains.
    string.is_empty().then_some(dst)

}

pub fn deserialize_or_empty_seq<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where 
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
    T: Default,
{

    use serde::Deserialize;
    
    #[derive(serde::Deserialize, Debug, Clone)]
    #[serde(untagged)]
    enum SomeOrSeq<T> {
        Some(T),
        Seq([(); 0]),
    }

    match SomeOrSeq::deserialize(deserializer)? {
        SomeOrSeq::Some(val) => Ok(val),
        SomeOrSeq::Seq([]) => Ok(T::default()),
    }

}
