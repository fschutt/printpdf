extern crate lopdf;

use *;

#[derive(Debug, Copy, Clone)]
pub struct Point { 
    /// x position from the bottom left corner in pt
    pub x: f64, 
    /// y position from the bottom left corner in pt
    pub y: f64 
}

impl Point {

    /// Create a new point. 
    /// **WARNING: The reference point for a point is the bottom left corner, not the top left**
    #[inline]
    pub fn new(x_mm: f64, y_mm: f64)
    -> Self
    {
        Self {
            x: mm_to_pt!(x_mm),
            y: mm_to_pt!(y_mm),
        }
    }
}

impl PartialEq for Point {
    // custom compare function because of floating point inaccuracy
    fn eq(&self, other: &Point) -> bool {
        if self.x.is_normal() && other.x.is_normal() &&
           self.y.is_normal() && other.y.is_normal() {
            // four floating point numbers have to match
            let x_eq = (self.x * 1000.0).round() == (other.x * 1000.0).round();
            if !x_eq { return false; }
            let y_eq = (self.y * 1000.0).round() == (other.y * 1000.0).round();
            if y_eq { return true; }
        }
        return false;
    }
}
