use std::{path::PathBuf, sync::OnceLock};

use super::Store;
use crate::config::Config;

#[derive(Clone)]
pub struct RocksDB;

impl RocksDB {
    pub fn open() -> &'static rocksdb::DB {
        static DB: OnceLock<rocksdb::DB> = OnceLock::new();

        // Retrieve the database.
        let database = DB.get_or_init(|| {
            // Customize database options.
            let mut options = rocksdb::Options::default();
            options.set_compression_type(rocksdb::DBCompressionType::Lz4);
            let path = PathBuf::from(&Config::get().store_path);

            {
                options.increase_parallelism(2);
                options.set_max_background_jobs(4);
                options.create_if_missing(true);

                rocksdb::DB::open(&options, path).expect("Failed to open RocksDB")
            }
        });

        database
    }
}

impl Store for RocksDB {
    type Iter<'a> = RocksDBIter<'a>;

    fn put(key: &[u8], value: &[u8]) -> anyhow::Result<()> {
        RocksDB::open().put(key, value).map_err(Into::into)
    }

    fn get(key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        RocksDB::open().get(key).map_err(Into::into)
    }

    fn delete(key: &[u8]) -> anyhow::Result<()> {
        RocksDB::open().delete(key).map_err(Into::into)
    }

    fn iter<'a>(prefix: &[u8]) -> Self::Iter<'a> {
        RocksDBIter::new(RocksDB::open().prefix_iterator(prefix))
    }
}

pub struct RocksDBIter<'a> {
    db_iter: rocksdb::DBRawIterator<'a>,
}

impl<'a> RocksDBIter<'a> {
    pub fn new(db_iter: rocksdb::DBIterator<'a>) -> Self {
        Self {
            db_iter: db_iter.into(),
        }
    }
}

impl<'a> Iterator for RocksDBIter<'a> {
    type Item = (Box<[u8]>, Box<[u8]>);

    fn next(&mut self) -> Option<Self::Item> {
        if !self.db_iter.valid() {
            return None;
        }

        let (key, value) = self.db_iter.item()?;

        let (key, value) = (
            key.to_vec().into_boxed_slice(),
            value.to_vec().into_boxed_slice(),
        );
        self.db_iter.next();

        Some((key, value))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn init() {
        // remove tmp
        // std::fs::remove_dir_all("/tmp/test_rocksdb").ok();
        std::env::set_var("STORAGE_SERVICE_DATABASE_PATH", "/tmp/test_rocksdb");
    }

    #[test]
    fn test_rocksdb_op() {
        init();

        let map = RocksDB::open_map::<String, String>("test_op");
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
    fn test_rocksdb_iter() {
        init();

        let map = RocksDB::open_map::<String, String>("test_iter2");

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
