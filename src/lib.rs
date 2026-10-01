#![no_std]

extern crate alloc;

pub mod color;
pub mod denoise;
pub mod image;
pub mod resize;
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
        use crate::resize::resizers;
        let mut img = Image::new(2, 2);
        img.put_pixel(0, 0, Color::rgb(0, 0, 0));
        img.put_pixel(1, 0, Color::rgb(100, 0, 0));
        img.put_pixel(0, 1, Color::rgb(0, 100, 0));
        img.put_pixel(1, 1, Color::rgb(0, 0, 100));
        // corners map exactly
        let big = img.resize(&resizers::bilinear(4, 4));
        assert_eq!(big.width(), 4);
        assert_eq!(big.height(), 4);
        assert_eq!(big.get_pixel(0, 0), Some(Color::rgb(0, 0, 0)));
        assert_eq!(big.get_pixel(3, 0), Some(Color::rgb(100, 0, 0)));
        assert_eq!(big.get_pixel(0, 3), Some(Color::rgb(0, 100, 0)));
        assert_eq!(big.get_pixel(3, 3), Some(Color::rgb(0, 0, 100)));
        // 2x2 -> 3x3 center = exact average of corners
        let mid = img.resize(&resizers::bilinear(3, 3));
        assert_eq!(mid.get_pixel(1, 1), Some(Color::rgb(25, 25, 25)));
    }

    #[test]
    fn resize_nearest_keeps_blocks() {
        use crate::resize::resizers;
        let mut img = Image::new(2, 2);
        img.put_pixel(0, 0, Color::RED);
        img.put_pixel(1, 0, Color::GREEN);
        img.put_pixel(0, 1, Color::BLUE);
        img.put_pixel(1, 1, Color::WHITE);
        let big = img.resize(&resizers::nearest(4, 4));
        assert_eq!(big.get_pixel(0, 0), Some(Color::RED));
        assert_eq!(big.get_pixel(1, 1), Some(Color::RED));
        assert_eq!(big.get_pixel(3, 3), Some(Color::WHITE));
    }

    #[test]
    fn resize_down_and_zero() {
        use crate::resize::resizers;
        let img = Image::new(4, 4);
        let small = img.resize(&resizers::bilinear(2, 2));
        assert_eq!((small.width(), small.height()), (2, 2));
        let empty = img.resize(&resizers::bilinear(0, 10));
        assert_eq!(empty.as_rgb(), &[]);
    }

    #[test]
    fn denoise_solid_stays_solid() {
        use crate::denoise::denoisers;
        let mut img = Image::new(5, 5);
        for y in 0..5 {
            for x in 0..5 {
                img.put_pixel(x, y, Color::rgb(40, 50, 60));
            }
        }
        for out in [
            img.denoise(&denoisers::mean(1)),
            img.denoise(&denoisers::median(1)),
            img.denoise(&denoisers::bilateral(2, 1.5, 25.0)),
        ] {
            assert!(out.as_rgb().chunks_exact(3).all(|p| p == [40, 50, 60]));
        }
    }

    #[test]
    fn denoise_median_kills_salt() {
        use crate::denoise::denoisers;
        let mut img = Image::new(5, 5); // black + white dot center
        img.put_pixel(2, 2, Color::WHITE);
        let out = img.denoise(&denoisers::median(1));
        assert_eq!(out.get_pixel(2, 2), Some(Color::BLACK));
    }

    #[test]
    fn denoise_mean_blurs_dot() {
        use crate::denoise::denoisers;
        let mut img = Image::new(5, 5);
        img.put_pixel(2, 2, Color::WHITE);
        let out = img.denoise(&denoisers::mean(1));
        let c = out.get_pixel(2, 2).unwrap();
        assert!(c.r > 0 && c.r < 255, "got {c:?}");
    }
}
