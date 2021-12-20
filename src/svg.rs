//! Abstraction class for images. Please use this class
//! instead of adding `ImageXObjects` yourself

use crate::{Mm, Px, XObject, XObjectRef, PdfLayerReference};
use lopdf::Stream;
use std::{error::Error, fmt};

/// SVG - wrapper around an `XObject` to allow for more
/// control within the library
#[derive(Debug)]
pub struct Svg {
    /// The PDF document, converted from SVG using svg2pdf
    svg_xobject: Stream,
    /// Width of the rendered SVG content
    pub width: Px,
    /// Height of the rendered SVG content
    pub height: Px,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SvgParseError {
    Svg2PdfConversionError,
    PdfParsingError,
    NoContentStream,
    // PDF returned by pdf2svg is not in the expected form
    InternalError,
}

impl fmt::Display for SvgParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Svg2PdfConversionError => "svg2pdf conversion error",
                Self::PdfParsingError => "error parsing svg2pdf pdf data",
                Self::NoContentStream => "svg2pdf returned no content stream",
                Self::InternalError => "pdf returned by pdf2svg in unexpected form",
            }
        )
    }
}

impl Error for SvgParseError {}

/// Transform that is applied immediately before the
/// image gets painted. Does not affect anything other
/// than the image.
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct SvgTransform {
    pub translate_x: Option<Mm>,
    pub translate_y: Option<Mm>,
    /// Rotate (clockwise), in degree angles
    pub rotate: Option<SvgRotation>,
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    /// If set to None, will be set to 300.0 for images
    pub dpi: Option<f64>,
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct SvgRotation {
    pub angle_ccw_degrees: f64,
    pub rotation_center_x: Px,
    pub rotation_center_y: Px,
}

fn export_svg_to_xobject_pdf(svg: &str) -> Option<lopdf::Stream> {

    use pdf_writer::{Content, Finish, Name, PdfWriter, Rect, Ref, Str};
    use lopdf::Object;

    // Allocate the indirect reference IDs and names.
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let page_id = Ref::new(3);
    let content_id = Ref::new(4);
    let svg_id = Ref::new(5);
    let svg_name = Name(b"S1");

    // Start writing a PDF.
    let mut writer = PdfWriter::new();
    writer.catalog(catalog_id).pages(page_tree_id);
    writer.pages(page_tree_id).kids([page_id]).count(1);

    // Set up a simple A4 page.
    let mut page = writer.page(page_id);
    page.media_box(Rect::new(0.0, 0.0, 595.0, 842.0));
    page.parent(page_tree_id);
    page.contents(content_id);

    // Add the font and, more importantly, the SVG to the resource dictionary
    // so that it can be referenced in the content stream.
    let mut resources = page.resources();
    resources.x_objects().pair(svg_name, svg_id);
    resources.finish();
    page.finish();

    // Let's add an SVG graphic to this file.
    // We need to load its source first and manually parse it into a usvg Tree.
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default().to_ref()).ok()?;

    // Then, we will write it to the page as the 6th indirect object.
    //
    // This call allocates some indirect object reference IDs for itself. If we
    // wanted to write some more indirect objects afterwards, we could use the
    // return value as the next unused reference ID.
    svg2pdf::convert_tree_into(&tree, svg2pdf::Options::default(), &mut writer, svg_id);

    // Write a content stream
    let mut content = Content::new();
    writer.stream(content_id, &content.finish());

    let bytes = writer.finish();
    let document = lopdf::Document::load_mem(&bytes).ok()?;
    let svg_xobject = document.get_object((5, 0)).ok()?;
    let object = svg_xobject.as_stream().unwrap();

    Some(object.clone())
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct SvgXObjectRef {
    xobject_ref: XObjectRef,
    /// Width of the rendered SVG content
    pub width: Px,
    /// Height of the rendered SVG content
    pub height: Px,
}

impl SvgXObjectRef {
    pub fn add_to_layer(self, layer: &PdfLayerReference, transform: SvgTransform) {

        use crate::CurTransMat;
        use crate::scale::Pt;

        // PDF maps an image to a 1x1 square, we have to adjust the transform matrix
        // to fix the distortion

        let width = self.width.clone();
        let height = self.height.clone();
        let dpi = transform.dpi.unwrap_or(300.0);
        let scale_x = transform.scale_x.unwrap_or(1.0);
        let scale_y = transform.scale_y.unwrap_or(1.0);

        // Image at the given dpi should 1px = 1pt
        let image_w = width.into_pt(dpi).0 * scale_x;
        let image_h = height.into_pt(dpi).0 * scale_y;

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

        layer.use_xobject(self.xobject_ref, &transforms);
    }
}

impl Svg {

    /// Internally parses the SVG string, converts it to a PDF
    /// document using the svg2pdf crate, parses the resulting PDF again
    /// (using lopdf), then extracts the SVG XObject.
    ///
    /// I wish there was a more direct way, but handling SVG is very tricky.
    pub fn parse(svg_string: &str) -> Result<Self, SvgParseError> {

        // SVG -> PDF bytes
        let svg_xobject = export_svg_to_xobject_pdf(svg_string)
            .ok_or(SvgParseError::Svg2PdfConversionError)?;

        let bbox = svg_xobject.dict.get(b"BBox").ok()
        .ok_or(SvgParseError::Svg2PdfConversionError)?
        .as_array().ok()
        .ok_or(SvgParseError::Svg2PdfConversionError)?;

        let width_px = bbox.get(2)
        .ok_or(SvgParseError::Svg2PdfConversionError)?
        .as_i64().ok()
        .ok_or(SvgParseError::Svg2PdfConversionError)?;

        let height_px = bbox.get(2)
        .ok_or(SvgParseError::Svg2PdfConversionError)?
        .as_i64().ok()
        .ok_or(SvgParseError::Svg2PdfConversionError)?;

        Ok(Self {
            svg_xobject,
            width: Px(width_px.max(0) as usize),
            height: Px(height_px.max(0) as usize),
        })
    }

    /// Adds the SVG to the pages /Resources, returns the name of
    /// the reference to the SVG, so that one SVG can be used more
    /// than once on a page
    pub fn into_xobject(self, layer: &PdfLayerReference) -> SvgXObjectRef {

        let width = self.width.clone();
        let height = self.height.clone();
        let xobject_ref = layer.add_xobject(XObject::External(self.svg_xobject));

        SvgXObjectRef {
            xobject_ref,
            width,
            height,
        }
    }

    /// Adds the image to a specific layer and consumes it.
    ///
    /// This is due to a PDF weirdness - images are basically just "names"
    /// and you have to make sure that they are added to resources of the
    /// same page as they are used on.
    ///
    /// You can use the "transform.dpi" parameter to specify a scaling -
    /// the default is 300dpi
    pub fn add_to_layer(self, layer: &PdfLayerReference, transform: SvgTransform) {
        let xobject = self.into_xobject(layer);
        xobject.add_to_layer(layer, transform);
    }
}
