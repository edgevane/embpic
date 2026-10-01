//! Private denoise backends (called only via `crate::denoise`).
//! All border handling: edge replication (clamp).

extern crate alloc;

use crate::image::Image;

/// Naive box blur, clamped borders. radius 0 = copy.
pub(crate) fn box_blur(img: &Image, radius: u32) -> Image {
    let (w, h) = (img.width(), img.height());
    if radius == 0 || w == 0 || h == 0 {
        return Image::from_rgb(w, h, img.as_rgb().to_vec());
    }
    let src = img.as_rgb();
    let r = radius as i32;
    let mut out = alloc::vec![0u8; src.len()];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 3];
            let mut n = 0u32;
            for dy in -r..=r {
                let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                for dx in -r..=r {
                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                    let i = ((sy * w + sx) * 3) as usize;
                    acc[0] += src[i] as u32;
                    acc[1] += src[i + 1] as u32;
                    acc[2] += src[i + 2] as u32;
                    n += 1;
                }
            }
            let d = ((y * w + x) * 3) as usize;
            out[d] = (acc[0] / n) as u8;
            out[d + 1] = (acc[1] / n) as u8;
            out[d + 2] = (acc[2] / n) as u8;
        }
    }
    Image::from_rgb(w, h, out)
}
