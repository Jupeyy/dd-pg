use std::io;

#[derive(Debug)]
pub struct PngResultPersistentFast {
    width: u32,
    height: u32,
}

impl PngResultPersistentFast {
    pub fn to_persistent(self, data: Vec<u8>) -> PngResultPersistent {
        PngResultPersistent {
            data,
            width: self.width,
            height: self.height,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct PngResultPersistent {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct PngResult<'a> {
    pub data: &'a [u8],
    pub width: u32,
    pub height: u32,
}

impl<'a> PngResult<'a> {
    pub fn to_persistent(self) -> PngResultPersistent {
        PngResultPersistent {
            data: self.data.to_vec(),
            width: self.width,
            height: self.height,
        }
    }

    pub fn prepare_moved_persistent(self) -> PngResultPersistentFast {
        PngResultPersistentFast {
            width: self.width,
            height: self.height,
        }
    }
}

/// takes a closure of (width, height, color_channel_count)
pub fn load_png_image<'a, T>(file: &Vec<u8>, alloc_mem: T) -> io::Result<PngResult<'a>>
where
    T: FnOnce(usize, usize, usize) -> &'a mut [u8],
{
    use png::ColorType::*;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;

    let real_img_size = reader.output_buffer_size();
    let color_type = reader.output_color_type().0;

    let info = reader.info();
    let img_data = alloc_mem(info.width as usize, info.height as usize, 4);
    let info = reader.next_frame(img_data)?;

    let data = match color_type {
        Rgb => {
            let tmp = img_data[0..real_img_size].to_vec();
            for (index, ga) in tmp.chunks(3).enumerate() {
                img_data[index * 4] = ga[0];
                img_data[index * 4 + 1] = ga[1];
                img_data[index * 4 + 2] = ga[2];
                img_data[index * 4 + 3] = 255;
            }
            img_data
        }
        Rgba => img_data,
        Grayscale => {
            let tmp = img_data[0..real_img_size].to_vec();
            for (index, g) in tmp.iter().enumerate() {
                img_data[index * 4] = *g;
                img_data[index * 4 + 1] = *g;
                img_data[index * 4 + 2] = *g;
                img_data[index * 4 + 3] = 255;
            }
            img_data
        }
        GrayscaleAlpha => {
            let tmp = img_data[0..real_img_size].to_vec();
            for (index, ga) in tmp.chunks(2).enumerate() {
                let g = ga[0];
                let a = ga[1];
                img_data[index * 4] = g;
                img_data[index * 4 + 1] = g;
                img_data[index * 4 + 2] = g;
                img_data[index * 4 + 3] = a;
            }
            img_data
        }
        _ => unreachable!("uncovered color type"),
    };

    Ok(PngResult {
        data,
        width: info.width,
        height: info.height,
    })
}

pub fn save_png_image_ex(
    raw_bytes: &[u8],
    width: u32,
    height: u32,
    compresion_best: bool,
) -> anyhow::Result<Vec<u8>> {
    use png::ColorType::*;
    let mut res: Vec<u8> = Default::default();
    let mut encoder = png::Encoder::new(&mut res, width, height);
    encoder.set_color(Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    if compresion_best {
        encoder.set_compression(png::Compression::Best);
    }
    let mut writer = encoder.write_header()?;

    writer.write_image_data(raw_bytes)?;

    writer.finish()?;

    Ok(res)
}

pub fn save_png_image(raw_bytes: &[u8], width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    save_png_image_ex(raw_bytes, width, height, false)
}
