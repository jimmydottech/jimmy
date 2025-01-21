use std::{path::PathBuf, sync::OnceLock};

use db_key::Key as DbKeyTrait;
use key::Key;
use leveldb::{
    database::Database,
    iterator::{Iterable, LevelDBIterator},
    kv::KV,
    options::{ReadOptions, WriteOptions},
};

use super::Store;
use crate::config::Config;

pub mod key;

#[derive(Clone)]
pub struct LevelDB;

impl LevelDB {
    pub fn open() -> &'static Database<Key> {
        static DB: OnceLock<Database<Key>> = OnceLock::new();

        let database = DB.get_or_init(|| {
            let mut options = leveldb::options::Options::new();
            options.create_if_missing = true;
            let path = PathBuf::from(&Config::get().store_path);
            std::fs::create_dir_all(&path).expect("Failed to create directory");
            // options.cache = Some(Cache::new(1024 * 1024));

            Database::open(&path, options).expect("Failed to open LevelDB")
        });

        database
    }
}

impl Store for LevelDB {
    type Iter<'a> = LevelDBIter<'a>;

    fn put(key: &[u8], value: &[u8]) -> anyhow::Result<()> {
        let db = LevelDB::open();
        let mut write_options = WriteOptions::new();
        write_options.sync = true;
        let key = Key::from_u8(key);
        db.put(write_options, key, value).map_err(Into::into)
    }

    fn get(key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        let db = LevelDB::open();
        let options = ReadOptions::new();
        let key = Key::from_u8(key);

        db.get(options, key).map_err(Into::into)
    }

    fn delete(key: &[u8]) -> anyhow::Result<()> {
        let db = LevelDB::open();
        let mut write_options = WriteOptions::new();
        write_options.sync = true;

        let key = Key::from_u8(key);
        db.delete(write_options, key).map_err(Into::into)
    }

    fn iter<'a>(_prefix: &[u8]) -> Self::Iter<'a> {
        let db = LevelDB::open();
        let options = ReadOptions::new();
        LevelDBIter::new(db.iter(options))
    }
}

pub struct LevelDBIter<'a> {
    db_iter: leveldb::database::iterator::Iterator<'a, Key>,
}

impl<'a> LevelDBIter<'a> {
    pub fn new(db_iter: leveldb::database::iterator::Iterator<'a, Key>) -> Self {
        Self { db_iter }
    }
}

impl<'a> Iterator for LevelDBIter<'a> {
    type Item = (Box<[u8]>, Box<[u8]>);

    fn next(&mut self) -> Option<Self::Item> {
        if !self.db_iter.valid() {
            return None;
        }

        self.db_iter.next().map(|(k, v)| {
            (
                k.as_bytes().to_vec().into_boxed_slice(),
                v.into_boxed_slice(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn init() {
        std::env::set_var("STORAGE_SERVICE_DATABASE_PATH", "/tmp/test_leveldb");
    }

    #[test]
    fn test_leveldb_op() {
        init();

        let map = LevelDB::open_map::<String, String>("test_op");
        map.insert("key".to_string(), "value".to_string()).unwrap();

        let value = map.get(&"key".to_string()).unwrap();

        assert_eq!(value, Some("value".to_string()));

        map.remove(&"key".to_string()).unwrap();

        let value = map.get(&"key".to_string()).unwrap();

        assert_eq!(value, None);

        let value = map.get(&"key".to_string()).unwrap();

        assert_eq!(value, None);
    }

    #[test]
    fn test_leveldb_iter() {
        init();

        let map = LevelDB::open_map::<String, String>("test_iter2");

        let mut data = HashMap::new();

        for i in 0..10 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);

            map.insert(key.clone(), value.clone()).unwrap();
            data.insert(key, value);
        }

        let mut iter = map.iter();

        for (key, value) in iter.by_ref() {
            let key = key.into_owned();
            let value = value.into_owned();

            assert_eq!(data.remove(&key), Some(value));
        }

        assert_eq!(iter.next(), None);
    }
}
