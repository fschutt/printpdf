//! Outline of a shape (shared between 2D and 3D)

extern crate lopdf;

use *;

#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    pub color: Color,
}

impl Outline {
    
    /// Creates a new outline
    #[inline]
    pub fn new(color: Color)
    -> Self
    {
        Self {
            color,
        }
    }
}

// todo: use Into<lopdf::Operation>

impl IntoPdfStreamOperation for Outline {

    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>
    {
        use lopdf::content::Operation;
        use lopdf::Object::*;

        let mut operations = Vec::<Operation>::new();


        // todo: if the color space has been set, we want to insert a reference to the color space of course
        // let has_icc_profile = self.color.get_icc_profile();
        
        let color_identifier = match self.color {
            Color::Rgb(_) => { OP_COLOR_SET_STROKE_CS_DEVICERGB }
            Color::Cmyk(_) => { OP_COLOR_SET_STROKE_CS_DEVICECMYK }
            Color::Grayscale(_) => { OP_COLOR_SET_STROKE_CS_DEVICEGRAY }
            Color::SpotColor(_) => { OP_COLOR_SET_STROKE_CS_DEVICECMYK }
        };

        let color_vec = self.color.into_vec().into_iter().map(move |float| Real(float)).collect();
        operations.place_back() <- Operation::new(color_identifier, color_vec);
        operations
    }
}