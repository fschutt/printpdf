//! Abstraction class for images.
//! Please use this class instead of adding `ImageXObjects` yourself

#[cfg(feature = "embedded_images")]
use image::{self, ImageDecoder, DynamicImage};
use Mm;
use {ImageXObject, PdfLayerReference};

/// Image - wrapper around an `ImageXObject` to allow for more control
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
            image,
        }
    }

}

#[cfg(feature = "embedded_images")]
impl<'a> Image {
    pub fn try_from<T: ImageDecoder<'a>>(image: T)
    -> Result<Self, image::ImageError>
    {
        let image = ImageXObject::try_from(image)?;
        Ok(Self {
            image,
        })
    }

    pub fn from_dynamic_image(image: &DynamicImage)
    -> Self
    {
        Self {
            image: ImageXObject::from_dynamic_image(image),
        }
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
    #[cfg_attr(feature = "cargo-clippy", allow(too_many_arguments))]
    pub fn add_to_layer(self, layer: PdfLayerReference,
                        translate_x: Option<Mm>, translate_y: Option<Mm>,
                        rotate_cw: Option<f64>,
                        scale_x: Option<f64>, scale_y: Option<f64>,
                        dpi: Option<f64>)
    {
        // PDF maps an image to a 1x1 square, we have to adjust the transform matrix
        // to fix the distortion
        let dpi = dpi.unwrap_or(300.0);

        //Image at the given dpi should 1px = 1pt
        let image_w = self.image.width.into_pt(dpi);
        let image_h = self.image.height.into_pt(dpi);

        let image = layer.add_image(self.image);

        let scale_x = scale_x.unwrap_or(1.);
        let scale_y = scale_y.unwrap_or(1.);
        let image_w = Some(image_w.0 * scale_x);
        let image_h = Some(image_h.0 * scale_y);

        layer.use_xobject(image, translate_x, translate_y, rotate_cw, image_w, image_h);
    }
}
