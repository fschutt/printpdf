//! Abstraction class for images. 
//! Please use this class instead of adding ImageXObjects yourself

extern crate image;

use std::convert::TryFrom;
use image::ImageDecoder;
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

impl<T: ImageDecoder> TryFrom<T> for Image {
    type Error = image::ImageError;
    fn try_from(image: T) 
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
        // PDF maps an image to a 1x1 square, we have to adjust the transform matrix
        // to fix the di1080stortion
        let aspect_ratio_w = self.image.width as f64 / self.image.height as f64;

        // Adjust scaling
        const px_at_300_dpi: f64 = 2.54_f64 / 300.0_f64;

        println!("px_at_300_dpi: {:?}", px_at_300_dpi);
        println!("total scaling factor: {:?} x {:?}", px_at_300_dpi * self.image.width as f64, px_at_300_dpi * self.image.height as f64);

        let image = layer.add_image(self.image);

        if let Some(scale_x) = scale_x {
            layer.use_xobject(image, translate_x, translate_y, rotate_cw, Some(scale_x * aspect_ratio_w), scale_y);
        } else {
           layer.use_xobject(image, translate_x, translate_y, rotate_cw, Some(aspect_ratio_w), scale_y); 
        }
    }
}