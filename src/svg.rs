use std::collections::BTreeMap;

use svg2pdf::{usvg, ConversionOptions, PageOptions};

use crate::{
    xobject::ExternalXObject, ColorSpace, DictItem, ExternalStream, PdfResources, PdfWarnMsg,
};

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
    pub fn parse(
        svg_string: &str,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Result<ExternalXObject, String> {
        // Parses the SVG, converts it to a PDF document using the svg2pdf crate,
        // parses the resulting PDF again, then extracts the first pages PDF content operations.

        // Let's first convert the SVG into an independent chunk.
        #[cfg(target_arch = "wasm32")]
        let options = usvg::Options::default();
        #[cfg(not(target_arch = "wasm32"))]
        let mut options = usvg::Options::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            options.fontdb_mut().load_system_fonts();
        }

        let dpi = 300.0;
        let tree = usvg::Tree::from_str(svg_string, &usvg::Options { dpi, ..options })
            .map_err(|err| format!("usvg parse: {err}"))?;

        let mut co = ConversionOptions::default();
        co.compress = false;
        co.embed_text = false; // TODO!

        let po = PageOptions { dpi };
        let pdf_bytes = svg2pdf::to_pdf(&tree, co, po)
            .map_err(|err| format!("convert svg tree to pdf: {err}"))?;

        let pdf = crate::deserialize::parse_pdf_from_bytes(
            &pdf_bytes,
            &crate::PdfParseOptions {
                fail_on_error: false,
            },
            warnings,
        )
        .map_err(|err| format!("convert svg tree to pdf: parse pdf: {err}"))?;

        let page = pdf
            .pages
            .get(0)
            .ok_or_else(|| format!("convert svg tree to pdf: no page rendered"))?;

        let stream = crate::serialize::translate_operations(
            &page.ops,
            &crate::serialize::prepare_fonts_for_serialization(&PdfResources::default(), &[], warnings),
            &BTreeMap::new(),
            true,
            warnings,
        );

        // Scale the PDF content down to a 1:1 unit square,
        // so that it behaves like an image
        let sx = 1.0 / page.media_box.width.0;
        let sy = 1.0 / page.media_box.height.0;

        let dict = [
            ("Type", DictItem::Name("XObject".into())),
            ("Subtype", DictItem::Name("Form".into())),
            (
                "ProcSet",
                DictItem::Array(vec![
                    DictItem::Name("PDF".into()),
                    DictItem::Name("Text".into()),
                    DictItem::Name("ImageC".into()),
                    DictItem::Name("ImageB".into()),
                ]),
            ),
            (
                "Resources",
                DictItem::Dict {
                    map: [(
                        "ColorSpace".to_string(),
                        DictItem::Name(ColorSpace::Rgb.as_string().into()),
                    )]
                    .into_iter()
                    .collect(),
                },
            ),
            (
                "BBox",
                DictItem::Array(vec![
                    DictItem::Real(0.0),
                    DictItem::Real(0.0),
                    DictItem::Real(page.media_box.width.0),
                    DictItem::Real(page.media_box.height.0),
                ]),
            ),
            (
                "Matrix",
                DictItem::Array(vec![
                    DictItem::Real(sx),
                    DictItem::Real(0.0),
                    DictItem::Real(0.0),
                    DictItem::Real(sy),
                    DictItem::Real(0.0),
                    DictItem::Real(0.0),
                ]),
            ),
        ];

        Ok(ExternalXObject {
            stream: ExternalStream {
                dict: dict.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
                content: stream,
                compress: false,
            },
            width: Some(page.media_box.width.into_px(dpi)),
            height: Some(page.media_box.height.into_px(dpi)),
            dpi: Some(dpi),
        })
    }
}
