use std::time::Duration;

use anyhow::anyhow;
use serde::Deserialize;

#[derive(Debug)]
pub enum ConfigValue {
    Int {
        min: i64,
        max: u64,
    },
    Float {
        min: f64,
        max: f64,
    },
    String {
        min_length: usize,
        max_length: usize,
    },
    StringOfList {
        allowed_values: Vec<String>,
    },
    Array {
        val_ty: Box<ConfigValue>,
    },
    /// basically { "name": any, "name2": any }, useful for e.g. a hashmap
    JSONRecord {
        val_ty: Box<ConfigValue>,
    },
    /// simply an unfinished console name
    Struct {
        attributes: Vec<ConfigValueAttr>,
    },
}

#[derive(Debug)]
pub struct ConfigValueAttr {
    pub name: String,
    pub val: ConfigValue,
}

pub trait ConfigInterface {
    /// structs might overwrite certain values of
    /// the config values attributes
    fn conf_value() -> ConfigValue;

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()>;
}

impl ConfigInterface for String {
    fn conf_value() -> ConfigValue {
        ConfigValue::String {
            min_length: 0,
            max_length: usize::MAX,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for Duration {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: 0,
            max: u64::MAX,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = Duration::from_millis(val.parse()?);
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl<'de, V: ConfigInterface + Deserialize<'de>> ConfigInterface for Vec<V> {
    fn conf_value() -> ConfigValue {
        ConfigValue::Array {
            val_ty: Box::new(V::conf_value()),
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = Self::deserialize(serde_json::to_value(val)?)?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for bool {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int { min: 0, max: 1 }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for u8 {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: Self::MIN as i64,
            max: Self::MAX as u64,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for i8 {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: Self::MIN as i64,
            max: Self::MAX as u64,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for u16 {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: Self::MIN as i64,
            max: Self::MAX as u64,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for i16 {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: Self::MIN as i64,
            max: Self::MAX as u64,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for u32 {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: Self::MIN as i64,
            max: Self::MAX as u64,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for i32 {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: Self::MIN as i64,
            max: Self::MAX as u64,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for u64 {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: Self::MIN as i64,
            max: Self::MAX as u64,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for i64 {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: Self::MIN as i64,
            max: Self::MAX as u64,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for f32 {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: Self::MIN as i64,
            max: Self::MAX as u64,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

impl ConfigInterface for f64 {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int {
            min: Self::MIN as i64,
            max: Self::MAX as u64,
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = val.parse()?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}
