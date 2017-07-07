//! Abstraction class for images. 
//! Please use this class instead of adding ImageXObjects yourself

use *;

/// Image - wrapper around an ImageXObject to allow for more control
/// within the library
pub struct Image {
    /// The actual image
    pub image: ImageXObject,
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
        let image = layer.add_image(ImageXObject { 
            bits_per_component: ColorBits::Bit1,
            clipping_bbox: None,
            color_space: ColorSpace::Greyscale,
            height: 8,
            image_filter: None,
            width: 8,
            interpolate: false,
            image_data: [0x40, 0x60, 0x70, 0x78, 0x78, 0x70, 0x60, 0x40].to_vec(),
        });

        layer.use_xobject(image, translate_x, translate_y, rotate_cw, scale_x, scale_y);
    }
}