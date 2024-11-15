use std::collections::HashMap;

use crate::units::Px;
use crate::xobject::ExternalXObject;
use svg2pdf::{usvg, ConversionOptions};

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
        use lopdf::Object;
        use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};

        // Parses the SVG, converts it to a PDF document using the svg2pdf crate,
        // parses the resulting PDF again
        // (using lopdf), then extracts the SVG XObject.
        //
        // I wish there was a more direct way, but handling SVG is very tricky.

        // Allocate the indirect reference IDs and names.
        let mut alloc = Ref::new(1);
        let catalog_id = alloc.bump();
        let page_tree_id = alloc.bump();
        let page_id = alloc.bump();
        let content_id = alloc.bump();
        let svg_name = Name(b"S1");

        // Start writing a PDF.
        let mut writer = Pdf::new();
        writer.catalog(catalog_id).pages(page_tree_id);
        writer.pages(page_tree_id).kids([page_id]).count(1);

        // Set up a simple A4 page.
        let mut page = writer.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, 595.0, 842.0));
        page.parent(page_tree_id);
        page.contents(content_id);

        // Let's first convert the SVG into an independent chunk.
        let mut options = usvg::Options::default();
        options.fontdb_mut().load_system_fonts();
        let tree = usvg::Tree::from_str(&svg_string, &options)
            .map_err(|err| format!("usvg parse: {err}"))?;
        let (mut svg_chunk, svg_id) = svg2pdf::to_chunk(&tree, ConversionOptions::default())
            .map_err(|err| format!("convert svg tree to chunk: {err}"))?;

        // Renumber the chunk so that we can embed it into our existing workflow, and also make sure
        // to update `svg_id`.
        let mut map = HashMap::new();
        svg_chunk = svg_chunk.renumber(|old| *map.entry(old).or_insert_with(|| alloc.bump()));
        let svg_id = map.get(&svg_id).unwrap();

        // Add the font and, more importantly, the SVG to the resource dictionary
        // so that it can be referenced in the content stream.
        let mut resources = page.resources();
        resources.x_objects().pair(svg_name, svg_id);
        resources.finish();
        page.finish();

        // Write a content stream
        let content = Content::new();
        writer.stream(content_id, &content.finish());
        // Write the SVG chunk into the PDF page.
        writer.extend(&svg_chunk);

        let bytes = writer.finish();
        let document = lopdf::Document::load_mem(&bytes)
            .map_err(|err| format!("lopdf load generated pdf: {err}"))?;
        let svg_xobject = document
            .get_object((5, 0))
            .map_err(|err| format!("grab xobject from generated pdf: {err}"))?;
        let object = svg_xobject.as_stream().unwrap();

        let bbox = object
            .dict
            .get(b"BBox")
            .map_err(|err| format!("extract xobject bbox: {err}"))?
            .as_array()
            .map_err(|err| format!("xobject bbox not an array: {err}"))?;

        let width_px = match bbox.get(2) {
            Some(Object::Integer(px)) => Ok(*px),
            Some(Object::Real(px)) => Ok(px.ceil() as i64),
            Some(obj) => Err(format!("xobject bbox width not a number: {obj:?}")),
            None => Err("xobject bbox missing width field".to_string()),
        }?;

        let height_px = match bbox.get(3) {
            Some(Object::Integer(px)) => Ok(*px),
            Some(Object::Real(px)) => Ok(px.ceil() as i64),
            Some(obj) => Err(format!("xobject bbox height not a number: {obj:?}")),
            None => Err("xobject bbox missing height field".to_string()),
        }?;

        Ok(ExternalXObject {
            stream: object.clone(),
            width: Some(Px(width_px.max(0) as usize)),
            height: Some(Px(height_px.max(0) as usize)),
        })
    }
}
