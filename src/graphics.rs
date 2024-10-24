use crate::units::{Mm, Pt};

use crate::constants::{
    OP_PATH_CONST_CLIP_EO, 
    OP_PATH_CONST_CLIP_NZ, 
    OP_PATH_PAINT_FILL_EO, 
    OP_PATH_PAINT_FILL_NZ,
    OP_PATH_PAINT_FILL_STROKE_CLOSE_EO, 
    OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ,
    OP_PATH_PAINT_FILL_STROKE_EO, 
    OP_PATH_PAINT_FILL_STROKE_NZ,
};

/// Rectangle struct (x, y, width, height)
#[derive(Debug, PartialEq, Clone)]
pub struct Rect {
    pub x: Pt,
    pub y: Pt,
    pub width: Pt,
    pub height: Pt,
}

/// The rule to use in filling/clipping paint operations.
///
/// This is meaningful in the following cases:
///
/// - When a path uses one of the _fill_ paint operations, this will determine the rule used to
/// fill the paths.
/// - When a path uses a [clip] painting mode, this will determine the rule used to limit the
/// regions of the page affected by painting operators.
///
/// Most of the time, `NonZero` is the appropriate option.
///
/// [clip]: PaintMode::Clip
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindingOrder {
    /// Make any filling or clipping paint operators follow the _even-odd rule_.
    ///
    /// This rule determines whether a point is inside a path by drawing a ray from that point in
    /// any direction and simply counting the number of path segments that cross the ray,
    /// regardless of direction. If this number is odd, the point is inside; if even, the point is
    /// outside. This yields the same results as the nonzero winding number rule for paths with
    /// simple shapes, but produces different results for more complex shapes.
    EvenOdd,

    /// Make any filling or clipping paint operators follow the _nonzero rule_.
    ///
    /// This rule determines whether a given point is inside a path by conceptually drawing a ray
    /// from that point to infinity in any direction and then examining the places where a segment
    /// of the path crosses the ray. Starting with a count of 0, the rule adds 1 each time a path
    /// segment crosses the ray from left to right and subtracts 1 each time a segment crosses from
    /// right to left. After counting all the crossings, if the result is 0, the point is outside
    /// the path; otherwise, it is inside.
    #[default]
    NonZero,
}

impl WindingOrder {
    /// Gets the operator for a clip paint operation.
    #[must_use]
    pub fn get_clip_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_CONST_CLIP_NZ,
            WindingOrder::EvenOdd => OP_PATH_CONST_CLIP_EO,
        }
    }

    /// Gets the operator for a fill paint operation.
    #[must_use]
    pub fn get_fill_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_PAINT_FILL_NZ,
            WindingOrder::EvenOdd => OP_PATH_PAINT_FILL_EO,
        }
    }

    /// Gets the operator for a close, fill and stroke painting operation.
    #[must_use]
    pub fn get_fill_stroke_close_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ,
            WindingOrder::EvenOdd => OP_PATH_PAINT_FILL_STROKE_CLOSE_EO,
        }
    }

    /// Gets the operator for a fill and stroke painting operation.
    #[must_use]
    pub fn get_fill_stroke_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_PAINT_FILL_STROKE_NZ,
            WindingOrder::EvenOdd => OP_PATH_PAINT_FILL_STROKE_EO,
        }
    }
}

/// The path-painting mode for a path.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PaintMode {
    /// Set the path in clipping mode instead of painting it.
    ///
    /// The path is not being drawing, but it will be used for clipping operations instead. The
    /// rule for clipping are determined by the value [`WindingOrder`] associated to the path.
    Clip,

    /// Fill the path.
    #[default]
    Fill,

    /// Paint a line along the path.
    Stroke,

    /// Fill the path and paint a line along it.
    FillStroke,
}

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
    pub fn new(x: Mm, y: Mm) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
        }
    }
}

impl PartialEq for Point {
    // custom compare function because of floating point inaccuracy
    fn eq(&self, other: &Point) -> bool {
        if self.x.0.is_normal()
            && other.x.0.is_normal()
            && self.y.0.is_normal()
            && other.y.0.is_normal()
        {
            // four floating point numbers have to match
            let x_eq = self.x == other.x;
            if !x_eq {
                return false;
            }
            let y_eq = self.y == other.y;
            if y_eq {
                return true;
            }
        }

        false
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Line {
    /// 2D Points for the line
    pub points: Vec<(Point, bool)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Polygon {
    /// 2D Points for the line
    pub rings: Vec<Line>,
    /// What type of polygon is this?
    pub mode: PaintMode,
    /// Winding order to use for constructing this polygon
    pub winding_order: WindingOrder,
}

impl FromIterator<(Point, bool)> for Polygon {
    fn from_iter<I: IntoIterator<Item = (Point, bool)>>(iter: I) -> Self {
        let mut points = Vec::new();
        for i in iter {
            points.push(i);
        }
        Polygon {
            rings: vec![Line { points }],
            ..Default::default()
        }
    }
}

