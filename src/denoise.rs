//! Public denoise API: pick a denoiser, run it.
//!
//! ```rust,no_run
//! use embpic::{denoise::denoisers, Image};
//! let img = Image::load("photo.jpg").unwrap();
//! let clean = img.denoise(&denoisers::mean(1));
//! let clean = img.denoise(&denoisers::median(1));
//! let clean = img.denoise(&denoisers::bilateral(2, 1.5, 25.0));
//! ```

#[path = "denoise/box_blur.rs"]
mod box_blur;
#[path = "denoise/median.rs"]
mod median;
#[path = "denoise/bilateral.rs"]
mod bilateral;

use crate::image::Image;

/// Anything that maps an image to a denoised image.
pub trait Denoiser {
    fn denoise(&self, img: &Image) -> Image;
}

/// Ready-made denoisers.
pub mod denoisers {
    use super::{bilateral, box_blur, median, Denoiser, Image};

    /// Mean (box) blur over a (2*radius+1)^2 window. Cheap, blurs edges.
    pub struct Mean {
        pub radius: u32,
    }

    /// Per-channel median over a (2*radius+1)^2 window.
    /// Kills salt-and-pepper noise, keeps edges better than mean.
    pub struct Median {
        pub radius: u32,
    }

    /// Edge-preserving bilateral filter.
    /// `sigma_space`: how far pixels mix; `sigma_color`: how different
    /// colors may be to still mix (larger = closer to plain blur).
    pub struct Bilateral {
        pub radius: u32,
        pub sigma_space: f32,
        pub sigma_color: f32,
    }

    /// Box blur with `radius` (0 = no-op copy).
    pub fn mean(radius: u32) -> Mean {
        Mean { radius }
    }

    /// Median with `radius` (0 = no-op copy).
    pub fn median(radius: u32) -> Median {
        Median { radius }
    }

    /// Bilateral with `radius`, spatial/color sigmas.
    pub fn bilateral(
        radius: u32, sigma_space: f32, sigma_color: f32,
    ) -> Bilateral {
        Bilateral { radius, sigma_space, sigma_color }
    }

    impl Denoiser for Mean {
        fn denoise(&self, img: &Image) -> Image {
            box_blur::box_blur(img, self.radius)
        }
    }

    impl Denoiser for Median {
        fn denoise(&self, img: &Image) -> Image {
            median::median(img, self.radius)
        }
    }

    impl Denoiser for Bilateral {
        fn denoise(&self, img: &Image) -> Image {
            bilateral::bilateral(
                img,
                self.radius,
                self.sigma_space,
                self.sigma_color,
            )
        }
    }
}
