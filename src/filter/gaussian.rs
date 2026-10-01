//! Private separable gaussian backend (clamped borders).

extern crate alloc;
use alloc::vec::Vec;

use crate::image::Image;
use crate::internal::math::exp_neg;

/// Separable gaussian blur. sigma <= 0 = copy.
/// Radius auto: ceil(3*sigma), capped at 16.
pub(crate) fn gaussian(img: &Image, sigma: f32) -> Image {
    let (w, h) = (img.width(), img.height());
    if sigma <= 0.0 || w == 0 || h == 0 {
        return Image::from_rgb(w, h, img.as_rgb().to_vec());
    }
    let mut radius = (sigma * 3.0 + 0.5) as u32 + 1;
    radius = radius.min(16).max(1);
    let r = radius as i32;
    let s2 = 2.0 * sigma * sigma;
    let mut kernel: Vec<f32> = Vec::with_capacity((2 * radius + 1) as usize);
    let mut ksum = 0.0f32;
    for i in -r..=r {
        let k = exp_neg((i * i) as f32 / s2);
        kernel.push(k);
        ksum += k;
    }
    let src = img.as_rgb();
    // horizontal pass into tmp (f32, unnormalized)
    let mut tmp = alloc::vec![0.0f32; src.len()];
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut acc = 0.0f32;
                for (ki, dx) in (-r..=r).enumerate() {
                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                    acc += src[((y * w + sx) * 3) as usize + c] as f32
                        * kernel[ki];
                }
                tmp[((y * w + x) * 3) as usize + c] = acc;
            }
        }
    }
    // vertical pass, normalize by ksum^2
    let norm = 1.0 / (ksum * ksum);
    let mut out = alloc::vec![0u8; src.len()];
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut acc = 0.0f32;
                for (ki, dy) in (-r..=r).enumerate() {
                    let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                    acc += tmp[((sy * w + x) * 3) as usize + c] * kernel[ki];
                }
                let d = ((y * w + x) * 3) as usize;
                out[d + c] = (acc * norm + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
    }
    Image::from_rgb(w, h, out)
}
