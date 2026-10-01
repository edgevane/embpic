//! Public resize API: pick a resizer, run it.
//!
//! ```rust,no_run
//! use embpic::{resize::resizers, Image};
//! let img = Image::new(320, 240);
//! let big = img.resize(&resizers::bilinear(1080, 1920));
//! let fast = img.resize(&resizers::nearest(160, 120));
//! ```

#[path = "resize/bilinear.rs"]
mod bilinear;
#[path = "resize/nearest.rs"]
mod nearest;

use crate::image::Image;

/// Anything that maps an image to a resized image.
pub trait Resizer {
    fn resize(&self, img: &Image) -> Image;
}

/// Ready-made resizers.
pub mod resizers {
    use super::{bilinear, nearest, Image, Resizer};

    /// Smooth bilinear resizer (default choice).
    pub struct Bilinear {
        pub width: u32,
        pub height: u32,
    }

    /// Fast nearest-neighbor resizer (blocky).
    pub struct Nearest {
        pub width: u32,
        pub height: u32,
    }

    /// Bilinear resize to `width` x `height`.
    pub fn bilinear(width: u32, height: u32) -> Bilinear {
        Bilinear { width, height }
    }

    /// Nearest-neighbor resize to `width` x `height`.
    pub fn nearest(width: u32, height: u32) -> Nearest {
        Nearest { width, height }
    }

    impl Resizer for Bilinear {
        fn resize(&self, img: &Image) -> Image {
            bilinear::bilinear(img, self.width, self.height)
        }
    }

    impl Resizer for Nearest {
        fn resize(&self, img: &Image) -> Image {
            nearest::nearest(img, self.width, self.height)
        }
    }
}
