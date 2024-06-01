use std::{
    collections::HashMap,
    num::{ParseFloatError, ParseIntError},
    str::ParseBoolError,
    time::Duration,
};

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigFromStrPathErr {
    #[error("Expected end of path, but found {0}")]
    EndOfPath(String),
    #[error("Value {path:?} not found in the allowed names: {allowed_paths:?}")]
    PathNotFound {
        path: String,
        allowed_paths: Vec<String>,
    },
    #[error("Failed to parse value: {0}")]
    ParsingErr(String),
    #[error("Validation failed: {0}")]
    ValidationError(String),
    // a fatal error, but not on the highest level
    #[error("{0}")]
    FatalErr(String),
}

#[derive(Error, Debug)]
pub enum ConfigFromStrErr {
    #[error("{0}")]
    PathErr(ConfigFromStrPathErr),
    #[error("{0}")]
    FatalErr(String),
}

#[derive(Debug, Clone)]
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
        min_length: usize,
        max_length: usize,
    },
    /// Basically { "name": any, "name2": any }, useful for e.g. a hashmap.
    /// However numbers as first letters are not allowed!
    JsonLikeRecord {
        val_ty: Box<ConfigValue>,
    },
    /// A container of console variables.
    Struct {
        attributes: Vec<ConfigValueAttr>,
        aliases: Vec<(String, String)>,
        name: String,
    },
}

#[derive(Debug, Clone)]
pub struct ConfigValueAttr {
    pub name: String,
    pub val: ConfigValue,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigFromStrFlags {
    Push = 1 << 0,
    Pop = 1 << 1,
    Rem = 1 << 2,
}

pub trait ConfigInterface {
    /// structs might overwrite certain values of
    /// the config values attributes
    fn conf_value() -> ConfigValue;

    /// sets the config value from a string
    /// takes path. which is the full path separated by `.`
    /// an optional modifier, which is only intersting for internal logic (e.g. array indices, hashmap indices)
    /// and optionally the value represented in a string
    /// always returns the current value as a string representation
    /// flags are a arbitrary type, for internal types see `ConfigFromStrFlags`
    fn try_set_from_str(
        &mut self,
        path: String,
        modifier: Option<String>,
        val: Option<String>,
        conf_val: Option<&ConfigValue>,
        flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr>;
}

impl ConfigInterface for String {
    fn conf_value() -> ConfigValue {
        ConfigValue::String {
            min_length: 0,
            max_length: usize::MAX,
        }
    }

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                // validate
                if !if let Some(conf_val) = conf_val {
                    if let ConfigValue::String {
                        min_length,
                        max_length,
                    } = conf_val
                    {
                        let char_count = val.chars().count();
                        if char_count < *min_length {
                            false
                        } else if char_count >= *max_length {
                            false
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                } else {
                    true
                } {
                    return Err(ConfigFromStrErr::PathErr(
                        ConfigFromStrPathErr::ValidationError(format!(
                            "The min/max length of the string was reached"
                        )),
                    ));
                }
                *self = val;
            }
            Ok(self.clone())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = Duration::from_millis(val.parse().map_err(|err: ParseIntError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?);
            }
            Ok(self.as_millis().to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
        }
    }
}

impl<V: Default + ConfigInterface + DeserializeOwned + Serialize> ConfigInterface for Vec<V> {
    fn conf_value() -> ConfigValue {
        let val_ty = V::conf_value();
        // TODO: make this compile time assert as soon as rust supports it
        assert!(!matches!(
            val_ty,
            ConfigValue::Array { .. } | ConfigValue::JsonLikeRecord { .. }
        ), "Currently arrays in arrays or records in arrays or the other way around are not allowed");
        ConfigValue::Array {
            val_ty: Box::new(val_ty),

            min_length: 0,
            max_length: usize::MAX,
        }
    }

    fn try_set_from_str(
        &mut self,
        path: String,
        modifier: Option<String>,
        val: Option<String>,
        conf_val: Option<&ConfigValue>,
        flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if (flags & ConfigFromStrFlags::Push as i32) != 0 {
            if conf_val.is_none()
                || conf_val.is_some_and(|v| {
                    if let ConfigValue::Array { max_length, .. } = v {
                        *max_length > self.len() + 1
                    } else {
                        false
                    }
                })
            {
                self.push(Default::default());
            } else {
                return Err(ConfigFromStrErr::PathErr(
                    ConfigFromStrPathErr::ValidationError(format!(
                        "The max length of the array is reached"
                    )),
                ));
            }
            Ok(serde_json::to_string(self).map_err(|err| {
                ConfigFromStrErr::FatalErr(format!("Could not serialize current value: {err}"))
            })?)
        } else if (flags & ConfigFromStrFlags::Pop as i32) != 0 {
            if conf_val.is_none()
                || conf_val.is_some_and(|v| {
                    if let ConfigValue::Array { min_length, .. } = v {
                        self.len() > 0 && *min_length <= self.len() - 1
                    } else {
                        false
                    }
                })
            {
                self.pop();
            } else {
                return Err(ConfigFromStrErr::PathErr(
                    ConfigFromStrPathErr::ValidationError(format!(
                        "The min length of the array is reached"
                    )),
                ));
            }
            Ok(serde_json::to_string(self).map_err(|err| {
                ConfigFromStrErr::FatalErr(format!("Could not serialize current value: {err}"))
            })?)
        } else if path.is_empty() {
            if let Some(val) = val {
                // modifier must be none
                if let Some(modifier) = modifier {
                    let index: usize = modifier.parse().map_err(|err| {
                        ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(format!(
                            "index not parsable: {err}"
                        )))
                    })?;
                    return self
                        .get_mut(index)
                        .ok_or_else(|| {
                            ConfigFromStrErr::PathErr(ConfigFromStrPathErr::FatalErr(
                                "value with that index does not exist, use `push <var>` to add new entry".into(),
                            ))
                        })?
                        .try_set_from_str(path, None, Some(val), None, 0);
                } else {
                    *self = serde_json::from_str(&val).map_err(|err: serde_json::Error| {
                        ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                    })?
                }
            }
            if let Some(modifier) = modifier {
                let index: usize = modifier.parse().map_err(|err| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(format!(
                        "index not parsable: {err}"
                    )))
                })?;
                // get value of child
                Ok(self
                    .get_mut(index)
                    .ok_or_else(|| {
                        ConfigFromStrErr::PathErr(ConfigFromStrPathErr::FatalErr(
                            "value with that index does not exist, use `push <var>` to add new entry".into(),
                        ))
                    })?
                    .try_set_from_str("".into(), None, None, None, 0)?)
            } else {
                Ok(serde_json::to_string(self).map_err(|err| {
                    ConfigFromStrErr::FatalErr(format!("Could not serialize current value: {err}"))
                })?)
            }
        } else {
            if let Some(modifier) = modifier {
                let index: usize = modifier.parse().map_err(|err| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(format!(
                        "index not parsable: {err}"
                    )))
                })?;
                self.get_mut(index)
                    .ok_or_else(|| {
                        ConfigFromStrErr::PathErr(ConfigFromStrPathErr::FatalErr(
                            "value with that index does not exist, use `push <var>` to add new entry".into(),
                        ))
                    })?
                    .try_set_from_str(path, None, val, None, 0)
            } else {
                Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::FatalErr(
                    "expected [index]... or nothing, but found another path".into(),
                )))
            }
        }
    }
}

impl<'de, T> crate::traits::ConfigInterface for HashMap<String, T>
where
    T: Default + ConfigInterface + Serialize + DeserializeOwned,
{
    fn conf_value() -> crate::traits::ConfigValue {
        let val_ty = T::conf_value();
        // TODO: make this compile time assert as soon as rust supports it
        assert!(!matches!(
            val_ty,
            ConfigValue::Array { .. } | ConfigValue::JsonLikeRecord { .. }
        ), "Currently arrays in arrays or records in arrays or the other way around are not allowed");
        ConfigValue::JsonLikeRecord {
            val_ty: Box::new(val_ty),
        }
    }

    fn try_set_from_str(
        &mut self,
        path: String,
        modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if (flags & ConfigFromStrFlags::Rem as i32) != 0 {
            if let Some(modifier) = modifier {
                let index: &String = &modifier;
                self.remove(index);
            }
            Ok(serde_json::to_string(self).map_err(|err| {
                ConfigFromStrErr::FatalErr(format!("Could not serialize current value: {err}"))
            })?)
        } else if path.is_empty() {
            if let Some(val) = val {
                if let Some(modifier) = modifier {
                    let index: &String = &modifier;
                    if !self.contains_key(index) {
                        self.insert(index.clone(), Default::default());
                    }
                    return self.get_mut(index).unwrap().try_set_from_str(
                        path,
                        None,
                        Some(val),
                        None,
                        0,
                    );
                } else {
                    *self = serde_json::from_str(&val).map_err(|err: serde_json::Error| {
                        ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                    })?;
                }
            }
            if let Some(modifier) = modifier {
                let index: &String = &modifier;
                // get value of child
                Ok(self
                    .get_mut(index)
                    .ok_or_else(|| {
                        ConfigFromStrErr::PathErr(ConfigFromStrPathErr::FatalErr(
                            "value not yet assigned".into(),
                        ))
                    })?
                    .try_set_from_str("".into(), None, None, None, 0)?)
            } else {
                Ok(serde_json::to_string(self).map_err(|err| {
                    ConfigFromStrErr::FatalErr(format!("Could not serialize current value: {err}"))
                })?)
            }
        } else {
            if let Some(modifier) = modifier {
                let index: &String = &modifier;
                // if value is Some, assume that the assign on the child will succeed
                // a.k.a. the user at least wanted to assign a value
                if !self.contains_key(index) && val.is_some() {
                    self.insert(index.clone(), Default::default());
                }
                self.get_mut(index)
                    .ok_or_else(|| {
                        ConfigFromStrErr::PathErr(ConfigFromStrPathErr::FatalErr(
                            "value not yet assigned".into(),
                        ))
                    })?
                    .try_set_from_str(path, None, val, None, 0)
            } else {
                Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::FatalErr(
                    "expected [key]... or nothing, but found another path".into(),
                )))
            }
        }
    }
}

impl ConfigInterface for bool {
    fn conf_value() -> ConfigValue {
        ConfigValue::Int { min: 0, max: 1 }
    }

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseBoolError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseIntError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseIntError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseIntError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseIntError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseIntError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseIntError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseIntError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseIntError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseFloatError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
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

    fn try_set_from_str(
        &mut self,
        path: String,
        _modifier: Option<String>,
        val: Option<String>,
        _conf_val: Option<&ConfigValue>,
        _flags: i32,
    ) -> anyhow::Result<String, ConfigFromStrErr> {
        if path.is_empty() {
            if let Some(val) = val {
                *self = val.parse().map_err(|err: ParseFloatError| {
                    ConfigFromStrErr::PathErr(ConfigFromStrPathErr::ParsingErr(err.to_string()))
                })?;
            }
            Ok(self.to_string())
        } else {
            Err(ConfigFromStrErr::PathErr(ConfigFromStrPathErr::EndOfPath(
                path,
            )))
        }
    }
}
