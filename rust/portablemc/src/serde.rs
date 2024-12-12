//! Common serde extensions.

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


/// A SHA-1 hash serialized and deserialized to/from has a 40 hex characters string.
#[derive(Debug, Clone)]
pub struct Sha1HashString(pub [u8; 20]);

impl Deref for Sha1HashString {
    type Target = [u8; 20];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Sha1HashString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl serde::Serialize for Sha1HashString {

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

impl<'de> serde::Deserialize<'de> for Sha1HashString {

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {

        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {

            type Value = Sha1HashString;
        
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a string sha-1 hash (40 hex characters)")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error, 
            {
                parse_hex_bytes::<20>(v)
                    .map(Sha1HashString)
                    .ok_or_else(|| E::custom("invalid sha-1 hash (40 hex characters)"))
            }

        }

        deserializer.deserialize_str(Visitor)

    }

}

/// Parse the given hex bytes string into the given destination slice, returning none if 
/// the input string cannot be parsed, is too short or too long.
pub(crate) fn parse_hex_bytes<const LEN: usize>(mut string: &str) -> Option<[u8; LEN]> {
    
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

pub(crate) fn deserialize_or_empty_seq<'de, D, T>(deserializer: D) -> Result<T, D::Error>
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
