use std::io::{Read, Write};

/// returns the size of the compressed chunk and the size that was
/// read from `file`
pub fn compressed_size(file: &[u8]) -> anyhow::Result<(u32, usize)> {
    let size_mem_size = std::mem::size_of::<u32>();
    anyhow::ensure!(file.len() >= size_mem_size);
    let file_size = u32::from_le_bytes([file[0], file[1], file[2], file[3]]);
    Ok((file_size, size_mem_size))
}

/// Decompresses a compressed file into an uncompressed file. Returns the bytes read
/// ### Prefer this method over using compression algorithms yourself, because it has side effects
pub fn decompress(file: &[u8]) -> anyhow::Result<(Vec<u8>, usize)> {
    let (file_size, read_size) = compressed_size(file)?;
    let mut uncompressed_file: Vec<u8> = Default::default();
    let mut dec = zstd::Decoder::new(&file[read_size..read_size + file_size as usize])?;
    dec.read_to_end(&mut uncompressed_file)?;
    dec.finish();
    Ok((uncompressed_file, read_size + file_size as usize))
}

/// Compresses an uncompressed file into a compressed file.
/// ### Prefer this method over using compression algorithms yourself. It has side effects
pub fn compress<W: std::io::Write>(uncompressed_file: &[u8], writer: &mut W) -> anyhow::Result<()> {
    let mut write_data: Vec<u8> = Default::default();
    // Compression level 15 seems to be a good trait performance vs map size
    // Tested with the test benchmark in this crate on some maps.
    let mut enc = zstd::Encoder::new(&mut write_data, 15)?;
    enc.write_all(uncompressed_file)?;
    enc.finish()?;
    writer.write_all(&(write_data.len() as u32).to_le_bytes())?;
    writer.write_all(&write_data)?;
    Ok(())
}
