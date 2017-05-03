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

    fn into(self)
    -> lopdf::content::Operation
    {
        operation!(PDF_TAG_END_LINE_FILL)
    }
}