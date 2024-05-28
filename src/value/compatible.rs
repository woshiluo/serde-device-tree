use core::{fmt, marker::PhantomData};
use serde::{de::Visitor, Deserialize};

/// Field representing compatability of a certain device in the tree.
///
/// This structure is represented in a list of string that is separated with Unicode `NUL` character.
pub struct Compatible<'a> {
    data: &'a [u8],
}

impl<'a> Compatible<'a> {
    pub fn iter(&self) -> Iter<'a> {
        Iter {
            remaining: self.data,
        }
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Compatible<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StrListVisitor<'de, 'a>(PhantomData<&'de ()>, PhantomData<Compatible<'a>>);
        impl<'de: 'a, 'a> Visitor<'de> for StrListVisitor<'de, 'a> {
            type Value = Compatible<'a>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "string list")
            }

            fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // TODO utf-8 check
                Ok(Compatible { data: v })
            }
        }
        deserializer.deserialize_bytes(StrListVisitor(PhantomData, PhantomData))
    }
}

impl fmt::Debug for Compatible<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

pub struct Iter<'a> {
    remaining: &'a [u8],
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.len() == 0 {
            return None;
        }
        let mut idx = 0;
        while let Some(byte) = self.remaining.get(idx) {
            if byte == &b'\0' {
                break;
            }
            idx += 1;
        }
        let (ans, rest) = self.remaining.split_at(idx);
        if let [0, ..] = rest {
            // skip '\0'
            self.remaining = &rest[1..];
        }
        Some(core::str::from_utf8(ans).unwrap())
    }
}
