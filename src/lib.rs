#![no_std]

extern crate alloc;

pub mod color;
pub mod image;

pub use color::Color;
pub use image::Image;

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
}
