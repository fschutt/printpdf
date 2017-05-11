//! Fill style, shared between 2D and 3D module

extern crate lopdf;

use *;

/// Fill color
#[derive(Debug, Clone, PartialEq)]
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
        use lopdf::Object::*;
        use lopdf::content::Operation;

        let mut operations = Vec::<lopdf::content::Operation>::new();
        let color_vec = self.color.into_vec().iter().map(move |float| Real(*float)).collect();
        operations.place_back() <- Operation::new(OP_COLOR_SET_FILL_COLOR, color_vec);
        operations
    }
}