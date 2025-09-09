use snarkvm_utilities::{FromBytesUnchecked, FromBytesVisitor};

use serde::{Deserializer, de};

/// (De-)serializer for ledger data that skips some checks.
pub struct StorageFormat {}

impl StorageFormat {
    pub fn deserialize<T: FromBytesUnchecked>(data: &[u8]) -> Result<T, bincode::Error> {
        let mut buffer = Vec::with_capacity(32);

        let mut deserializer = bincode::Deserializer::from_slice(data, bincode::DefaultOptions::new());
        let typename = std::any::type_name::<T>();

        deserializer.deserialize_bytes(FromBytesVisitor::new(&mut buffer, typename))?;
        FromBytesUnchecked::read_le_unchecked(&*buffer).map_err(de::Error::custom)
    }
}
