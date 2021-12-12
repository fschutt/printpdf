//! Abstraction class for images. Please use this class
//! instead of adding `ImageXObjects` yourself

#[cfg(feature = "embedded_images")]
use image_crate::{self, ImageDecoder, DynamicImage};
use crate::{Mm, Px, ImageXObject, PdfLayerReference};

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
    -> Result<Self, image_crate::ImageError>
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

/// Transform that is applied immediately before the
/// image gets painted. Does not affect anything other
/// than the image.
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct ImageTransform {
    pub translate_x: Option<Mm>,
    pub translate_y: Option<Mm>,
    /// Rotate (counter-clockwise) around a point, in degree angles
    pub rotate: Option<ImageRotation>,
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    /// If set to None, will be set to 300.0 for images
    pub dpi: Option<f64>,
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct ImageRotation {
    pub angle_ccw_degrees: f64,
    pub rotation_center_x: Px,
    pub rotation_center_y: Px,
}

impl Image {

    /// Adds the image to a specific layer and consumes it.
    ///
    /// This is due to a PDF weirdness - images are basically just "names"
    /// and you have to make sure that they are added to resources of the
    /// same page as they are used on.
    ///
    /// You can use the "transform.dpi" parameter to specify a scaling -
    /// the default is 300dpi
    pub fn add_to_layer(self, layer: PdfLayerReference, transform: ImageTransform)
    {
        use crate::CurTransMat;
        use Pt;

        // PDF maps an image to a 1x1 square, we have to adjust the transform matrix
        // to fix the distortion
        let dpi = transform.dpi.unwrap_or(300.0);

        //Image at the given dpi should 1px = 1pt
        let image_w = self.image.width.into_pt(dpi);
        let image_h = self.image.height.into_pt(dpi);

        let image = layer.add_image(self.image);

        let scale_x = transform.scale_x.unwrap_or(1.0);
        let scale_y = transform.scale_y.unwrap_or(1.0);
        let image_w = image_w.0 * scale_x;
        let image_h = image_h.0 * scale_y;


        let mut transforms = Vec::new();

        transforms.push(CurTransMat::Scale(image_w, image_h));

        if let Some(rotate) = transform.rotate.as_ref() {
            transforms.push(CurTransMat::Translate(
                Pt(-rotate.rotation_center_x.into_pt(dpi).0),
                Pt(-rotate.rotation_center_y.into_pt(dpi).0),
            ));
            transforms.push(CurTransMat::Rotate(
                rotate.angle_ccw_degrees,
            ));
            transforms.push(CurTransMat::Translate(
               rotate.rotation_center_x.into_pt(dpi),
               rotate.rotation_center_y.into_pt(dpi),
            ));
        }

        if transform.translate_x.is_some() ||
           transform.translate_y.is_some() {
            transforms.push(CurTransMat::Translate(
                transform.translate_x.unwrap_or(Mm(0.0)).into_pt(),
                transform.translate_y.unwrap_or(Mm(0.0)).into_pt(),
            ));
        }

        layer.use_xobject(image, &transforms);
    }
}
