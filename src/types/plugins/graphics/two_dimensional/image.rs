//! Abstraction class for images. 
//! Please use this class instead of adding ImageXObjects yourself

extern crate image;

use image::jpeg::JPEGDecoder;
use std::io::Read;
use std::convert::TryFrom;
use *;

/// Image - wrapper around an ImageXObject to allow for more control
/// within the library
#[derive(Debug)]
pub struct Image {
    /// The actual image
    pub image: ImageXObject,
}

impl From<ImageXObject> for Image {
    fn from(image: ImageXObject) 
    -> Self
    {
        Self {
            image: image,
        }
    }

}

impl<R: Read> TryFrom<JPEGDecoder<R>> for Image {
    type Error = image::ImageError;
    fn try_from(image: JPEGDecoder<R>) 
    -> std::result::Result<Self, Self::Error>
    {
        let image = ImageXObject::try_from(image)?;
        Ok(Self {
            image: image,
        })
    }
}

impl Image {

    /// Adds the image to a specific layer and consumes it
    /// This is due to a PDF weirdness - images are basically just "names"
    /// and you have to make sure that they are added to the same page
    /// as they are used on.
    pub fn add_to_layer(self, layer: PdfLayerReference,
                        translate_x: Option<f64>, translate_y: Option<f64>,
                        rotate_cw: Option<f64>,
                        scale_x: Option<f64>, scale_y: Option<f64>)
    {
        let image = layer.add_image(self.image);
        layer.use_xobject(image, translate_x, translate_y, rotate_cw, scale_x, scale_y);
    }
}