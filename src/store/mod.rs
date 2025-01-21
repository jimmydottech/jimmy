pub mod iter;
#[cfg(feature = "levels")]
pub mod leveldb;
pub mod map;
#[cfg(feature = "rocks")]
pub mod rocksdb;

pub use map::StoreMap;
use serde::{de::DeserializeOwned, Serialize};

/// TODO(ethan): define error
/// The Store is for storing some data in embedded storage.
pub trait Store {
    type Iter<'a>: Iterator<Item = (Box<[u8]>, Box<[u8]>)>;

    fn put(key: &[u8], value: &[u8]) -> anyhow::Result<()>;

    fn get(key: &[u8]) -> anyhow::Result<Option<Vec<u8>>>;

    fn delete(key: &[u8]) -> anyhow::Result<()>;

    fn iter<'a>(prefix: &[u8]) -> Self::Iter<'a>;

    fn open_map<K: Serialize + DeserializeOwned, V: Serialize + DeserializeOwned>(
        prefix: impl AsRef<str>,
    ) -> StoreMap<K, V, Self>
    where
        Self: Sized,
    {
        StoreMap::new(prefix)
    }
}

#[cfg(feature = "levels")]
pub type LocalStore = crate::store::leveldb::LevelDB;

#[cfg(all(feature = "rocks", not(feature = "levels")))]
pub type LocalStore = crate::store::rocksdb::RocksDB;
