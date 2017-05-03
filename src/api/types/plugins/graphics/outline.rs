extern crate lopdf;

use super::*;
use traits::*;
use glob_defines::*;

#[derive(Debug, Clone)]
pub struct Outline {
    pub color: Color,
    pub thickness: u8,
    /* pattern, etc */
}

impl Outline {
    
    pub fn new(color: Color, thickness: u8)
    -> Self
    {
        Self {
            color,
            thickness,
        }
    }
}

impl IntoPdfStreamOperation for Outline {
    fn into(self)
    -> lopdf::content::Operation
    {
        operation!(PDF_TAG_END_LINE_OUTLINE)
    }
}