//! Private bilateral backend (edge-preserving, clamped borders).
//! Gaussian weights via branchless fast-exp (no libm, no_std-safe).

extern crate alloc;

use crate::image::Image;

/// e^-x for x >= 0 (Schraudolph approx, clamped to [0, 1]).
fn fast_exp_neg(x: f32) -> f32 {
    if x <= 0.0 {
        return 1.0;
    }
    if x > 25.0 {
        return 0.0;
    }
    let v = 12102203.0 * (-x) + 1065353216.0;
    f32::from_bits(v as u32).clamp(0.0, 1.0)
}

/// Bilateral filter: spatial sigma + color sigma.
/// radius 0 = copy. Non-positive sigmas fall back to plain box weights.
pub(crate) fn bilateral(
    img: &Image, radius: u32, sigma_space: f32, sigma_color: f32,
) -> Image {
    let (w, h) = (img.width(), img.height());
    if radius == 0 || w == 0 || h == 0 {
        return Image::from_rgb(w, h, img.as_rgb().to_vec());
    }
    let src = img.as_rgb();
    let r = radius as i32;
    let ss2 = 2.0 * sigma_space * sigma_space;
    let sc2 = 2.0 * sigma_color * sigma_color;
    let use_gauss = ss2 > 0.0 && sc2 > 0.0;
    let mut out = alloc::vec![0u8; src.len()];
    for y in 0..h {
        for x in 0..w {
            let c0 = ((y * w + x) * 3) as usize;
            let (r0, g0, b0) =
                (src[c0] as f32, src[c0 + 1] as f32, src[c0 + 2] as f32);
            let mut acc = [0.0f32; 3];
            let mut wsum = 0.0f32;
            for dy in -r..=r {
                let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                for dx in -r..=r {
                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                    let i = ((sy * w + sx) * 3) as usize;
                    let wt = if use_gauss {
                        let ds =
                            (dx * dx + dy * dy) as f32;
                        let dr = src[i] as f32 - r0;
                        let dg = src[i + 1] as f32 - g0;
                        let db = src[i + 2] as f32 - b0;
                        let dc = dr * dr + dg * dg + db * db;
                        fast_exp_neg(ds / ss2 + dc / sc2)
                    } else {
                        1.0
                    };
                    acc[0] += src[i] as f32 * wt;
                    acc[1] += src[i + 1] as f32 * wt;
                    acc[2] += src[i + 2] as f32 * wt;
                    wsum += wt;
                }
            }
            let d = ((y * w + x) * 3) as usize;
            let inv = if wsum > 0.0 { 1.0 / wsum } else { 0.0 };
            out[d] = (acc[0] * inv + 0.5).clamp(0.0, 255.0) as u8;
            out[d + 1] = (acc[1] * inv + 0.5).clamp(0.0, 255.0) as u8;
            out[d + 2] = (acc[2] * inv + 0.5).clamp(0.0, 255.0) as u8;
        }
    }
    Image::from_rgb(w, h, out)
}
