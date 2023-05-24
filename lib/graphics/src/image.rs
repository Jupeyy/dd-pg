use std::sync::Arc;

use rayon::{
    prelude::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};

const TW_DILATE_ALPHA_THRESHOLD: u8 = 10;

pub fn Dilate(
    _thread_pool: &Arc<rayon::ThreadPool>,
    w: i32,
    h: i32,
    BPP: i32,
    pSrc: &[u8],
    pDest: &mut [u8],
    AlphaThreshold: u8,
) {
    let mut ix: i32;
    let mut iy: i32;
    let aDirX = [0, -1, 1, 0];
    let aDirY = [-1, 0, 0, 1];

    let AlphaCompIndex = BPP - 1;

    let mut m = 0;
    for y in 0..h {
        for x in 0..w {
            for i in 0..BPP {
                pDest[(m + i) as usize] = pSrc[(m + i) as usize];
            }
            if pSrc[(m + AlphaCompIndex) as usize] > AlphaThreshold {
                continue;
            }

            let mut aSumOfOpaque = [0, 0, 0];
            let mut Counter = 0;
            for c in 0..4 {
                ix = (x + aDirX[c]).clamp(0, w - 1);
                iy = (y + aDirY[c]).clamp(0, h - 1);
                let k = iy * w * BPP + ix * BPP;
                if pSrc[(k + AlphaCompIndex) as usize] > AlphaThreshold {
                    for p in 0..BPP - 1 {
                        // Seems safe for BPP = 3, 4 which we use. clang-analyzer seems to
                        // assume being called with larger value. TODO: Can make this
                        // safer anyway.
                        aSumOfOpaque[p as usize] += pSrc[(k + p) as usize] as i32;
                        // NOLINT(clang-analyzer-core.uninitialized.Assign)
                    }
                    Counter += 1;
                    break;
                }
            }

            if Counter > 0 {
                for i in 0..BPP - 1 {
                    aSumOfOpaque[i as usize] /= Counter;
                    pDest[(m + i) as usize] = aSumOfOpaque[i as usize] as u8;
                }

                pDest[(m + AlphaCompIndex) as usize] = 255;
            }
            m += BPP;
        }
    }
}

fn CopyColorValues(
    _thread_pool: &Arc<rayon::ThreadPool>,
    w: i32,
    h: i32,
    BPP: i32,
    pSrc: &[u8],
    pDest: &mut [u8],
) {
    let mut m = 0;
    for _y in 0..h {
        for _x in 0..w {
            for i in 0..BPP - 1 {
                if pDest[(m + 3) as usize] == 0 {
                    pDest[(m + i) as usize] = pSrc[(m + i) as usize];
                }
            }
            m += BPP;
        }
    }
}

pub fn DilateImage(
    thread_pool: &Arc<rayon::ThreadPool>,
    pImageBuff: &mut [u8],
    w: i32,
    h: i32,
    BPP: i32,
) {
    DilateImageSub(thread_pool, pImageBuff, w, h, BPP, 0, 0, w, h);
}

pub fn DilateImageSub(
    thread_pool: &Arc<rayon::ThreadPool>,
    pImageBuff: &mut [u8],
    w: i32,
    _h: i32,
    BPP: i32,
    x: i32,
    y: i32,
    sw: i32,
    sh: i32,
) {
    let [mut pBuffer1, mut pBuffer2] = [Vec::<u8>::new(), Vec::<u8>::new()];
    pBuffer1.resize(
        sw as usize * sh as usize * std::mem::size_of::<u8>() * BPP as usize,
        Default::default(),
    );
    pBuffer2.resize(
        sw as usize * sh as usize * std::mem::size_of::<u8>() * BPP as usize,
        Default::default(),
    );

    let mut pBufferOriginal = Vec::<u8>::new();
    pBufferOriginal.resize(
        sw as usize * sh as usize * std::mem::size_of::<u8>() * BPP as usize,
        Default::default(),
    );

    let pPixelBuff = pImageBuff;

    for Y in 0..sh {
        let SrcImgOffset = ((y + Y) * w * BPP) + (x * BPP);
        let DstImgOffset = Y * sw * BPP;
        let CopySize = sw * BPP;
        pBufferOriginal
            .split_at_mut(DstImgOffset as usize)
            .1
            .copy_from_slice(&pPixelBuff.split_at(SrcImgOffset as usize).1[0..CopySize as usize]);
    }

    Dilate(
        thread_pool,
        sw,
        sh,
        BPP,
        pBufferOriginal.as_slice(),
        pBuffer1.as_mut_slice(),
        TW_DILATE_ALPHA_THRESHOLD,
    );

    for _i in 0..5 {
        Dilate(
            thread_pool,
            sw,
            sh,
            BPP,
            pBuffer1.as_slice(),
            pBuffer2.as_mut_slice(),
            TW_DILATE_ALPHA_THRESHOLD,
        );
        Dilate(
            thread_pool,
            sw,
            sh,
            BPP,
            pBuffer2.as_slice(),
            pBuffer1.as_mut_slice(),
            TW_DILATE_ALPHA_THRESHOLD,
        );
    }

    CopyColorValues(
        thread_pool,
        sw,
        sh,
        BPP,
        pBuffer1.as_slice(),
        pBufferOriginal.as_mut_slice(),
    );

    for Y in 0..sh {
        let SrcImgOffset = ((y + Y) * w * BPP) + (x * BPP);
        let DstImgOffset = Y * sw * BPP;
        let CopySize = sw * BPP;
        pPixelBuff
            .split_at_mut(SrcImgOffset as usize)
            .1
            .copy_from_slice(
                &pBufferOriginal.split_at(DstImgOffset as usize).1[0..CopySize as usize],
            );
    }
}

fn CubicHermite(A: f32, B: f32, C: f32, D: f32, t: f32) -> f32 {
    let a = -A / 2.0 + (3.0 * B) / 2.0 - (3.0 * C) / 2.0 + D / 2.0;
    let b = A - (5.0 * B) / 2.0 + 2.0 * C - D / 2.0;
    let c = -A / 2.0 + C / 2.0;
    let d = B;

    return (a * t * t * t) + (b * t * t) + (c * t) + d;
}

fn GetPixelClamped(
    pSourceImage: &[u8],
    x_param: i32,
    y_param: i32,
    W: usize,
    H: usize,
    BPP: usize,
    aTmp: &mut [u8],
) {
    let x = x_param.clamp(0, W as i32 - 1);
    let y = y_param.clamp(0, H as i32 - 1);

    for i in 0..BPP {
        aTmp[i] = pSourceImage[x as usize * BPP as usize
            + (W as usize * BPP as usize * y as usize) as usize
            + i as usize];
    }
}

fn SampleBicubic(
    pSourceImage: &[u8],
    u: f32,
    v: f32,
    W: usize,
    H: usize,
    BPP: usize,
    aSample: &mut [u8],
) {
    let X = (u * W as f32) - 0.5;
    let xInt = X as i32;
    let xFract = X - (X).floor();

    let Y = (v * H as f32) - 0.5;
    let yInt = Y as i32;
    let yFract = Y - (Y).floor();

    let mut aPX00: [u8; 4] = Default::default();
    let mut aPX10: [u8; 4] = Default::default();
    let mut aPX20: [u8; 4] = Default::default();
    let mut aPX30: [u8; 4] = Default::default();

    let mut aPX01: [u8; 4] = Default::default();
    let mut aPX11: [u8; 4] = Default::default();
    let mut aPX21: [u8; 4] = Default::default();
    let mut aPX31: [u8; 4] = Default::default();

    let mut aPX02: [u8; 4] = Default::default();
    let mut aPX12: [u8; 4] = Default::default();
    let mut aPX22: [u8; 4] = Default::default();
    let mut aPX32: [u8; 4] = Default::default();

    let mut aPX03: [u8; 4] = Default::default();
    let mut aPX13: [u8; 4] = Default::default();
    let mut aPX23: [u8; 4] = Default::default();
    let mut aPX33: [u8; 4] = Default::default();

    GetPixelClamped(
        pSourceImage,
        xInt - 1,
        yInt - 1,
        W,
        H,
        BPP,
        aPX00.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 0,
        yInt - 1,
        W,
        H,
        BPP,
        aPX10.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 1,
        yInt - 1,
        W,
        H,
        BPP,
        aPX20.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 2,
        yInt - 1,
        W,
        H,
        BPP,
        aPX30.as_mut_slice(),
    );

    GetPixelClamped(
        pSourceImage,
        xInt - 1,
        yInt + 0,
        W,
        H,
        BPP,
        aPX01.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 0,
        yInt + 0,
        W,
        H,
        BPP,
        aPX11.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 1,
        yInt + 0,
        W,
        H,
        BPP,
        aPX21.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 2,
        yInt + 0,
        W,
        H,
        BPP,
        aPX31.as_mut_slice(),
    );

    GetPixelClamped(
        pSourceImage,
        xInt - 1,
        yInt + 1,
        W,
        H,
        BPP,
        aPX02.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 0,
        yInt + 1,
        W,
        H,
        BPP,
        aPX12.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 1,
        yInt + 1,
        W,
        H,
        BPP,
        aPX22.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 2,
        yInt + 1,
        W,
        H,
        BPP,
        aPX32.as_mut_slice(),
    );

    GetPixelClamped(
        pSourceImage,
        xInt - 1,
        yInt + 2,
        W,
        H,
        BPP,
        aPX03.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 0,
        yInt + 2,
        W,
        H,
        BPP,
        aPX13.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 1,
        yInt + 2,
        W,
        H,
        BPP,
        aPX23.as_mut_slice(),
    );
    GetPixelClamped(
        pSourceImage,
        xInt + 2,
        yInt + 2,
        W,
        H,
        BPP,
        aPX33.as_mut_slice(),
    );

    for i in 0..BPP as usize {
        let Clmn0 = CubicHermite(
            aPX00[i] as f32,
            aPX10[i] as f32,
            aPX20[i] as f32,
            aPX30[i] as f32,
            xFract,
        );
        let Clmn1 = CubicHermite(
            aPX01[i] as f32,
            aPX11[i] as f32,
            aPX21[i] as f32,
            aPX31[i] as f32,
            xFract,
        );
        let Clmn2 = CubicHermite(
            aPX02[i] as f32,
            aPX12[i] as f32,
            aPX22[i] as f32,
            aPX32[i] as f32,
            xFract,
        );
        let Clmn3 = CubicHermite(
            aPX03[i] as f32,
            aPX13[i] as f32,
            aPX23[i] as f32,
            aPX33[i] as f32,
            xFract,
        );

        let mut Valuef = CubicHermite(Clmn0, Clmn1, Clmn2, Clmn3, yFract);

        Valuef = Valuef.clamp(0.0, 255.0);

        aSample[i] = Valuef as u8;
    }
}

fn ResizeImageInner(
    thread_pool: &Arc<rayon::ThreadPool>,
    pSourceImage: &[u8],
    SW: usize,
    SH: usize,
    pDestinationImage: &mut Vec<u8>,
    W: usize,
    H: usize,
    BPP: usize,
) {
    thread_pool.install(|| {
        pDestinationImage
            .par_chunks_exact_mut(W * BPP)
            .enumerate()
            .for_each(|(y, write_chunk)| {
                let v = y as f32 / (H - 1) as f32;
                let mut aSample: [u8; 4] = Default::default();

                for x in 0..W as i32 {
                    let u = x as f32 / (W - 1) as f32;
                    SampleBicubic(pSourceImage, u, v, SW, SH, BPP, aSample.as_mut_slice());

                    for i in 0..BPP as usize {
                        write_chunk[x as usize * BPP as usize + i as usize] = aSample[i];
                    }
                }
            });
    });
}

pub fn ResizeImage(
    thread_pool: &Arc<rayon::ThreadPool>,
    pImageData: &[u8],
    Width: usize,
    Height: usize,
    NewWidth: usize,
    NewHeight: usize,
    BPP: usize,
) -> Vec<u8> {
    let mut img_data = Vec::<u8>::new();
    img_data.resize(
        NewWidth as usize * NewHeight as usize * BPP as usize,
        Default::default(),
    );

    ResizeImageInner(
        thread_pool,
        pImageData,
        Width,
        Height,
        &mut img_data,
        NewWidth,
        NewHeight,
        BPP,
    );

    return img_data;
}

pub fn Resize(
    thread_pool: &Arc<rayon::ThreadPool>,
    pData: &[u8],
    Width: usize,
    Height: usize,
    NewWidth: usize,
    NewHeight: usize,
    BPP: usize,
) -> Vec<u8> {
    return ResizeImage(thread_pool, pData, Width, Height, NewWidth, NewHeight, BPP);
}

pub fn Texture2DTo3D(
    thread_pool: &Arc<rayon::ThreadPool>,
    pImageBuffer: &[u8],
    ImageWidth: usize,
    ImageHeight: usize,
    ImageColorChannelCount: usize,
    SplitCountWidth: usize,
    SplitCountHeight: usize,
    pTarget3DImageData: &mut [u8],
    Target3DImageWidth: &mut usize,
    Target3DImageHeight: &mut usize,
) -> bool {
    *Target3DImageWidth = ImageWidth / SplitCountWidth;
    *Target3DImageHeight = ImageHeight / SplitCountHeight;

    let FullImageWidth = ImageWidth as usize * ImageColorChannelCount as usize;

    let TargetImageFullWidth = *Target3DImageWidth as usize * ImageColorChannelCount as usize;
    thread_pool.install(|| {
        pTarget3DImageData
            .par_chunks_exact_mut(TargetImageFullWidth)
            .enumerate()
            .for_each(|(index, write_chunk)| {
                let x_src = (index / *Target3DImageHeight) % SplitCountWidth;
                let y_src = index % *Target3DImageHeight
                    + ((index / (SplitCountWidth * *Target3DImageHeight)) * *Target3DImageHeight);
                let src_off = y_src * FullImageWidth + (x_src * TargetImageFullWidth);

                write_chunk.copy_from_slice(&pImageBuffer[src_off..src_off + TargetImageFullWidth]);
            });
    });

    return true;
}

pub fn HighestBit(OfVarParam: u32) -> u32 {
    let mut OfVar = OfVarParam;
    if OfVar == 0 {
        return 0;
    }

    let mut RetV = 1;

    loop {
        OfVar >>= 1;
        if OfVar == 0 {
            break;
        }
        RetV <<= 1;
    }

    return RetV;
}
