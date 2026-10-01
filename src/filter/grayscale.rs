//! Private grayscale backend (Rec.601 luma, rounded).

extern crate alloc;

use crate::image::Image;

/// Grayscale: v = 0.299R + 0.587G + 0.114B, replicated to RGB.
pub(crate) fn grayscale(img: &Image) -> Image {
    let (w, h) = (img.width(), img.height());
    let src = img.as_rgb();
    let mut out = alloc::vec![0u8; src.len()];
    for (d, s) in out.chunks_exact_mut(3).zip(src.chunks_exact(3)) {
        let v = (0.299 * s[0] as f32 + 0.587 * s[1] as f32 + 0.114 * s[2] as f32
            + 0.5)
            .clamp(0.0, 255.0) as u8;
        d[0] = v;
        d[1] = v;
        d[2] = v;
    }
    Image::from_rgb(w, h, out)
}
