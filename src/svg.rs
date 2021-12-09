//! Abstraction class for images. Please use this class
//! instead of adding `ImageXObjects` yourself

use crate::{Mm, XObject, PdfLayerReference};
use lopdf::Stream;

/// SVG - wrapper around an `XObject` to allow for more
/// control within the library
#[derive(Debug)]
pub struct Svg {
    /// The PDF document, converted from SVG using svg2pdf
    svg_xobject: Stream,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SvgParseError {
    Svg2PdfConversionError,
    PdfParsingError,
    NoContentStream,
    // PDF returned by pdf2svg is not in the expected form
    InternalError,
}

/// Transform that is applied immediately before the
/// image gets painted. Does not affect anything other
/// than the image.
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct SvgTransform {
    pub translate_x: Option<Mm>,
    pub translate_y: Option<Mm>,
    /// Rotate (clockwise), in degree angles
    pub rotate_cw: Option<f64>,
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    /// If set to None, will be set to 300.0 for images
    pub dpi: Option<f64>,
}


impl Svg {

    /// Internally parses the SVG string, converts it to a PDF
    /// document using the svg2pdf crate, parses the resulting PDF again
    /// (using lopdf), then extracts the SVG XObject.
    ///
    /// I wish there was a more direct way, but handling SVG is very tricky.
    pub fn parse(svg_string: &str) -> Result<Self, SvgParseError> {

        use lopdf::Object;

        // SVG -> PDF bytes
        let pdf_bytes = svg2pdf::convert_str(svg_string, svg2pdf::Options::default()).ok()
        .ok_or(SvgParseError::Svg2PdfConversionError)?;

        // PDF bytes -> lopdf::Document
        let pdf_parsed = lopdf::Document::load_mem(&pdf_bytes).ok()
        .ok_or(SvgParseError::PdfParsingError)?;

        // Analyze the file and split out all resources
        let _ = std::fs::write("../test.pdf", &pdf_bytes);

        // now extract the main /Page stream
        let svg_xobject = pdf_parsed.objects.values().find_map(|s| match s {
            Object::Stream(s) => Some(s.clone()),
            _ => None,
        }).ok_or(SvgParseError::NoContentStream)?;

        // TODO: wrong, but whatever
        Ok(Self {
            svg_xobject,
        })
    }

    /// Adds the image to a specific layer and consumes it.
    ///
    /// This is due to a PDF weirdness - images are basically just "names"
    /// and you have to make sure that they are added to resources of the
    /// same page as they are used on.
    ///
    /// You can use the "transform.dpi" parameter to specify a scaling -
    /// the default is 300dpi
    pub fn add_to_layer(self, layer: PdfLayerReference, transform: SvgTransform)
    {
        use crate::{Px, CurTransMat};

        // PDF maps an image to a 1x1 square, we have to adjust the transform matrix
        // to fix the distortion
        let dpi = transform.dpi.unwrap_or(300.0);

        // Image at the given dpi should 1px = 1pt
        let image_w = Px(100).into_pt(dpi);
        let image_h = Px(100).into_pt(dpi);
        // let image_w = self.image.width.into_pt(dpi);
        // let image_h = self.image.height.into_pt(dpi);

        let image = layer.add_xobject(XObject::External(self.svg_xobject));

        let scale_x = transform.scale_x.unwrap_or(1.0);
        let scale_y = transform.scale_y.unwrap_or(1.0);
        let image_w = image_w.0 * scale_x;
        let image_h = image_h.0 * scale_y;

        layer.use_xobject(image, &[
            CurTransMat::Translate(transform.translate_x.unwrap_or(Mm(0.0)), transform.translate_y.unwrap_or(Mm(0.0))),
            CurTransMat::Rotate(transform.rotate_cw.unwrap_or(0.0)),
            CurTransMat::Scale(image_w, image_h)
        ]);
    }
}
