//! Private contrast backend (linear stretch around mid-gray).

extern crate alloc;

use crate::image::Image;

/// High contrast: v' = (v - 128) * factor + 128, clamped.
/// factor 1.0 = no-op. factor 0.0 = flat mid-gray.
pub(crate) fn high_contrast(img: &Image, factor: f32) -> Image {
    let (w, h) = (img.width(), img.height());
    let src = img.as_rgb();
    let mut out = alloc::vec![0u8; src.len()];
    for (d, s) in out.iter_mut().zip(src.iter()) {
        *d = ((*s as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;
    }
    Image::from_rgb(w, h, out)
}
