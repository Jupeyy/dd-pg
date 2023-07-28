use std::io;

pub struct PngResult<'a> {
    pub data: &'a [u8],
    pub width: u32,
    pub height: u32,
}

pub fn load_png_image<'a, T>(file: &Vec<u8>, alloc_mem: T) -> io::Result<PngResult<'a>>
where
    T: FnOnce(usize) -> &'a mut [u8],
{
    use png::ColorType::*;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;

    let real_img_size = reader.output_buffer_size();
    let color_type = reader.output_color_type().0;

    let data_size = match color_type {
        Rgb => (real_img_size * 4) / 3,
        Rgba => real_img_size,
        Grayscale => real_img_size * 4,
        GrayscaleAlpha => real_img_size * 4,
        _ => unreachable!("uncovered color type"),
    };

    let img_data = alloc_mem(data_size);
    let info = reader.next_frame(img_data)?;

    let data = match color_type {
        Rgb => {
            let tmp = img_data[0..real_img_size].to_vec();
            for (index, ga) in tmp.chunks(3).enumerate() {
                img_data[index * 4 + 0] = ga[0];
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
                img_data[index * 4 + 0] = *g;
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
                img_data[index * 4 + 0] = g;
                img_data[index * 4 + 1] = g;
                img_data[index * 4 + 2] = g;
                img_data[index * 4 + 3] = a;
            }
            img_data
        }
        _ => unreachable!("uncovered color type"),
    };

    Ok(PngResult {
        data: data,
        width: info.width,
        height: info.height,
    })
}

pub fn save_png_image<'a, T>(
    raw_bytes: &Vec<u8>,
    width: u32,
    height: u32,
) -> anyhow::Result<Vec<u8>> {
    use png::ColorType::*;
    let mut res: Vec<u8> = Default::default();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(&mut res), width, height);
    encoder.set_color(Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;

    writer.write_image_data(&raw_bytes)?;

    writer.finish()?;

    Ok(res)
}
