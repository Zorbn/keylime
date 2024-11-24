use std::{fmt, marker::PhantomData};

use serde::{
    de::{MapAccess, Visitor},
    Deserialize, Deserializer,
};

struct PairVisitor<K, V> {
    phantom: PhantomData<(K, V)>,
}

impl<'de, K, V> Visitor<'de> for PairVisitor<K, V>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    type Value = Vec<(K, V)>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map")
    }

    fn visit_map<T>(self, mut access: T) -> Result<Vec<(K, V)>, T::Error>
    where
        T: MapAccess<'de>,
    {
        let mut result = if let Some(size) = access.size_hint() {
            Vec::with_capacity(size)
        } else {
            Vec::new()
        };

        while let Some((key, value)) = access.next_entry()? {
            result.push((key, value));
        }

        Ok(result)
    }
}

pub fn deserialize_pairs<'de, D, K, V>(deserializer: D) -> Result<Vec<(K, V)>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    deserializer.deserialize_map(PairVisitor::<K, V> {
        phantom: PhantomData,
    })
}
