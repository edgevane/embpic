//! Private nearest-neighbor backend (integer math only).

extern crate alloc;

use crate::image::Image;

/// Nearest-neighbor resize. Fast, blocky; zero dims yield empty.
pub(crate) fn nearest(img: &Image, new_width: u32, new_height: u32) -> Image {
    let (w, h) = (img.width(), img.height());
    if new_width == 0 || new_height == 0 || w == 0 || h == 0 {
        return Image::new(new_width, new_height);
    }
    let src = img.as_rgb();
    let mut buf = alloc::vec![0u8; (new_width * new_height * 3) as usize];
    for y in 0..new_height {
        let sy = (y * h / new_height).min(h - 1);
        for x in 0..new_width {
            let sx = (x * w / new_width).min(w - 1);
            let s = ((sy * w + sx) * 3) as usize;
            let d = ((y * new_width + x) * 3) as usize;
            buf[d] = src[s];
            buf[d + 1] = src[s + 1];
            buf[d + 2] = src[s + 2];
        }
    }
    Image::from_rgb(new_width, new_height, buf)
}
