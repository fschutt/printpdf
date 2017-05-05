//! Fill style, shared between 2D and 3D module

extern crate lopdf;

use super::*;
use traits::*;
use glob_defines::*;

/// Fill color
#[derive(Debug, Clone)]
pub struct Fill {
    pub color: Color,
    pub outline: Option<Outline>,
}

impl Fill {
    pub fn new(color: Color, outline: Option<Outline>)
    -> Self
    {
        Self {
            color,
            outline
        }
    }
}


impl IntoPdfStreamOperation for Fill {

    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>
    {
        operation!(PDF_TAG_END_LINE_FILL)
    }
}