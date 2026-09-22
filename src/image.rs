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
