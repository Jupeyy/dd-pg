use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub type BackendWriteFiles = Arc<parking_lot::Mutex<HashMap<PathBuf, Vec<u8>>>>;
