use {Mm, Pt};

#[derive(Debug, Copy, Clone)]
pub struct Point {
    /// x position from the bottom left corner in pt
    pub x: Pt,
    /// y position from the bottom left corner in pt
    pub y: Pt,
}

impl Point {

    /// Create a new point.
    /// **WARNING: The reference point for a point is the bottom left corner, not the top left**
    #[inline]
    pub fn new(x: Mm, y: Mm)
    -> Self
    {
        Self {
            x: x.into(),
            y: y.into(),
        }
    }
}

impl PartialEq for Point {

    // custom compare function because of floating point inaccuracy
    fn eq(&self, other: &Point) -> bool {

        if self.x.0.is_normal() && other.x.0.is_normal() &&
           self.y.0.is_normal() && other.y.0.is_normal() {
            // four floating point numbers have to match
            let x_eq = self.x == other.x;
            if !x_eq { return false; }
            let y_eq = self.y == other.y;
            if y_eq { return true; }
        }

        false
    }
}
