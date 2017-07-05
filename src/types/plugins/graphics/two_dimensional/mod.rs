//! 2D elements for Pdf (stub), to be expanded

pub mod point;
pub mod line;
pub mod font;
pub mod svg;

pub use self::point::Point;
pub use self::line::Line;
pub use self::font::*;
pub use self::svg::Svg;

use std::sync::{Arc, Mutex};
use super::Color;

lazy_static! {
    static ref CURRENT_OUTLINE_COLOR: Arc<Mutex<Color>> = 
    Arc::new(Mutex::new(
        Color::Rgb(super::color::Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None })
    ));
}


lazy_static! {
    static ref CURRENT_FILL_COLOR: Arc<Mutex<Color>> = 
    Arc::new(Mutex::new(
        Color::Rgb(super::color::Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None })
    ));
}