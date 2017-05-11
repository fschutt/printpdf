//! Wrapper for Fill / Outline

use *;

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub fill: Fill,
    pub outline: Outline,
}

impl Style {

    /// Creates a new style
    pub fn new(fill: Fill, outline: Outline)
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

        let mut operations = Vec::<lopdf::content::Operation>::new();
        operations.append(&mut Box::new(self.fill.clone()).into_stream_op());
        operations.append(&mut Box::new(self.outline).into_stream_op());
        operations
    }
}
