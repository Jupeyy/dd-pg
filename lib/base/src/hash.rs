pub type Hash = [u8; blake3::OUT_LEN];

/// generates the blake3 hash for the given slice
///
/// __Note__: this function is only for
/// signature checking (files/keys etc.).
/// It should not be used to hash passwords.
pub fn generate_hash_for(data: &[u8]) -> Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(data);
    let hash = hasher.finalize();
    hash.into()
}

/// generates the blake3 hash for the given slices
///
/// __Note__: this function is only for
/// signature checking (files/keys etc.).
/// It should not be used to hash passwords.
pub fn generate_hash_for_multi(data: &[&[u8]]) -> Hash {
    let mut hasher = blake3::Hasher::new();
    for data in data {
        hasher.update(data);
    }
    let hash = hasher.finalize();
    hash.into()
}

/// Encode a hash as lowercase hex string with leading zero
pub fn decode_hash(hash: &str) -> Option<Hash> {
    hex::decode(hash).ok().and_then(|hash| hash.try_into().ok())
}

/// Encode a hash as lowercase hex string with leading zero
pub fn fmt_hash(hash: &Hash) -> String {
    hex::encode(hash)
}

/// Split name & blake3 hash from a file name.
/// This even works, if the file name never contained
/// the hash in first place.
/// The given name should always be without extension.
/// It also works for resources.
/// E.g. mymap_<HASH> => (mymap, <HASH>)
pub fn name_and_hash(name: &str, file: &[u8]) -> (String, Hash) {
    let hash = generate_hash_for(file);
    let hash_str = format!("_{}", fmt_hash(&hash));

    let file_name = if name.ends_with(&hash_str) {
        name.strip_suffix(&hash_str).unwrap()
    } else {
        name
    };

    (file_name.to_string(), hash)
}
