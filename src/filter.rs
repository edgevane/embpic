//! Public filter API: pick a filter, run it.
//!
//! ```rust,no_run
//! use embpic::{filter::filters, Image};
//! let img = Image::new(320, 240);
//! let soft = img.filter(&filters::gaussian(1.5));
//! let gray = img.filter(&filters::grayscale());
//! let punch = img.filter(&filters::high_contrast(1.8));
//! ```

#[path = "filter/gaussian.rs"]
mod gaussian;
#[path = "filter/grayscale.rs"]
mod grayscale;
#[path = "filter/high_contrast.rs"]
mod high_contrast;

use crate::image::Image;

/// Anything that maps an image to a filtered image.
pub trait Filter {
    fn apply(&self, img: &Image) -> Image;
}

/// Ready-made filters.
pub mod filters {
    use super::{gaussian, grayscale, high_contrast, Filter, Image};

    /// Separable gaussian blur. `sigma <= 0` = no-op copy.
    pub struct Gaussian {
        pub sigma: f32,
    }

    /// Grayscale via Rec.601 luma (result stays RGB).
    pub struct Grayscale;

    /// Linear contrast stretch around mid-gray.
    /// `factor` 1.0 = no-op, 0.0 = flat gray, >1 = punchier.
    pub struct HighContrast {
        pub factor: f32,
    }

    /// Gaussian blur with `sigma`.
    pub fn gaussian(sigma: f32) -> Gaussian {
        Gaussian { sigma }
    }

    /// Grayscale conversion.
    pub fn grayscale() -> Grayscale {
        Grayscale
    }

    /// Contrast stretch with `factor`.
    pub fn high_contrast(factor: f32) -> HighContrast {
        HighContrast { factor }
    }

    impl Filter for Gaussian {
        fn apply(&self, img: &Image) -> Image {
            gaussian::gaussian(img, self.sigma)
        }
    }

    impl Filter for Grayscale {
        fn apply(&self, img: &Image) -> Image {
            grayscale::grayscale(img)
        }
    }

    impl Filter for HighContrast {
        fn apply(&self, img: &Image) -> Image {
            high_contrast::high_contrast(img, self.factor)
        }
    }
}
