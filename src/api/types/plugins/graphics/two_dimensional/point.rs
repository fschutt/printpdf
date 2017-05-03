extern crate lopdf;

use traits::*;

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
    pub fn new(x_mm: f64, y_mm: f64)
    -> Self
    {
        Self {
            x: mm_to_pt!(x_mm),
            y: mm_to_pt!(y_mm),
        }
    }
}

impl IntoPdfStreamOperation for Point {
    fn into(self)
    -> lopdf::content::Operation 
    {
        unimplemented!()
    }
}


