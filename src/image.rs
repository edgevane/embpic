extern crate alloc;
use alloc::vec::Vec;

use crate::Color;

pub struct Image {
    width: u32,
    height: u32,
    buf: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width as usize) * (height as usize) * 3;
        Self {
            width,
            height,
            buf: alloc::vec![0; len],
        }
    }

    pub fn from_rgb(width: u32, height: u32, buf: Vec<u8>) -> Self {
        debug_assert_eq!(buf.len(), (width as usize) * (height as usize) * 3);
        Self { width, height, buf }
    }

    /// Load image by file extension: `.jpg` / `.jpeg` supported.
    pub fn load(path: &str) -> Result<Self, LoadError> {
        if has_jpg_ext(path) {
            crate::internal::jpg::load(path).map_err(LoadError::Jpg)
        } else {
            Err(LoadError::UnknownExtension)
        }
    }

    /// Save image by file extension: `.jpg` / `.jpeg` supported.
    pub fn save(&self, path: &str) -> Result<(), LoadError> {
        if has_jpg_ext(path) {
            crate::internal::jpg::save(self, path).map_err(LoadError::Jpg)
        } else {
            Err(LoadError::UnknownExtension)
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn as_rgb(&self) -> &[u8] {
        &self.buf
    }

    pub fn put_pixel(&mut self, x: u32, y: u32, c: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = ((y * self.width + x) * 3) as usize;
        self.buf[i] = c.r;
        self.buf[i + 1] = c.g;
        self.buf[i + 2] = c.b;
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = ((y * self.width + x) * 3) as usize;
        Some(Color::rgb(self.buf[i], self.buf[i + 1], self.buf[i + 2]))
    }

    /// Nearest-neighbor resize. Fast, blocky; use `resize` for smooth.
    /// Returns a new image; `Self` untouched.
    pub fn resize_nearest(&self, new_width: u32, new_height: u32) -> Self {
        if new_width == 0 || new_height == 0 || self.width == 0 || self.height == 0
        {
            return Self::new(new_width, new_height);
        }
        let mut out = Self::new(new_width, new_height);
        for y in 0..new_height {
            let sy = (y * self.height / new_height).min(self.height - 1);
            for x in 0..new_width {
                let sx = (x * self.width / new_width).min(self.width - 1);
                let s = ((sy * self.width + sx) * 3) as usize;
                let d = ((y * new_width + x) * 3) as usize;
                out.buf[d] = self.buf[s];
                out.buf[d + 1] = self.buf[s + 1];
                out.buf[d + 2] = self.buf[s + 2];
            }
        }
        out
    }

    /// Bilinear resize. Returns a new image; `Self` untouched.
    /// Corners map exactly; zero `new_width`/`new_height` yields empty.
    pub fn resize(&self, new_width: u32, new_height: u32) -> Self {
        if new_width == 0 || new_height == 0 || self.width == 0 || self.height == 0
        {
            return Self::new(new_width, new_height);
        }
        if self.width == 1 && self.height == 1 {
            let mut out = Self::new(new_width, new_height);
            for px in out.buf.chunks_exact_mut(3) {
                px.copy_from_slice(&self.buf[..3]);
            }
            return out;
        }
        let mut out = Self::new(new_width, new_height);
        let x_scale = if new_width > 1 {
            (self.width - 1) as f32 / (new_width - 1) as f32
        } else {
            0.0
        };
        let y_scale = if new_height > 1 {
            (self.height - 1) as f32 / (new_height - 1) as f32
        } else {
            0.0
        };
        for y in 0..new_height {
            let sy = y as f32 * y_scale;
            let y0 = (sy as u32).min(self.height - 1);
            let y1 = (y0 + 1).min(self.height - 1);
            let fy = sy - y0 as f32;
            for x in 0..new_width {
                let sx = x as f32 * x_scale;
                let x0 = (sx as u32).min(self.width - 1);
                let x1 = (x0 + 1).min(self.width - 1);
                let fx = sx - x0 as f32;
                let d = ((y * new_width + x) * 3) as usize;
                for c in 0..3 {
                    let p00 = self.buf[((y0 * self.width + x0) * 3) as usize + c] as f32;
                    let p10 = self.buf[((y0 * self.width + x1) * 3) as usize + c] as f32;
                    let p01 = self.buf[((y1 * self.width + x0) * 3) as usize + c] as f32;
                    let p11 = self.buf[((y1 * self.width + x1) * 3) as usize + c] as f32;
                    let top = p00 + (p10 - p00) * fx;
                    let bot = p01 + (p11 - p01) * fx;
                    let v = top + (bot - top) * fy;
                    out.buf[d + c] = v.clamp(0.0, 255.0) as u8;
                }
            }
        }
        out
    }
}

fn has_jpg_ext(path: &str) -> bool {
    let b = path.as_bytes();
    let check = |s: &[u8]| {
        b.len() >= s.len()
            && b[b.len() - s.len()..]
                .iter()
                .zip(s.iter())
                .all(|(a, c)| a.to_ascii_lowercase() == *c)
    };
    check(b".jpg") || check(b".jpeg")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    UnknownExtension,
    Jpg(crate::internal::JpgError),
}
