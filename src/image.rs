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

    /// Load image by file extension: `.jpg` / `.jpeg` / `.webp` supported.
    pub fn load(path: &str) -> Result<Self, LoadError> {
        if has_jpg_ext(path) {
            crate::internal::jpg::load(path).map_err(LoadError::Jpg)
        } else if has_webp_ext(path) {
            crate::internal::webp::load(path).map_err(LoadError::Webp)
        } else {
            Err(LoadError::UnknownExtension)
        }
    }

    /// Save image by file extension: `.jpg` / `.jpeg` / `.webp` supported.
    pub fn save(&self, path: &str) -> Result<(), LoadError> {
        if has_jpg_ext(path) {
            crate::internal::jpg::save(self, path).map_err(LoadError::Jpg)
        } else if has_webp_ext(path) {
            crate::internal::webp::save(self, path).map_err(LoadError::Webp)
        } else {
            Err(LoadError::UnknownExtension)
        }
    }

    /// Denoise via any [`crate::denoise::Denoiser`]. Returns a new image.
    pub fn denoise(&self, d: &impl crate::denoise::Denoiser) -> Self {
        d.denoise(self)
    }

    /// Filter via any [`crate::filter::Filter`]. Returns a new image.
    pub fn filter(&self, f: &impl crate::filter::Filter) -> Self {
        f.apply(self)
    }

    /// Per-channel min-max normalize: each channel stretched so its
    /// min maps to 0 and max to 255. Flat channels pass through.
    /// Returns a new image; `Self` untouched.
    pub fn normalize(&self) -> Self {
        let src = self.as_rgb();
        let mut min = [255u8; 3];
        let mut max = [0u8; 3];
        for px in src.chunks_exact(3) {
            for c in 0..3 {
                min[c] = min[c].min(px[c]);
                max[c] = max[c].max(px[c]);
            }
        }
        let mut out = Vec::with_capacity(src.len());
        out.extend_from_slice(src);
        for px in out.chunks_exact_mut(3) {
            for c in 0..3 {
                if max[c] > min[c] {
                    px[c] = (((px[c] as u32 - min[c] as u32) * 255)
                        / (max[c] as u32 - min[c] as u32))
                        as u8;
                }
            }
        }
        Self { width: self.width, height: self.height, buf: out }
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

    /// Resize via any [`crate::resize::Resizer`]. Returns a new image.
    pub fn resize(&self, r: &impl crate::resize::Resizer) -> Self {
        r.resize(self)
    }
}

fn has_jpg_ext(path: &str) -> bool {
    has_ext(path, b".jpg") || has_ext(path, b".jpeg")
}

fn has_webp_ext(path: &str) -> bool {
    has_ext(path, b".webp")
}

fn has_ext(path: &str, s: &[u8]) -> bool {
    let b = path.as_bytes();
    b.len() >= s.len()
        && b[b.len() - s.len()..]
            .iter()
            .zip(s.iter())
            .all(|(a, c)| a.to_ascii_lowercase() == *c)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    UnknownExtension,
    Jpg(crate::internal::JpgError),
    Webp(crate::internal::WebpError),
}
