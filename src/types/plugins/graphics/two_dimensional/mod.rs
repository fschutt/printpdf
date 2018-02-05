//! 2D elements for Pdf (stub), to be expanded

pub mod point;
pub mod line;
pub mod font;
// pub mod svg;
pub mod image;

pub use self::point::Point;
pub use self::line::Line;
pub use self::font::*;
// pub use self::svg::Svg;
pub use self::image::Image;