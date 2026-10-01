#![no_std]

extern crate alloc;

pub mod color;
pub mod image;
mod internal;

pub use color::Color;
pub use image::{Image, LoadError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_black() {
        let img = Image::new(2, 2);
        assert_eq!(img.as_rgb(), &[0; 12]);
    }

    #[test]
    fn put_get_pixel() {
        let mut img = Image::new(10, 10);
        img.put_pixel(3, 4, Color::rgb(255, 128, 0));
        assert_eq!(img.get_pixel(3, 4), Some(Color::rgb(255, 128, 0)));
    }

    #[test]
    fn out_of_bounds_is_noop() {
        let mut img = Image::new(2, 2);
        img.put_pixel(5, 5, Color::WHITE);
        assert_eq!(img.get_pixel(5, 5), None);
        assert_eq!(img.as_rgb(), &[0; 12]);
    }

    #[test]
    fn resize_up_bilinear() {
        let mut img = Image::new(2, 2);
        img.put_pixel(0, 0, Color::rgb(0, 0, 0));
        img.put_pixel(1, 0, Color::rgb(100, 0, 0));
        img.put_pixel(0, 1, Color::rgb(0, 100, 0));
        img.put_pixel(1, 1, Color::rgb(0, 0, 100));
        // corners map exactly
        let big = img.resize(4, 4);
        assert_eq!(big.width(), 4);
        assert_eq!(big.height(), 4);
        assert_eq!(big.get_pixel(0, 0), Some(Color::rgb(0, 0, 0)));
        assert_eq!(big.get_pixel(3, 0), Some(Color::rgb(100, 0, 0)));
        assert_eq!(big.get_pixel(0, 3), Some(Color::rgb(0, 100, 0)));
        assert_eq!(big.get_pixel(3, 3), Some(Color::rgb(0, 0, 100)));
        // 2x2 -> 3x3 center = exact average of corners
        let mid = img.resize(3, 3);
        assert_eq!(mid.get_pixel(1, 1), Some(Color::rgb(25, 25, 25)));
    }

    #[test]
    fn resize_nearest_keeps_blocks() {
        let mut img = Image::new(2, 2);
        img.put_pixel(0, 0, Color::RED);
        img.put_pixel(1, 0, Color::GREEN);
        img.put_pixel(0, 1, Color::BLUE);
        img.put_pixel(1, 1, Color::WHITE);
        let big = img.resize_nearest(4, 4);
        assert_eq!(big.get_pixel(0, 0), Some(Color::RED));
        assert_eq!(big.get_pixel(1, 1), Some(Color::RED));
        assert_eq!(big.get_pixel(3, 3), Some(Color::WHITE));
    }

    #[test]
    fn resize_down_and_zero() {
        let img = Image::new(4, 4);
        let small = img.resize(2, 2);
        assert_eq!((small.width(), small.height()), (2, 2));
        let empty = img.resize(0, 10);
        assert_eq!(empty.as_rgb(), &[]);
    }
}
