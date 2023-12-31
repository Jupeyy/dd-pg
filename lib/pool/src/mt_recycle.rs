use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use bincode::{BorrowDecode, Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::traits::Recyclable;

#[derive(Debug)]
pub struct Recycle<T: Recyclable + Send> {
    pub(crate) pool: Option<Arc<spin::Mutex<Vec<T>>>>,
    pub(crate) item: T,
}

impl<T: Recyclable + Send> Drop for Recycle<T> {
    fn drop(&mut self) {
        if let Some(pool) = &self.pool {
            let mut repl = T::new();
            std::mem::swap(&mut self.item, &mut repl);
            repl.reset();
            pool.lock().push(repl);
        }
    }
}

impl<T: Recyclable + Send> Recycle<T> {
    pub fn new_without_pool() -> Self {
        Self {
            pool: None,
            item: T::new(),
        }
    }

    pub fn take(mut self) -> T {
        let mut repl = T::new();
        self.pool = None;
        std::mem::swap(&mut self.item, &mut repl);
        repl
    }
}

impl<T: Recyclable + Send> Deref for Recycle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<T: Recyclable + Send> DerefMut for Recycle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}

impl<T: Serialize + Recyclable + Send> Serialize for Recycle<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.item.serialize(serializer)
    }
}

impl<'de, T: Send> Deserialize<'de> for Recycle<T>
where
    T: Deserialize<'de> + Recyclable,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self {
            item: T::deserialize(deserializer)?,
            pool: None,
        })
    }
}

impl<T: Serialize + Recyclable + Send> Encode for Recycle<T> {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        let conf = *encoder.config();
        bincode::serde::encode_into_writer(self, encoder.writer(), conf)?;
        Ok(())
    }
}

impl<T: Send> Decode for Recycle<T>
where
    T: for<'de> Deserialize<'de> + Recyclable,
{
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let conf = *decoder.config();
        let res_decode = bincode::serde::decode_from_reader::<Self, _, _>(decoder.reader(), conf)?;
        Ok(res_decode)
    }
}

impl<'de, T: Send> BorrowDecode<'de> for Recycle<T>
where
    T: for<'a> Deserialize<'a> + Recyclable,
{
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let conf = *decoder.config();
        let res_decode = bincode::serde::decode_from_reader::<Self, _, _>(decoder.reader(), conf)?;
        Ok(res_decode)
    }
}
