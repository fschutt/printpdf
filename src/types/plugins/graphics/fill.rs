//! Fill style, shared between 2D and 3D module

extern crate lopdf;

use *;

/// Fill color
#[derive(Debug, Clone)]
pub struct Fill {
    pub color: Color,
}

impl Fill {
    pub fn new(color: Color)
    -> Self
    {
        Self {
            color,
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