use std::sync::Arc;

use rayon::{
    prelude::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};

const TW_DILATE_ALPHA_THRESHOLD: u8 = 10;

pub fn dilate(
    _thread_pool: &Arc<rayon::ThreadPool>,
    w: i32,
    h: i32,
    bpp: i32,
    src_buff: &[u8],
    dest_buff: &mut [u8],
    alpha_threshold: u8,
) {
    let mut ix: i32;
    let mut iy: i32;
    let dirs_x = [0, -1, 1, 0];
    let dirs_y = [-1, 0, 0, 1];

    let alpha_comp_index = bpp - 1;

    let mut m = 0;
    for y in 0..h {
        for x in 0..w {
            for i in 0..bpp {
                dest_buff[(m + i) as usize] = src_buff[(m + i) as usize];
            }
            if src_buff[(m + alpha_comp_index) as usize] > alpha_threshold {
                continue;
            }

            let mut sums_of_opaque = [0, 0, 0];
            let mut counter = 0;
            for c in 0..4 {
                ix = (x + dirs_x[c]).clamp(0, w - 1);
                iy = (y + dirs_y[c]).clamp(0, h - 1);
                let k = iy * w * bpp + ix * bpp;
                if src_buff[(k + alpha_comp_index) as usize] > alpha_threshold {
                    for p in 0..bpp - 1 {
                        // Seems safe for BPP = 3, 4 which we use. clang-analyzer seems to
                        // assume being called with larger value. TODO: Can make this
                        // safer anyway.
                        sums_of_opaque[p as usize] += src_buff[(k + p) as usize] as i32;
                        // NOLINT(clang-analyzer-core.uninitialized.Assign)
                    }
                    counter += 1;
                    break;
                }
            }

            if counter > 0 {
                for i in 0..bpp - 1 {
                    sums_of_opaque[i as usize] /= counter;
                    dest_buff[(m + i) as usize] = sums_of_opaque[i as usize] as u8;
                }

                dest_buff[(m + alpha_comp_index) as usize] = 255;
            }
            m += bpp;
        }
    }
}

fn copy_color_values(
    _thread_pool: &Arc<rayon::ThreadPool>,
    w: i32,
    h: i32,
    bpp: i32,
    src_buffer: &[u8],
    dest_buffer: &mut [u8],
) {
    let mut m = 0;
    for _y in 0..h {
        for _x in 0..w {
            for i in 0..bpp - 1 {
                if dest_buffer[(m + 3) as usize] == 0 {
                    dest_buffer[(m + i) as usize] = src_buffer[(m + i) as usize];
                }
            }
            m += bpp;
        }
    }
}

pub fn dilate_image(
    thread_pool: &Arc<rayon::ThreadPool>,
    img_buff: &mut [u8],
    w: i32,
    h: i32,
    bpp: i32,
) {
    dilate_image_sub(thread_pool, img_buff, w, h, bpp, 0, 0, w, h);
}

pub fn dilate_image_sub(
    thread_pool: &Arc<rayon::ThreadPool>,
    img_buff: &mut [u8],
    w: i32,
    _h: i32,
    bpp: i32,
    x: i32,
    y: i32,
    sw: i32,
    sh: i32,
) {
    let [mut buffer_data1, mut buffer_data2] = [Vec::<u8>::new(), Vec::<u8>::new()];
    buffer_data1.resize(
        sw as usize * sh as usize * std::mem::size_of::<u8>() * bpp as usize,
        Default::default(),
    );
    buffer_data2.resize(
        sw as usize * sh as usize * std::mem::size_of::<u8>() * bpp as usize,
        Default::default(),
    );

    let mut buffer_data_original = Vec::<u8>::new();
    buffer_data_original.resize(
        sw as usize * sh as usize * std::mem::size_of::<u8>() * bpp as usize,
        Default::default(),
    );

    let pixel_buffer_data = img_buff;

    for yh in 0..sh {
        let src_img_offset = ((y + yh) * w * bpp) + (x * bpp);
        let dst_img_offset = yh * sw * bpp;
        let copy_size = sw * bpp;
        buffer_data_original
            .split_at_mut(dst_img_offset as usize)
            .1
            .copy_from_slice(
                &pixel_buffer_data.split_at(src_img_offset as usize).1[0..copy_size as usize],
            );
    }

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

    for yh in 0..sh {
        let src_img_offset = ((y + yh) * w * bpp) + (x * bpp);
        let dst_img_offset = yh * sw * bpp;
        let copy_size = sw * bpp;
        pixel_buffer_data
            .split_at_mut(src_img_offset as usize)
            .1
            .copy_from_slice(
                &buffer_data_original.split_at(dst_img_offset as usize).1[0..copy_size as usize],
            );
    }
}

fn cubic_hermite(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let a = -a / 2.0 + (3.0 * b) / 2.0 - (3.0 * c) / 2.0 + d / 2.0;
    let b = a - (5.0 * b) / 2.0 + 2.0 * c - d / 2.0;
    let c = -a / 2.0 + c / 2.0;
    let d = b;

    return (a * t * t * t) + (b * t * t) + (c * t) + d;
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
        tmp_buff[i] = src_img_buff[x as usize * bpp as usize
            + (w as usize * bpp as usize * y as usize) as usize
            + i as usize];
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
        x_int + 0,
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
        y_int + 0,
        w,
        h,
        bpp,
        pxs_01.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 0,
        y_int + 0,
        w,
        h,
        bpp,
        pxs_11.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 1,
        y_int + 0,
        w,
        h,
        bpp,
        pxs_21.as_mut_slice(),
    );
    get_pixel_clamped(
        src_image_buff,
        x_int + 2,
        y_int + 0,
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
        x_int + 0,
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
        x_int + 0,
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

    for i in 0..bpp as usize {
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

                    for i in 0..bpp as usize {
                        write_chunk[x as usize * bpp as usize + i as usize] = samples[i];
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
    img_data.resize(
        new_width as usize * new_height as usize * bpp as usize,
        Default::default(),
    );

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

    return img_data;
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
    return resize_image(
        thread_pool,
        data_buff,
        width,
        height,
        new_width,
        new_height,
        bpp,
    );
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

    let full_image_width = image_width as usize * image_color_channel_count as usize;

    let target_image_full_width =
        *target_3d_img_width as usize * image_color_channel_count as usize;
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

    return true;
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

    return ret_v;
}
