//! Utilities for rectangle paths.

use crate::path::{PaintMode, WindingOrder};
use crate::{Mm, Point, OP_PATH_CONST_RECT, OP_PATH_PAINT_END, OP_PATH_PAINT_STROKE};

/// A helper struct to insert rectangular shapes into a PDF.
///
/// This can be used to paint rectangles or to clip other paths.
#[derive(Debug, Copy, Clone)]
pub struct Rect {
    /// Position of the lower left point of the rectangle, relative to the bottom left corner of
    /// the PDF page in pt.
    pub ll: Point,
    /// Position of the upper right point of the rectangle, relative to the bottom left corner of
    /// the PDF page in pt.
    pub ur: Point,
    /// The paint mode of the rectangle.
    pub mode: PaintMode,
    /// The path-painting/clipping path operator.
    pub winding: WindingOrder,
}

impl Rect {
    /// Create a new point.
    ///
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
            mode: PaintMode::default(),
            winding: WindingOrder::default(),
        }
    }

    /// Returns a new `Rect` with the specified `mode`.
    #[inline]
    #[must_use]
    pub fn with_mode(mut self, mode: PaintMode) -> Self {
        self.mode = mode;
        self
    }

    /// Returns a new `Rect` with the specified `winding`.
    #[inline]
    #[must_use]
    pub fn with_winding(mut self, winding: WindingOrder) -> Self {
        self.winding = winding;
        self
    }

    /// Transform the `Rect` into a `Vec` of PDF [`Operation`]s.
    ///
    /// [`Operation`]: lopdf::content::Operation
    #[must_use]
    pub fn into_stream_op(self) -> Vec<lopdf::content::Operation> {
        use lopdf::content::Operation;

        let width = self.ur.x - self.ll.x;
        let height = self.ur.y - self.ll.y;

        let rect_op = Operation::new(
            OP_PATH_CONST_RECT,
            vec![self.ll.x.into(), self.ll.y.into(), width.into(), height.into()],
        );

        let paint_op = match self.mode {
            PaintMode::Clip => Operation::new(self.winding.get_clip_op(), vec![]),
            PaintMode::Fill => Operation::new(self.winding.get_fill_op(), vec![]),
            PaintMode::Stroke => Operation::new(OP_PATH_PAINT_STROKE, vec![]),
            PaintMode::FillStroke => Operation::new(self.winding.get_fill_stroke_op(), vec![]),
        };

        if matches!(self.mode, PaintMode::Clip) {
            vec![rect_op, paint_op, Operation::new(OP_PATH_PAINT_END, vec![])]
        } else {
            vec![rect_op, paint_op]
        }
    }
}

impl PartialEq for Rect {
    // custom compare function because of floating point inaccuracy
    fn eq(&self, other: &Rect) -> bool {
        self.ll == other.ll && self.ur == other.ur
    }
}
