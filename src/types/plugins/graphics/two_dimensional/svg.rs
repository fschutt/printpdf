extern crate lopdf;
extern crate svg;

use traits::*;
use *;

/// Unit for SVG elements. Uses uom crate for normalization
/// Since this library is designed to output PDFs, the only measurement
/// that PDF understands is point, so eventually everything is converted into point.
#[derive(Debug, Clone)]
pub enum SvgUnit {
    /// Multiple of the total height of the first seen font (usually 16px)
    /// Not yet supported, will be 16px
    Em(f64),
    /// Multiple of the x-height of the first seen font
    /// Not yet supported, will be interpreted as x-height of the Arial font
    Ex(f64),
    /// Pixel
    Px(f64),
    /// Inch
    In(f64),
    /// Centimeter
    Cm(f64),
    /// Millimeter
    Mm(f64),
    /// Point
    Pt(f64),
    /// Percent (todo: of what?)
    /// Not supported, will be interpreted as Point
    Pc(f64),
}

// Should convert to points. For now, only returns the inner number 
impl Into<f64> for SvgUnit {
    fn into(self)
    -> f64
    {
        use self::SvgUnit::*;

        // todo!!!!!

        match self {
            Em(em) => 16.0 * em,
            Ex(ex) => 16.0 * ex,
            Px(px) => 300.0 * px,
            In(inch) => 254.0 * inch,
            Cm(cm) => 10.0 * 254.0 * cm,
            Mm(mm) => 254.0 * mm,
            Pt(pt) => pt,
            Pc(pc) => pc,
        }
    }
}

/// SVG data 
#[derive(Debug, Clone)]
pub struct Svg {
    /// The actual line drawing, etc. operations, in order
    operations: Vec<lopdf::content::Operation>,
    /// Width of this SVG file, is None if not found
    width: Option<SvgUnit>,
    /// Height of this SVG file, is None if not found
    height: Option<SvgUnit>,
}

impl Svg {

    pub fn new<R>(svg_data: R)
    -> std::result::Result<Self, ::std::io::Error> where R: ::std::io::Read
    {
        // use svg::node::element::path::{Command, Data};
        // use svg::node::element::tag::Path;
        use svg::parser::Event;

        let mut initial_width = None;
        let mut initial_height = None;

        let parser = svg::read(svg_data)?;

        // get width and height
        for event in parser {
            match event {
                Event::Tag(_, _, attributes) => {
                    let mut mark_break = false;
                    if let Some(w) = attributes.get("width") {
                        if let Ok(parsed_w) = w.parse::<f64>() {
                            initial_width = Some(SvgUnit::Pt(parsed_w));
                            mark_break = true;
                        }
                    }

                    if let Some(h) = attributes.get("height") {
                        if let Ok(parsed_h) = h.parse::<f64>() {
                            initial_height = Some(SvgUnit::Pt(parsed_h));
                            mark_break = true;
                        }
                    }

                    if mark_break { break; }
                },
                _ => {},
            }
        }

        Ok( Svg {
            operations: Vec::new(),
            width: initial_width,
            height: initial_height,
        })
    }

    /// This is similar to the `image.add_to_layer` method, the only difference being
    /// that it calls a different function
    /// This should be seperated out into a macro
    pub fn add_to_layer(self, layer: PdfLayerReference,
                        translate_x: Option<f64>, translate_y: Option<f64>,
                        rotate_cw: Option<f64>,
                        scale_x: Option<f64>, scale_y: Option<f64>,
                        dpi: Option<f64>)
    -> std::result::Result<(), std::io::Error>
    {
        let svg_w: f64 = self.width.clone().unwrap_or(SvgUnit::Pt(10.0)).into();
        let svg_h: f64 = self.height.clone().unwrap_or(SvgUnit::Pt(10.0)).into();

        // add svg as XObject to page
        let svg_ref = layer.add_svg(self)?;

        // add reference of XObject to layer stream
        if let Some(scale_x) = scale_x {
            if let Some(scale_y) = scale_y {
                layer.use_xobject(svg_ref, translate_x, translate_y, rotate_cw, Some(scale_x * svg_w), Some(svg_h * scale_y));
            } else {
                layer.use_xobject(svg_ref, translate_x, translate_y, rotate_cw, Some(scale_x * svg_w), Some(svg_h));

            }
        } else {
            if let Some(scale_y) = scale_y {
                layer.use_xobject(svg_ref, translate_x, translate_y, rotate_cw, Some(svg_w), Some(svg_h * scale_y)); 
            } else {
                layer.use_xobject(svg_ref, translate_x, translate_y, rotate_cw, Some(svg_w), Some(svg_h)); 
            }
        }

        Ok(())
    }
}

impl std::convert::TryInto<FormXObject> for Svg {
    type Error = std::io::Error;

    fn try_into(self) 
    -> std::result::Result<FormXObject, std::io::Error>
    {
        let content = lopdf::content::Content{ operations: self.operations };
        let vec_u8 = content.encode()?;

        Ok(FormXObject {
            form_type: FormType::Type1,
            bytes: vec_u8,
            matrix: Some(CurTransMat::identity()),
            resources: None,
            group: None,
            ref_dict: None,
            metadata: None,
            piece_info: None,
            last_modified: None,
            struct_parent: None,
            struct_parents: None,
            opi: None,
            oc: None,
            name: None, /* todo */
        })
    }
}

/// SVG XObject, identified by its name
#[derive(Debug)]
pub struct SvgRef {
    pub(super) name: String
}

impl SvgRef {
    /// Creates a new SvgRef from an index
    pub fn new(index: usize)
    -> Self
    {
        Self {
            name: format!("SVG{}", index),
        }
    }
}