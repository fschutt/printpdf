//! Utilities to work with path objects.

use crate::glob_defines::{
    OP_PATH_CONST_CLIP_EO, OP_PATH_CONST_CLIP_NZ, OP_PATH_PAINT_FILL_EO, OP_PATH_PAINT_FILL_NZ,
    OP_PATH_PAINT_FILL_STROKE_CLOSE_EO, OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ,
};

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
