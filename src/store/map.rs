use std::marker::PhantomData;

use serde::{de::DeserializeOwned, Serialize};

use super::{iter::MapIter, Store};

/// The StoreMap is for storing some data in embedded storage.
#[derive(Clone)]
pub struct StoreMap<K: Serialize + DeserializeOwned, V: Serialize + DeserializeOwned, S: Store> {
    pub prefix: Box<[u8]>,
    phantom: PhantomData<(K, V, S)>,
}

impl<K: Serialize + DeserializeOwned, V: Serialize + DeserializeOwned, S: Store> StoreMap<K, V, S> {
    pub fn new(prefix: impl AsRef<str>) -> Self {
        Self {
            prefix: prefix.as_ref().as_bytes().to_vec().into_boxed_slice(),
            phantom: PhantomData,
        }
    }

    pub fn insert(&self, key: K, value: V) -> anyhow::Result<()> {
        let key = bincode::serialize(&key)?;
        let value = bincode::serialize(&value)?;
        S::put(&[&self.prefix, key.as_slice()].concat(), &value)?;

        Ok(())
    }

    pub fn get(&self, key: &K) -> anyhow::Result<Option<V>> {
        let key = bincode::serialize(&key)?;
        let value = S::get(&[&self.prefix, key.as_slice()].concat())?;
        match value {
            Some(value) => Ok(Some(bincode::deserialize(&value)?)),
            None => Ok(None),
        }
    }

    pub fn remove(&self, key: &K) -> anyhow::Result<()> {
        let key = bincode::serialize(&key)?;
        S::delete(&[&self.prefix, key.as_slice()].concat())
    }

    pub fn iter(&self) -> MapIter<K, V, S::Iter<'_>> {
        let iter = S::iter(self.prefix.as_ref());
        MapIter::new(&self.prefix, iter)
    }
}
