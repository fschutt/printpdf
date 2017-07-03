extern crate lopdf;
extern crate svg;

use traits::*;

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

/// SVG data 
#[derive(Debug, Clone)]
pub struct Svg {
    /// Raw data from the input
    svg_data: Vec<u8>,
    /// Width of this SVG file, is None if not found
    width: Option<SvgUnit>,
    /// Height of this SVG file, is None if not found
    height: Option<SvgUnit>,
}

impl Svg {

    pub fn new<R>(svg_data: R)
    -> Result<Self, ::std::io::Error> where R: ::std::io::Read
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
            svg_data: Vec::new(),
            width: initial_width,
            height: initial_height,
        })
    }
}

impl IntoPdfObject for Svg {
    fn into_obj(self: Box<Self>)
    -> Vec<lopdf::Object>
    {
        // make SVG to stream, then use it in the doument as a reference
        Vec::new()
    }
} 