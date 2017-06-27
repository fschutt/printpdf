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

        // same as outline
        // a bit weird, I expected OP_COLOR_SET_FILL_COLOR to work, ...

        // todo: incorporate ICC profile instead of just setting the default device cmyk color space
        let color_identifier = match self.color {
            Color::Rgb(_) => { OP_COLOR_SET_FILL_CS_DEVICERGB }
            Color::Cmyk(_) => { OP_COLOR_SET_FILL_CS_DEVICECMYK }
            Color::Grayscale(_) => { OP_COLOR_SET_FILL_CS_DEVICEGRAY }
            Color::SpotColor(_) => { OP_COLOR_SET_FILL_CS_DEVICECMYK }
        };

        let color_vec = self.color.into_vec().into_iter().map(move |float| Real(float)).collect();

        vec![Operation::new(color_identifier, color_vec)]
    }
}