//! Wrapper for Fill / Outline

use *;
#[derive(Debug, Clone)]
pub struct Style {
    pub fill: Option<Fill>,
    pub outline: Option<Outline>,
}

impl Style {

    /// Creates a new style
    pub fn new(fill: Option<Fill>, outline: Option<Outline>)
    -> Self
    {
        Self {
            fill,
            outline,
        }
    }
}

impl IntoPdfStreamOperation for Style {

    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>
    {
        operation!(PDF_TAG_END_LINE_FILL)
    }
}
