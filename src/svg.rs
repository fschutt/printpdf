use std::collections::BTreeMap;

use lopdf::Object;
use svg2pdf::{ConversionOptions, PageOptions, usvg};

use crate::{ColorSpace, PdfResources, xobject::ExternalXObject};

/// SVG - wrapper around an `XObject` to allow for more
/// control within the library.
///
/// When placing multiple copies of the same SVG on the
/// same layer, it is better to use the `into_xobject`
/// method to get a reference, rather than a clone
#[derive(Debug, Clone)]
pub struct Svg {}

impl Svg {
    /// Parses the SVG string, converts it to a PDF XObject
    pub fn parse(svg_string: &str) -> Result<ExternalXObject, String> {
        // Parses the SVG, converts it to a PDF document using the svg2pdf crate,
        // parses the resulting PDF again, then extracts the first pages PDF content operations.

        // Let's first convert the SVG into an independent chunk.
        let mut options = usvg::Options::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            options.fontdb_mut().load_system_fonts();
        }

        let dpi = 300.0;
        let tree = usvg::Tree::from_str(svg_string, &options)
            .map_err(|err| format!("usvg parse: {err}"))?;

        let mut co = ConversionOptions::default();
        co.compress = false;
        co.embed_text = false; // TODO!

        let po = PageOptions { dpi };
        let pdf_bytes = svg2pdf::to_pdf(&tree, co, po)
            .map_err(|err| format!("convert svg tree to pdf: {err}"))?;

        let (pdf, _) =
            crate::deserialize::parse_pdf_from_bytes(&pdf_bytes, &crate::PdfParseOptions {
                fail_on_error: false,
            })
            .map_err(|err| format!("convert svg tree to pdf: parse pdf: {err}"))?;

        let page = pdf
            .pages
            .get(0)
            .ok_or_else(|| format!("convert svg tree to pdf: no page rendered"))?;

        let width_pt = page.media_box.width;
        let height_pt = page.media_box.height;
        let stream = crate::serialize::translate_operations(
            &page.ops,
            &crate::serialize::prepare_fonts(&PdfResources::default(), &[]),
            &BTreeMap::new(),
        );

        let px_width = width_pt.into_px(dpi);
        let px_height = height_pt.into_px(dpi);

        let dict = lopdf::Dictionary::from_iter(vec![
            ("Type", Object::Name("XObject".into())),
            ("Subtype", Object::Name("Form".into())),
            ("Width", Object::Integer(px_width.0 as i64)),
            (
                "ColorSpace",
                Object::Name(ColorSpace::Rgb.as_string().into()),
            ),
            (
                "BBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(px_width.0 as i64),
                    Object::Integer(px_height.0 as i64),
                ]),
            ),
        ]);

        Ok(ExternalXObject {
            stream: lopdf::Stream::new(dict, stream),
            width: Some(px_width),
            height: Some(px_height),
            dpi: Some(dpi),
        })
    }
}
