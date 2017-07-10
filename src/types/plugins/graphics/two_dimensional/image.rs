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
    /// 
    /// You can use the "dpi" parameter to specify a scaling - the default is 300dpi
    /// 
    pub fn add_to_layer(self, layer: PdfLayerReference,
                        translate_x: Option<f64>, translate_y: Option<f64>,
                        rotate_cw: Option<f64>,
                        scale_x: Option<f64>, scale_y: Option<f64>,
                        dpi: Option<f64>)
    {
        // PDF maps an image to a 1x1 square, we have to adjust the transform matrix
        // to fix the distortion
        let dpi = dpi.unwrap_or(300.0);

        //Image at the given dpi should 1px = 1pt
        let image_w = self.image.width as f64 * (mm_to_pt!(self.image.width as f64  / dpi * 25.4) / self.image.width as f64);
        let image_h = self.image.height as f64 * (mm_to_pt!(self.image.height as f64 / dpi * 25.4) / self.image.height as f64);

        let image = layer.add_image(self.image);

        if let Some(scale_x) = scale_x {
            if let Some(scale_y) = scale_y {
                layer.use_xobject(image, translate_x, translate_y, rotate_cw, Some(scale_x * image_w), Some(image_h * scale_y));
            } else {
                layer.use_xobject(image, translate_x, translate_y, rotate_cw, Some(scale_x * image_w), Some(image_h));

            }
        } else {
            if let Some(scale_y) = scale_y {
                layer.use_xobject(image, translate_x, translate_y, rotate_cw, Some(image_w), Some(image_h * scale_y)); 
            } else {
                layer.use_xobject(image, translate_x, translate_y, rotate_cw, Some(image_w), Some(image_h)); 
            }
        }


    }
}