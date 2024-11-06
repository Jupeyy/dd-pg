use anyhow::anyhow;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

/// This type is similar to a [`std::time::Duration`].
/// It always gives the Duration from the UNIX_EPOCH in UTC.
#[derive(Debug, Hiarc, Default, Clone, Copy, Serialize, Deserialize)]
pub struct UnixUtcTimestamp {
    pub secs: u64,
    pub subsec_nanos: u32,
}

impl UnixUtcTimestamp {
    pub fn to_chrono(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        let secs: Option<i64> = self.secs.try_into().ok();
        secs.and_then(|secs| {
            <chrono::DateTime<chrono::Utc>>::from_timestamp(secs, self.subsec_nanos)
        })
    }

    pub fn from_chrono(utc_timestamp: chrono::DateTime<chrono::Utc>) -> Option<Self> {
        let secs: Option<u64> = utc_timestamp.timestamp().try_into().ok();
        secs.map(|secs| Self {
            secs,
            subsec_nanos: utc_timestamp.timestamp_subsec_nanos(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DbType {
    I16(i16),
    I32(i32),
    I64(i64),
    // Other int types are not supported by all backends
    F32(f32),
    F64(f64),
    Bool(bool),
    String(String),
    Vec(Vec<u8>),
    DateTime(UnixUtcTimestamp),
}

impl TryInto<i16> for DbType {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<i16, Self::Error> {
        Ok(if let Self::I16(v) = self {
            v
        } else {
            return Err(anyhow!("not of type i16"));
        })
    }
}

impl TryInto<i32> for DbType {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<i32, Self::Error> {
        Ok(if let Self::I32(v) = self {
            v
        } else {
            return Err(anyhow!("not of type i32"));
        })
    }
}

impl TryInto<i64> for DbType {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<i64, Self::Error> {
        Ok(if let Self::I64(v) = self {
            v
        } else {
            return Err(anyhow!("not of type i64"));
        })
    }
}

impl TryInto<f32> for DbType {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<f32, Self::Error> {
        Ok(if let Self::F32(v) = self {
            v
        } else {
            return Err(anyhow!("not of type f32"));
        })
    }
}

impl TryInto<f64> for DbType {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<f64, Self::Error> {
        Ok(if let Self::F64(v) = self {
            v
        } else {
            return Err(anyhow!("not of type f64"));
        })
    }
}

impl TryInto<bool> for DbType {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        Ok(if let Self::Bool(v) = self {
            v
        } else {
            return Err(anyhow!("not of type bool"));
        })
    }
}

impl TryInto<String> for DbType {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<String, Self::Error> {
        Ok(if let Self::String(v) = self {
            v
        } else {
            return Err(anyhow!("not of type String"));
        })
    }
}

impl TryInto<Vec<u8>> for DbType {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(if let Self::Vec(v) = self {
            v
        } else {
            return Err(anyhow!("not of type Vec<u8>"));
        })
    }
}

impl TryInto<UnixUtcTimestamp> for DbType {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<UnixUtcTimestamp, Self::Error> {
        Ok(if let Self::DateTime(v) = self {
            v
        } else {
            return Err(anyhow!("not of type UnixTimestamp"));
        })
    }
}

impl From<i16> for DbType {
    fn from(value: i16) -> Self {
        Self::I16(value)
    }
}

impl From<i32> for DbType {
    fn from(value: i32) -> Self {
        Self::I32(value)
    }
}

impl From<i64> for DbType {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<f32> for DbType {
    fn from(value: f32) -> Self {
        Self::F32(value)
    }
}

impl From<f64> for DbType {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

impl From<bool> for DbType {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<String> for DbType {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for DbType {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<Vec<u8>> for DbType {
    fn from(value: Vec<u8>) -> Self {
        Self::Vec(value)
    }
}

impl From<&[u8]> for DbType {
    fn from(value: &[u8]) -> Self {
        Self::Vec(value.to_vec())
    }
}

impl From<UnixUtcTimestamp> for DbType {
    fn from(value: UnixUtcTimestamp) -> Self {
        Self::DateTime(value)
    }
}
