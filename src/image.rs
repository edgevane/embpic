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
