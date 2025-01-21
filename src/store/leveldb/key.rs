use bytes::Bytes;
pub use db_key::Key as DbKeyTrait;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key(Bytes);

impl DbKeyTrait for Key {
    fn from_u8(key: &[u8]) -> Self {
        Self(Bytes::copy_from_slice(key))
    }

    fn as_slice<T, F: Fn(&[u8]) -> T>(&self, f: F) -> T {
        f(&self.0)
    }
}

impl Key {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
