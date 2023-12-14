//! Utilities for rectangle paths.

use crate::{Mm, Point};

/// A helper struct to insert rectangular shapes into a PDF.
///
/// This can be used to paint rectangles or to clip other paths.
#[derive(Debug, Copy, Clone)]
pub struct Rect {
    /// x position from the bottom left corner in pt
    pub ll: Point,
    /// y position from the bottom left corner in pt
    pub ur: Point,
}

impl Rect {
    /// Create a new point.
    /// **WARNING: The reference point for a point is the bottom left corner, not the top left**
    #[inline]
    pub fn new(llx: Mm, lly: Mm, urx: Mm, ury: Mm) -> Self {
        Self {
            ll: Point {
                x: llx.into(),
                y: lly.into(),
            },
            ur: Point {
                x: urx.into(),
                y: ury.into(),
            },
        }
    }
}

impl PartialEq for Rect {
    // custom compare function because of floating point inaccuracy
    fn eq(&self, other: &Rect) -> bool {
        self.ll == other.ll && self.ur == other.ur
    }
}
