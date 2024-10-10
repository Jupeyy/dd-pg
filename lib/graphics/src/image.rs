use std::sync::Arc;

use rayon::{
    prelude::{IndexedParallelIterator, ParallelIterator},
    slice::{ParallelSlice, ParallelSliceMut},
};

const TW_DILATE_ALPHA_THRESHOLD: u8 = 10;

pub fn dilate(
    thread_pool: &Arc<rayon::ThreadPool>,
    w: usize,
    h: usize,
    bpp: usize,
    src_buff: &[u8],
    dest_buff: &mut [u8],
    alpha_threshold: u8,
) {
    let dirs_x = [0, -1, 1, 0];
    let dirs_y = [-1, 0, 0, 1];

    let alpha_comp_index = bpp - 1;

    thread_pool.install(|| {
        dest_buff
            .par_chunks_exact_mut(bpp)
            .enumerate()
            .take(w * h)
            .for_each(|(i, dst)| {
                let x = i % w;
                let y = i / w;

                let m = y * w * bpp + x * bpp;
                dst.copy_from_slice(&src_buff[m..(bpp + m)]);
                if src_buff[m + alpha_comp_index] > alpha_threshold {
                    return;
                }

                // clear pixels that are considered transparent
                // this allows the image to always be black where no dilate is needed
                dst[0..(bpp - 1)].fill(0);

                let mut sums_of_opaque = [0, 0, 0];
                let mut counter = 0;
                for c in 0..4 {
                    let ix = (x as i64 + dirs_x[c]).clamp(0, w as i64 - 1) as usize;
                    let iy = (y as i64 + dirs_y[c]).clamp(0, h as i64 - 1) as usize;
                    let k = iy * w * bpp + ix * bpp;
                    if src_buff[k + alpha_comp_index] > alpha_threshold {
                        for p in 0..bpp - 1 {
                            // Seems safe for BPP = 3, 4 which we use.
                            sums_of_opaque[p] += src_buff[k + p] as u32;
                        }
                        counter += 1;
                        break;
                    }
                }

                if counter > 0 {
                    for i in 0..bpp - 1 {
                        sums_of_opaque[i] /= counter;
                        dst[i] = sums_of_opaque[i] as u8;
                    }

                    dst[alpha_comp_index] = 255;
                }
            });
    });
}

fn copy_color_values(
    thread_pool: &Arc<rayon::ThreadPool>,
    w: usize,
    h: usize,
    bpp: usize,
    src_buffer: &[u8],
    dest_buffer: &mut [u8],
) {
    thread_pool.install(|| {
        dest_buffer
            .par_chunks_exact_mut(bpp)
            .take(w * h)
            .zip(src_buffer.par_chunks_exact(bpp).take(w * h))
            .for_each(|(dst, src)| {
                if dst[bpp - 1] == 0 {
                    dst[0..bpp - 1].copy_from_slice(&src[0..bpp - 1]);
                }
            });
    });
}

pub fn dilate_image_sub(
    thread_pool: &Arc<rayon::ThreadPool>,
    img_buff: &mut [u8],
    w: usize,
    _h: usize,
    bpp: usize,
    x: usize,
    y: usize,
    sw: usize,
    sh: usize,
) {
    let [mut buffer_data1, mut buffer_data2] = [
        vec![0; sw * sh * std::mem::size_of::<u8>() * bpp],
        vec![0; sw * sh * std::mem::size_of::<u8>() * bpp],
    ];

    let mut buffer_data_original = vec![0; sw * sh * std::mem::size_of::<u8>() * bpp];

    let pixel_buffer_data = img_buff;

    thread_pool.install(|| {
        // fill buffer_data_original completely
        buffer_data_original
            .chunks_exact_mut(sw * bpp)
            .enumerate()
            .for_each(|(yh, chunk)| {
                let src_img_offset = ((y + yh) * w * bpp) + (x * bpp);

                chunk.copy_from_slice(
                    &pixel_buffer_data[src_img_offset..src_img_offset + chunk.len()],
                );
            });
    });

    dilate(
        thread_pool,
        sw,
        sh,
        bpp,
        buffer_data_original.as_slice(),
        buffer_data1.as_mut_slice(),
        TW_DILATE_ALPHA_THRESHOLD,
    );

    for _i in 0..5 {
        dilate(
            thread_pool,
            sw,
            sh,
            bpp,
            buffer_data1.as_slice(),
            buffer_data2.as_mut_slice(),
            TW_DILATE_ALPHA_THRESHOLD,
        );
        dilate(
            thread_pool,
            sw,
            sh,
            bpp,
            buffer_data2.as_slice(),
            buffer_data1.as_mut_slice(),
            TW_DILATE_ALPHA_THRESHOLD,
        );
    }

    copy_color_values(
        thread_pool,
        sw,
        sh,
        bpp,
        buffer_data1.as_slice(),
        buffer_data_original.as_mut_slice(),
    );

    thread_pool.install(|| {
        pixel_buffer_data
            .chunks_exact_mut(w * bpp)
            .skip(y)
            .take(sh)
            .enumerate()
            .for_each(|(yh, chunk)| {
                let src_img_offset = x * bpp;
                let dst_img_offset = yh * sw * bpp;
                let copy_size = sw * bpp;
                chunk[src_img_offset..src_img_offset + copy_size].copy_from_slice(
                    &buffer_data_original[dst_img_offset..dst_img_offset + copy_size],
                );
            });
    });
}

pub fn dilate_image(
    thread_pool: &Arc<rayon::ThreadPool>,
    img_buff: &mut [u8],
    w: usize,
    h: usize,
    bpp: usize,
) {
    dilate_image_sub(thread_pool, img_buff, w, h, bpp, 0, 0, w, h);
}

fn cubic_hermite(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let a = -a / 2.0 + (3.0 * b) / 2.0 - (3.0 * c) / 2.0 + d / 2.0;
    let b = a - (5.0 * b) / 2.0 + 2.0 * c - d / 2.0;
    let c = -a / 2.0 + c / 2.0;
    let d = b;

    (a * t * t * t) + (b * t * t) + (c * t) + d
}

fn get_pixel_clamped(
    src_img_buff: &[u8],
    x_param: i32,
    y_param: i32,
    w: usize,
    h: usize,
    bpp: usize,
    tmp_buff: &mut [u8],
) {
    let x = x_param.clamp(0, w as i32 - 1);
    let y = y_param.clamp(0, h as i32 - 1);

    for i in 0..bpp {
        tmp_buff[i] = src_img_buff[x as usize * bpp + (w * bpp * y as usize) + i];
    }
}

fn sample_bicubic(
    src_image_buff: &[u8],
    u: f32,
    v: f32,
    w: usize,
    h: usize,
    bpp: usize,
    samples: &mut [u8],
) {
    let x = (u * w as f32) - 0.5;
    let x_int = x as i32;
    let x_fract = x - (x).floor();

    let y = (v * h as f32) - 0.5;
    let y_int = y as i32;
    let y_fract = y - (y).floor();

    let mut pxs_00: [u8; 4] = Default::default();
    let mut pxs_10: [u8; 4] = Default::default();
    let mut pxs_20: [u8; 4] = Default::default();
    let mut pxs_30: [u8; 4] = Default::default();

    let mut pxs_01: [u8; 4] = Default::default();
    let mut pxs_11: [u8; 4] = Default::default();
    let mut pxs_21: [u8; 4] = Default::default();
    let mut pxs_31: [u8; 4] = Default::default();

    let mut pxs_02: [u8; 4] = Default::default();
    let mut pxs_12: [u8; 4] = Default::default();
    let mut pxs_22: [u8; 4] = Default::default();
    let mut pxs_32: [u8; 4] = Default::default();

    let mut pxs_03: [u8; 4] = Default::default();
    let mut pxs_13: [u8; 4] = Default::default();
    let mut pxs_23: [u8; 4] = Default::default();
    let mut pxs_33: [u8; 4] = Default::default();

    get_pixel_clamped(
        src_image_buff,
        x_int - 1,
        y_int - 1,
        w,
        h,
        bpp,
        pxs_00.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int,
        y_int - 1,
        w,
        h,
        bpp,
        pxs_10.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 1,
        y_int - 1,
        w,
        h,
        bpp,
        pxs_20.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 2,
        y_int - 1,
        w,
        h,
        bpp,
        pxs_30.as_mut_slice(),
    );

    get_pixel_clamped(
        src_image_buff,
        x_int - 1,
        y_int,
        w,
        h,
        bpp,
        pxs_01.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int,
        y_int,
        w,
        h,
        bpp,
        pxs_11.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 1,
        y_int,
        w,
        h,
        bpp,
        pxs_21.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 2,
        y_int,
        w,
        h,
        bpp,
        pxs_31.as_mut_slice(),
    );

    get_pixel_clamped(
        src_image_buff,
        x_int - 1,
        y_int + 1,
        w,
        h,
        bpp,
        pxs_02.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int,
        y_int + 1,
        w,
        h,
        bpp,
        pxs_12.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 1,
        y_int + 1,
        w,
        h,
        bpp,
        pxs_22.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 2,
        y_int + 1,
        w,
        h,
        bpp,
        pxs_32.as_mut_slice(),
    );

    get_pixel_clamped(
        src_image_buff,
        x_int - 1,
        y_int + 2,
        w,
        h,
        bpp,
        pxs_03.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int,
        y_int + 2,
        w,
        h,
        bpp,
        pxs_13.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 1,
        y_int + 2,
        w,
        h,
        bpp,
        pxs_23.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 2,
        y_int + 2,
        w,
        h,
        bpp,
        pxs_33.as_mut_slice(),
    );

    for i in 0..bpp {
        let column_0 = cubic_hermite(
            pxs_00[i] as f32,
            pxs_10[i] as f32,
            pxs_20[i] as f32,
            pxs_30[i] as f32,
            x_fract,
        );
        let column_1 = cubic_hermite(
            pxs_01[i] as f32,
            pxs_11[i] as f32,
            pxs_21[i] as f32,
            pxs_31[i] as f32,
            x_fract,
        );
        let column_2 = cubic_hermite(
            pxs_02[i] as f32,
            pxs_12[i] as f32,
            pxs_22[i] as f32,
            pxs_32[i] as f32,
            x_fract,
        );
        let column_3 = cubic_hermite(
            pxs_03[i] as f32,
            pxs_13[i] as f32,
            pxs_23[i] as f32,
            pxs_33[i] as f32,
            x_fract,
        );

        let mut value = cubic_hermite(column_0, column_1, column_2, column_3, y_fract);

        value = value.clamp(0.0, 255.0);

        samples[i] = value as u8;
    }
}

fn resize_image_inner(
    thread_pool: &Arc<rayon::ThreadPool>,
    src_image_buff: &[u8],
    sw: usize,
    sh: usize,
    dst_image_buff: &mut Vec<u8>,
    w: usize,
    h: usize,
    bpp: usize,
) {
    thread_pool.install(|| {
        dst_image_buff
            .par_chunks_exact_mut(w * bpp)
            .enumerate()
            .for_each(|(y, write_chunk)| {
                let v = y as f32 / (h - 1) as f32;
                let mut samples: [u8; 4] = Default::default();

                for x in 0..w as i32 {
                    let u = x as f32 / (w - 1) as f32;
                    sample_bicubic(src_image_buff, u, v, sw, sh, bpp, samples.as_mut_slice());

                    for i in 0..bpp {
                        write_chunk[x as usize * bpp + i] = samples[i];
                    }
                }
            });
    });
}

pub fn resize_image(
    thread_pool: &Arc<rayon::ThreadPool>,
    img_data_buff: &[u8],
    width: usize,
    height: usize,
    new_width: usize,
    new_height: usize,
    bpp: usize,
) -> Vec<u8> {
    let mut img_data = Vec::<u8>::new();
    img_data.resize(new_width * new_height * bpp, Default::default());

    resize_image_inner(
        thread_pool,
        img_data_buff,
        width,
        height,
        &mut img_data,
        new_width,
        new_height,
        bpp,
    );

    img_data
}

pub fn resize(
    thread_pool: &Arc<rayon::ThreadPool>,
    data_buff: &[u8],
    width: usize,
    height: usize,
    new_width: usize,
    new_height: usize,
    bpp: usize,
) -> Vec<u8> {
    resize_image(
        thread_pool,
        data_buff,
        width,
        height,
        new_width,
        new_height,
        bpp,
    )
}

pub fn texture_2d_to_3d(
    thread_pool: &Arc<rayon::ThreadPool>,
    img_buff: &[u8],
    image_width: usize,
    image_height: usize,
    image_color_channel_count: usize,
    split_count_width: usize,
    split_count_height: usize,
    target_3d_img_buff_data: &mut [u8],
    target_3d_img_width: &mut usize,
    target_3d_img_height: &mut usize,
) -> bool {
    *target_3d_img_width = image_width / split_count_width;
    *target_3d_img_height = image_height / split_count_height;

    let full_image_width = image_width * image_color_channel_count;

    let target_image_full_width = { *target_3d_img_width } * image_color_channel_count;
    thread_pool.install(|| {
        target_3d_img_buff_data
            .par_chunks_exact_mut(target_image_full_width)
            .enumerate()
            .for_each(|(index, write_chunk)| {
                let x_src = (index / *target_3d_img_height) % split_count_width;
                let y_src = index % *target_3d_img_height
                    + ((index / (split_count_width * *target_3d_img_height))
                        * *target_3d_img_height);
                let src_off = y_src * full_image_width + (x_src * target_image_full_width);

                write_chunk.copy_from_slice(&img_buff[src_off..src_off + target_image_full_width]);
            });
    });

    true
}

pub fn highest_bit(of_var_param: u32) -> u32 {
    let mut of_var = of_var_param;
    if of_var == 0 {
        return 0;
    }

    let mut ret_v = 1;

    loop {
        of_var >>= 1;
        if of_var == 0 {
            break;
        }
        ret_v <<= 1;
    }

    ret_v
}
