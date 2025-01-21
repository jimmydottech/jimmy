use std::borrow::Cow;

use serde::{de::DeserializeOwned, Serialize};
use tracing::trace;

pub struct MapIter<
    'a,
    K: 'a + Serialize + DeserializeOwned,
    V: 'a + Serialize + DeserializeOwned,
    I: Iterator<Item = (Box<[u8]>, Box<[u8]>)>,
> {
    prefix: Vec<u8>,
    iter: I,
    phantom: std::marker::PhantomData<(&'a K, &'a V)>,
}

impl<
        'a,
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
        I: Iterator<Item = (Box<[u8]>, Box<[u8]>)>,
    > MapIter<'a, K, V, I>
{
    pub fn new(prefix: &[u8], iter: I) -> Self {
        Self {
            prefix: prefix.to_vec(),
            iter,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<
        'a,
        K: Serialize + DeserializeOwned + Clone,
        V: Serialize + DeserializeOwned + Clone,
        I: Iterator<Item = (Box<[u8]>, Box<[u8]>)>,
    > Iterator for MapIter<'a, K, V, I>
{
    type Item = (Cow<'a, K>, Cow<'a, V>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((key, value)) = self.iter.next() {
            // Deserialize the key and value.
            let key = bincode::deserialize(&key[self.prefix.len()..])
                .map_err(|e| {
                    trace!("Store Iter deserialize(key) error: {e}");
                })
                .ok()?;

            let value = bincode::deserialize(&value)
                .map_err(|e| {
                    trace!("Store Iter deserialize(value) error: {e}");
                })
                .ok()?;
            Some((Cow::Owned(key), Cow::Owned(value)))
        } else {
            None
        }
    }
}
