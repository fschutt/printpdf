//! Module for graphics (line, polygon, 3D, etc.)

pub mod two_dimensional;
pub mod three_dimensional;

pub use self::two_dimensional::*;
pub use self::three_dimensional::*;

pub mod outline;
pub mod fill;
pub mod style;
pub mod color;
pub mod icc_profile;

pub use self::style::Style;
pub use self::outline::Outline;
pub use self::fill::Fill;
pub use self::color::{Color, Rgb, Cmyk, Grayscale};
pub use self::icc_profile::{IccProfile, IccProfileType};
