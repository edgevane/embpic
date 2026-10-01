//! Private bilinear backend.

extern crate alloc;

use crate::image::Image;

/// Bilinear resize. Corners map exactly; zero dims yield empty.
pub(crate) fn bilinear(img: &Image, new_width: u32, new_height: u32) -> Image {
    let (w, h) = (img.width(), img.height());
    if new_width == 0
        || new_height == 0
        || w == 0
        || h == 0
    {
        return Image::new(new_width, new_height);
    }
    if w == 1 && h == 1 {
        let px = &img.as_rgb()[..3];
        let mut buf = alloc::vec![0u8; (new_width * new_height * 3) as usize];
        for dst in buf.chunks_exact_mut(3) {
            dst.copy_from_slice(px);
        }
        return Image::from_rgb(new_width, new_height, buf);
    }
    let mut buf = alloc::vec![0u8; (new_width * new_height * 3) as usize];
    let x_scale = if new_width > 1 {
        (w - 1) as f32 / (new_width - 1) as f32
    } else {
        0.0
    };
    let y_scale = if new_height > 1 {
        (h - 1) as f32 / (new_height - 1) as f32
    } else {
        0.0
    };
    let src = img.as_rgb();
    for y in 0..new_height {
        let sy = y as f32 * y_scale;
        let y0 = (sy as u32).min(h - 1);
        let y1 = (y0 + 1).min(h - 1);
        let fy = sy - y0 as f32;
        for x in 0..new_width {
            let sx = x as f32 * x_scale;
            let x0 = (sx as u32).min(w - 1);
            let x1 = (x0 + 1).min(w - 1);
            let fx = sx - x0 as f32;
            let d = ((y * new_width + x) * 3) as usize;
            for c in 0..3 {
                let p00 = src[((y0 * w + x0) * 3) as usize + c] as f32;
                let p10 = src[((y0 * w + x1) * 3) as usize + c] as f32;
                let p01 = src[((y1 * w + x0) * 3) as usize + c] as f32;
                let p11 = src[((y1 * w + x1) * 3) as usize + c] as f32;
                let top = p00 + (p10 - p00) * fx;
                let bot = p01 + (p11 - p01) * fx;
                let v = top + (bot - top) * fy;
                buf[d + c] = v.clamp(0.0, 255.0) as u8;
            }
        }
    }
    Image::from_rgb(new_width, new_height, buf)
}
