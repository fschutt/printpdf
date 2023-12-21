//! Utilities to work with path objects.

use crate::glob_defines::{
    OP_PATH_CONST_CLIP_EO, OP_PATH_CONST_CLIP_NZ, OP_PATH_PAINT_FILL_EO, OP_PATH_PAINT_FILL_NZ,
    OP_PATH_PAINT_FILL_STROKE_CLOSE_EO, OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ,
};

#[derive(Debug, Clone, Copy)]
pub enum WindingOrder {
    EvenOdd,
    NonZero,
}

impl Default for WindingOrder {
    fn default() -> Self {
        WindingOrder::NonZero
    }
}

impl WindingOrder {
    pub fn get_clip_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_CONST_CLIP_NZ,
            WindingOrder::EvenOdd => OP_PATH_CONST_CLIP_EO,
        }
    }

    pub fn get_fill_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_PAINT_FILL_NZ,
            WindingOrder::EvenOdd => OP_PATH_PAINT_FILL_EO,
        }
    }

    pub fn get_fill_stroke_close_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ,
            WindingOrder::EvenOdd => OP_PATH_PAINT_FILL_STROKE_CLOSE_EO,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PaintMode {
    Clip,
    Fill,
    Stroke,
    FillStroke,
}

impl Default for PaintMode {
    fn default() -> PaintMode {
        PaintMode::Fill
    }
}
