//! Private median backend (per-channel median, clamped borders).

extern crate alloc;
use alloc::vec::Vec;

use crate::image::Image;

/// Median over a (2*radius+1)^2 window, per channel. radius 0 = copy.
pub(crate) fn median(img: &Image, radius: u32) -> Image {
    let (w, h) = (img.width(), img.height());
    if radius == 0 || w == 0 || h == 0 {
        return Image::from_rgb(w, h, img.as_rgb().to_vec());
    }
    let src = img.as_rgb();
    let r = radius as i32;
    let mut out = alloc::vec![0u8; src.len()];
    let mut win: Vec<u8> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let d = ((y * w + x) * 3) as usize;
            for c in 0..3 {
                win.clear();
                for dy in -r..=r {
                    let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                    for dx in -r..=r {
                        let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                        win.push(src[((sy * w + sx) * 3) as usize + c]);
                    }
                }
                win.sort_unstable();
                out[d + c] = win[win.len() / 2];
            }
        }
    }
    Image::from_rgb(w, h, out)
}
