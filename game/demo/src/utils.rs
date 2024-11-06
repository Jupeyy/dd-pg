use std::io::Read;

use serde::de::DeserializeOwned;

pub fn decomp<'a>(v: &[u8], writer: &'a mut Vec<u8>) -> anyhow::Result<&'a [u8]> {
    writer.clear();
    let mut decoder = zstd::Decoder::new(v)?;
    decoder.read_to_end(&mut *writer)?;
    decoder.finish();

    Ok(writer.as_mut_slice())
}
pub fn deser_ex<T: DeserializeOwned>(v: &[u8], fixed_size: bool) -> anyhow::Result<(T, usize)> {
    if fixed_size {
        Ok(bincode::serde::decode_from_slice(
            v,
            bincode::config::standard().with_fixed_int_encoding(),
        )?)
    } else {
        Ok(bincode::serde::decode_from_slice(
            v,
            bincode::config::standard(),
        )?)
    }
}
pub fn deser<T: DeserializeOwned>(v: &[u8]) -> anyhow::Result<(T, usize)> {
    deser_ex(v, false)
}
