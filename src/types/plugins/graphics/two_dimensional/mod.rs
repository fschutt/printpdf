//! 2D elements for Pdf (stub), to be expanded

pub mod point;
pub mod line;
pub mod font;
pub mod svg;

pub use self::point::Point;
pub use self::line::Line;
pub use self::font::Font;
pub use self::svg::Svg;

use std::sync::{Arc, Mutex};
use super::{Outline, Fill, Color};

lazy_static! {
    static ref CURRENT_OUTLINE: Arc<Mutex<Outline>> = 
    Arc::new(Mutex::new(Outline { 
        color: Color::Rgb(super::color::Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }), 
        thickness: 5 
    }));
}


lazy_static! {
    static ref CURRENT_FILL: Arc<Mutex<Fill>> = 
    Arc::new(Mutex::new(Fill {
        color: Color::Rgb(super::color::Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }),
    }));
}