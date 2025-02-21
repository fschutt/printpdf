//! deserialize.rs
//!
//! This module implements parsing of a PDF (using lopdf) and converting it into a
//! printpdf::PdfDocument. In particular, it decompresses the content streams and then
//! converts lopdf operations to printpdf Ops.

use crate::{
    conformance::PdfConformance, Color, LineDashPattern, Op, PageAnnotMap, PdfDocument, PdfDocumentInfo, PdfMetadata, PdfPage, PdfResources
};
use lopdf::{
    Document as LopdfDocument, 
    Dictionary as LopdfDictionary, 
    Object, ObjectId
};
use time::{Date, OffsetDateTime, Time, UtcOffset};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PdfParseOptions {
    pub fail_on_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PdfWarnMsg {
    pub page: usize,
    pub op_id: usize,
    pub severity: &'static str,
    pub msg: String,
}

impl PdfWarnMsg {
    pub const ERROR: &'static str = "error";
    pub const WARNING: &'static str = "warning";

    pub fn error(page: usize, op_id: usize, e: String) -> Self {
        PdfWarnMsg { page, op_id, severity: Self::ERROR, msg: e }
    }
}

/// Parses a PDF file from bytes into a printpdf PdfDocument.
pub fn parse_pdf_from_bytes(bytes: &[u8], opts: &PdfParseOptions) -> Result<(PdfDocument, Vec<PdfWarnMsg>), String> {
    
    // Load the PDF document using lopdf.
    let doc = LopdfDocument::load_mem(bytes)
        .map_err(|e| format!("Failed to load PDF: {}", e))?;

    // Get the catalog from the trailer.
    let root_obj = doc.trailer.get(b"Root")
        .map_err(|_| "Missing Root in trailer".to_string())?;
    
    let root_ref = match root_obj {
        Object::Reference(r) => *r,
        _ => return Err("Invalid Root reference".to_string()),
    };

    let catalog = doc.get_object(root_ref)
        .map_err(|e| format!("Failed to get catalog: {}", e))?
        .as_dict()
        .map_err(|e| format!("Catalog is not a dictionary: {}", e))?;

    // Get the Pages tree from the catalog.
    let pages_obj = catalog.get(b"Pages")
        .map_err(|e| format!("Missing Pages key in catalog: {}", e))?;
    let pages_ref = match pages_obj {
        Object::Reference(r) => *r,
        _ => return Err("Pages key is not a reference".to_string()),
    };
    let pages_dict = doc.get_object(pages_ref)
        .map_err(|e| format!("Failed to get Pages object: {}", e))?
        .as_dict()
        .map_err(|e| format!("Pages object is not a dictionary: {}", e))?;

    // Recursively collect all page object references.
    let page_refs = collect_page_refs(pages_dict, &doc)?;

    // Parse each page.
    let mut pages = Vec::new();
    let mut warnings = Vec::new();
    for (i, page_ref) in page_refs.into_iter().enumerate() {
        let page_obj = doc.get_object(page_ref)
            .map_err(|e| format!("Failed to get page object: {}", e))?
            .as_dict()
            .map_err(|e| format!("Page object is not a dictionary: {}", e))?;
        let pdf_page = parse_page(i, page_obj, &doc, &mut warnings)?;
        pages.push(pdf_page);
    }

    // Parse document metadata (for this example we create a very simple metadata object).
    let metadata = parse_metadata(&doc.trailer);

    // For simplicity, we set global resources to default.
    let resources = PdfResources::default();

    // Build the final PdfDocument.
    let pdf_doc = PdfDocument {
        metadata: PdfMetadata { 
            info: metadata, 
            xmp: None,
        },
        resources,
        bookmarks: PageAnnotMap::default(),
        pages,
    };

    Ok((pdf_doc, warnings))
}

/// Recursively collects page object references from a Pages tree dictionary.
fn collect_page_refs(dict: &LopdfDictionary, doc: &LopdfDocument) -> Result<Vec<ObjectId>, String> {
    
    let mut pages = Vec::new();
    
    // The Pages tree must have a "Kids" array.
    let kids = dict.get(b"Kids")
        .map_err(|e| format!("Pages dictionary missing Kids key: {}", e))?;
    
    let page_refs = kids.as_array()
    .map(|s| s.iter().filter_map(|k| k.as_reference().ok())
    .collect::<Vec<_>>())
    .map_err(|_| "Pages.Kids is not an array".to_string())?;

    for r in page_refs {

        let kid_obj = doc.get_object(r)
        .map_err(|e| format!("Failed to get kid object: {}", e))?;

        if let Ok(kid_dict) = kid_obj.as_dict() {
            let kid_type = kid_dict.get(b"Type")
                .map_err(|e| format!("Kid missing Type: {}", e))?;
            match kid_type {
                Object::Name(ref t) if t == b"Page" => {
                    pages.push(r);
                },
                Object::Name(ref t) if t == b"Pages" => {
                    let mut child_pages = collect_page_refs(kid_dict, doc)?;
                    pages.append(&mut child_pages);
                },
                _ => return Err(format!("Unknown kid type: {:?}", kid_type)),
            }
        }
    }

    Ok(pages)
}

/// Parses a single page dictionary into a PdfPage.
fn parse_page(num: usize, page: &LopdfDictionary, doc: &LopdfDocument, warnings: &mut Vec<PdfWarnMsg>) -> Result<PdfPage, String> {
    
    // Parse MediaBox (required). PDF defines it as an array of 4 numbers.
    let media_box_obj = page.get(b"MediaBox")
        .map_err(|e| format!("Page missing MediaBox: {}", e))?;
    let media_box = parse_rect(media_box_obj)?;
    // TrimBox and CropBox are optional; use MediaBox as default.
    let trim_box = if let Ok(obj) = page.get(b"TrimBox") {
        parse_rect(obj)?
    } else {
        media_box.clone()
    };
    let crop_box = if let Ok(obj) = page.get(b"CropBox") {
        parse_rect(obj)?
    } else {
        media_box.clone()
    };

    // Get the Contents entry (could be a reference, an array, or a stream)
    let contents_obj = page.get(b"Contents")
        .map_err(|e| format!("Page missing Contents: {}", e))?;
    
    let mut content_data = Vec::new();
    match contents_obj {
        Object::Reference(r) => {
            let stream = doc.get_object(*r)
                .map_err(|e| format!("Failed to get content stream: {}", e))?
                .as_stream()
                .map_err(|e| format!("Content object is not a stream: {}", e))?;
            let data = stream.decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            content_data.extend(data);
        },
        Object::Array(arr) => {
            for obj in arr {
                if let Object::Reference(r) = obj {
                    let stream = doc.get_object(*r)
                        .map_err(|e| format!("Failed to get content stream: {}", e))?
                        .as_stream()
                        .map_err(|e| format!("Content object is not a stream: {}", e))?;
                    let data = stream.decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone());
                    content_data.extend(data);
                } else {
                    return Err("Content array element is not a reference".to_string());
                }
            }
        },
        _ => {
            // Try to interpret it as a stream.
            let stream = contents_obj.as_stream()
                .map_err(|e| format!("Contents not a stream: {}", e))?;
            let data = stream.decompressed_content()
                .map_err(|e| format!("Failed to decompress content: {}", e))?;
            content_data.extend(data);
        }
    }

    // Decode the content stream into a vector of lopdf operations.
    let content = lopdf::content::Content::decode(&content_data)
        .map_err(|e| format!("Failed to decode content stream: {}", e))?;
    let ops = content.operations;

    // Convert lopdf operations to printpdf Ops.
    let mut page_state = PageState::default();
    let mut printpdf_ops = Vec::new();
    for (op_id, op) in ops.iter().enumerate() {
        let parsed_op = parse_op(num, op_id, &op, &mut page_state, warnings)?;
        printpdf_ops.extend(parsed_op.into_iter());
    }

    Ok(PdfPage {
        media_box,
        trim_box,
        crop_box,
        ops: printpdf_ops,
    })
}

/// Converts a single lopdf Operation to a printpdf Op.
/// We use a mutable TextState to keep track of the current font and size.
#[derive(Debug, Clone, Default)]
pub struct PageState {
    /// True if we are inside a `BT ... ET` text block
    pub in_text_mode: bool,

    /// Current font resource and size (only relevant if `in_text_mode` = true)
    pub current_font: Option<crate::FontId>,
    pub current_font_size: Option<crate::units::Pt>,

    /// Current transformation matrix stack. Each entry is a 6-float array [a b c d e f].
    pub transform_stack: Vec<[f32; 6]>,

    /// Name of the current layer, if any (set by BDC with /OC).
    pub current_layer: Option<String>,

    // ------------------- PATH / SUBPATH STATE -------------------

    /// Accumulated subpaths. Each subpath is a list of `(Point, is_bezier_control_point)`.
    /// We store multiple subpaths so that if the path has `m`, `l`, `c`, `m` again, etc.,
    /// they become separate “rings” or subpaths. We only produce a final shape on stroke/fill.
    pub subpaths: Vec<Vec<(crate::graphics::Point, bool)>>,

    /// The subpath currently being constructed (i.e. after the last `m`).
    pub current_subpath: Vec<(crate::graphics::Point, bool)>,

    /// True if we have a "closepath" (like the `h` operator) for the current subpath.
    /// Some PDF operators forcibly close subpaths, e.g. `b` / `s` vs. `B` / `S`.
    pub current_subpath_closed: bool,
}

/// Convert a single lopdf Operation into zero, one, or many `printpdf::Op`.
/// We maintain / mutate `PageState` so that repeated path operators (`m`, `l`, `c`, etc.)
/// accumulate subpaths, and we only emit path-based Ops at stroke or fill time.
pub fn parse_op(
    page: usize,
    op_id: usize,
    op: &lopdf::content::Operation, 
    state: &mut PageState,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<Vec<Op>, String> {
    use crate::units::Pt;
    let mut out_ops = Vec::new();
    match op.operator.as_str() {
        // --- Graphics State Save/Restore ---
        "q" => {
            let top = state.transform_stack.last()
                .copied()
                .unwrap_or([1.0,0.0,0.0,1.0,0.0,0.0]);
            state.transform_stack.push(top);
            out_ops.push(Op::SaveGraphicsState);
        }
        "Q" => {
            if state.transform_stack.pop().is_none() {
                // we won't fail the parse, just warn
                warnings.push(PdfWarnMsg::error(page, op_id, format!("Warning: 'Q' with empty transform stack")));
            }
            out_ops.push(Op::RestoreGraphicsState);
        }

        // --- Text mode begin/end ---
        "BT" => {
            state.in_text_mode = true;
            state.current_font = None;
            state.current_font_size = None;
            out_ops.push(Op::StartTextSection);
        }
        "ET" => {
            state.in_text_mode = false;
            out_ops.push(Op::EndTextSection);
        }

        // --- Font + size (Tf) ---
        "Tf" => {
            if op.operands.len() == 2 {
                if let Some(font_name) = as_name(&op.operands[0]) {
                    state.current_font = Some(crate::FontId(font_name));
                }
                let size_val = to_f32(&op.operands[1]);
                state.current_font_size = Some(crate::units::Pt(size_val));

                // produce a corresponding printpdf op:
                if let (Some(fid), Some(sz)) = (&state.current_font, &state.current_font_size) {
                    out_ops.push(Op::SetFontSize {
                        size: *sz,
                        font: fid.clone(),
                    });
                }
            } else {
                warnings.push(PdfWarnMsg::error(page, op_id, format!("Warning: 'Tf' expects 2 operands, got {}", op.operands.len())));
                out_ops.push(Op::Unknown {
                    key: "Tf".into(),
                    value: op.operands.clone()
                });
            }
        }

        // --- Show text (Tj) single string example ---
        "Tj" => {
            if !state.in_text_mode {
                warnings.push(PdfWarnMsg::error(page, op_id, format!("Warning: 'Tj' outside of text mode!")));
            }
            if op.operands.is_empty() {
                warnings.push(PdfWarnMsg::error(page, op_id, format!("Warning: 'Tj' with no operands")));
            } else if let lopdf::Object::String(bytes, _) = &op.operands[0] {
                let text_str = String::from_utf8_lossy(bytes).to_string();
                if let (Some(fid), Some(sz)) = (&state.current_font, state.current_font_size) {
                    out_ops.push(Op::WriteText {
                        text: text_str,
                        font: fid.clone(),
                        size: sz
                    });
                }
            } else {
                warnings.push(PdfWarnMsg::error(page, op_id, format!("Warning: 'Tj' operand is not string")));
            }
        }

        // --- Move text cursor (Td) ---
        "Td" => {
            if op.operands.len() == 2 {
                let tx = to_f32(&op.operands[0]);
                let ty = to_f32(&op.operands[1]);
                out_ops.push(Op::SetTextCursor {
                    pos: crate::graphics::Point {
                        x: crate::units::Pt(tx),
                        y: crate::units::Pt(ty),
                    },
                });
            }
        }

        // --- Begin/End layer (BDC/EMC) ---
        "BDC" => {
            // Typically something like: [Name("OC"), Name("MyLayer")] 
            if op.operands.len() == 2 {
                if let Some(layer_nm) = as_name(&op.operands[1]) {
                    state.current_layer = Some(layer_nm.clone());
                    out_ops.push(Op::BeginLayer {
                        layer_id: crate::LayerInternalId(layer_nm),
                    });
                }
            }
        }
        "EMC" => {
            if let Some(layer_str) = state.current_layer.take() {
                out_ops.push(Op::EndLayer {
                    layer_id: crate::LayerInternalId(layer_str),
                });
            } else {
                warnings.push(PdfWarnMsg::error(page, op_id, format!("Warning: 'EMC' with no current_layer")));
            }
        }

        // --- Transformation (cm) ---
        "cm" => {
            if op.operands.len() == 6 {
                let floats: Vec<f32> = op.operands.iter().map(to_f32).collect();
                if let Some(top) = state.transform_stack.last_mut() {
                    // multiply top by these floats
                    let combined = crate::matrix::CurTransMat::combine_matrix(*top, floats.as_slice().try_into().unwrap());
                    *top = combined;
                }
                out_ops.push(Op::SetTransformationMatrix {
                    matrix: crate::matrix::CurTransMat::Raw(floats.try_into().unwrap()),
                });
            } else {
                warnings.push(PdfWarnMsg::error(page, op_id, format!("Warning: 'cm' expects 6 floats")));
            }
        }

        // --- Path building: moveTo (m), lineTo (l), closepath (h), curveTo (c), etc. ---

        "m" => {
            // Start a new subpath
            if !state.current_subpath.is_empty() {
                // push the old subpath into subpaths
                state.subpaths.push(std::mem::take(&mut state.current_subpath));
            }
            let x = to_f32(&op.operands.get(0).unwrap_or(&lopdf::Object::Null));
            let y = to_f32(&op.operands.get(1).unwrap_or(&lopdf::Object::Null));
            state.current_subpath.push((
                crate::graphics::Point {
                    x: crate::units::Pt(x),
                    y: crate::units::Pt(y),
                },
                false
            ));
            state.current_subpath_closed = false;
        }
        "l" => {
            // lineTo
            let x = to_f32(&op.operands.get(0).unwrap_or(&lopdf::Object::Null));
            let y = to_f32(&op.operands.get(1).unwrap_or(&lopdf::Object::Null));
            state.current_subpath.push((
                crate::graphics::Point {
                    x: crate::units::Pt(x),
                    y: crate::units::Pt(y),
                },
                false
            ));
        }
        "c" => {
            // c x1 y1 x2 y2 x3 y3
            // We should already have a current_subpath with at least 1 point.
            if op.operands.len() == 6 {
                let x1 = to_f32(&op.operands[0]);
                let y1 = to_f32(&op.operands[1]);
                let x2 = to_f32(&op.operands[2]);
                let y2 = to_f32(&op.operands[3]);
                let x3 = to_f32(&op.operands[4]);
                let y3 = to_f32(&op.operands[5]);

                // Append these points to your “current_subpath” in a way that
                // your final geometry code knows it’s a Bézier curve. That might
                // mean storing them in some specialized “CurveTo” variant or
                // tagging them as control points.

                // For example:
                state.current_subpath.push((
                    crate::graphics::Point { x: Pt(x1), y: Pt(y1) },
                    true  // could mark as "control point"
                ));
                state.current_subpath.push((
                    crate::graphics::Point { x: Pt(x2), y: Pt(y2) },
                    true
                ));
                state.current_subpath.push((
                    crate::graphics::Point { x: Pt(x3), y: Pt(y3) },
                    false // endpoint
                ));
            } else {
                // handle error / warning
            }
        }
        "v" => {
            // v x2 y2 x3 y3
            // The first control point is implied to be the current point.
            // So in standard PDF usage:
            //   c (x0,y0) [current point], (x1,y1) [= current point], (x2,y2), (x3,y3)
            if op.operands.len() == 4 {
                // The "x1,y1" is the current subpath's last point,
                // so we treat that as control-pt #1 implicitly.
                let x2 = to_f32(&op.operands[0]);
                let y2 = to_f32(&op.operands[1]);
                let x3 = to_f32(&op.operands[2]);
                let y3 = to_f32(&op.operands[3]);
    
                // The first control point is the same as the last subpath point:
                // but we still need to mark the next one as a control point:
                state.current_subpath.push((
                    crate::graphics::Point { x: Pt(x2), y: Pt(y2) },
                    true // second control
                ));
                // And the final endpoint:
                state.current_subpath.push((
                    crate::graphics::Point { x: Pt(x3), y: Pt(y3) },
                    false
                ));
            } else {
                // handle error
            }
        }
        "y" => {
            // Cubic Bezier: "y" => first control point + final endpoint
            // y x1 y1 x3 y3
            // The second control point is implied to be x3,y3.
            if op.operands.len() == 4 {
                let x1 = to_f32(&op.operands[0]);
                let y1 = to_f32(&op.operands[1]);
                let x3 = to_f32(&op.operands[2]);
                let y3 = to_f32(&op.operands[3]);
    
                // The second control point is the same as final endpoint,
                // so we store the first control point explicitly:
                state.current_subpath.push((
                    crate::graphics::Point { x: Pt(x1), y: Pt(y1) },
                    true  // first control
                ));
                // Then the final endpoint (which is also the second control)
                state.current_subpath.push((
                    crate::graphics::Point { x: Pt(x3), y: Pt(y3) },
                    false
                ));
            } else {
                // handle error
            }
        }    
        "h" => {
            // closepath, i.e. connect last point to first point
            // We'll just mark a flag that we want to close it in fill/stroke
            state.current_subpath_closed = true;
        }

        // --- Stroke (S), Fill (f), Fill+Stroke (B), etc. ---
        // We'll unify them to produce one final polygon in `out_ops`.
        "S" => {
            if let Some(op) = finalize_current_path(state, crate::graphics::PaintMode::Stroke) {
                out_ops.push(op);
            }
        }
        "f" => {
            if let Some(op) = finalize_current_path(state, crate::graphics::PaintMode::Fill) {
                out_ops.push(op);
            }
        }
        "B" => {
            // fill+stroke
            if let Some(op) = finalize_current_path(state, crate::graphics::PaintMode::FillStroke) {
                out_ops.push(op);
            }
        }
        "f*" => {
            // Fill with the even-odd winding rule, no subpath closing
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::Fill,
                crate::graphics::WindingOrder::EvenOdd,
            ) {
                out_ops.push(op);
            }
        }
        "b" => {
            // Fill + stroke + close the subpath
            state.current_subpath_closed = true;
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::FillStroke,
                crate::graphics::WindingOrder::NonZero,
            ) {
                out_ops.push(op);
            }
        }
        "b*" => {
            // Fill + stroke using even-odd, plus close subpath
            state.current_subpath_closed = true;
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::FillStroke,
                crate::graphics::WindingOrder::EvenOdd,
            ) {
                out_ops.push(op);
            }
        }
        "B*" => {
            // Fill + stroke with even-odd, but subpath is not forcibly closed
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::FillStroke,
                crate::graphics::WindingOrder::EvenOdd,
            ) {
                out_ops.push(op);
            }
        }
        "s" => {
            // Stroke path and close it
            state.current_subpath_closed = true;
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::Stroke,
                crate::graphics::WindingOrder::NonZero,
            ) {
                out_ops.push(op);
            }
        }

        // --- Painting state operators 

        "w" => {
            // Set line width
            // "w" sets the line width (stroke thickness in user‐space units)
            // e.g. "3 w" => 3pt line width
            if let Some(val) = op.operands.get(0) {
                let width = to_f32(val);
                out_ops.push(Op::SetOutlineThickness {
                    pt: crate::units::Pt(width),
                });
            }
        }

        "M" => {
            // Set miter limit
            // PDF operator "M <limit>" sets the miter limit.
            // You don't currently have a specific Op variant for that,
            // so you can either create one or store as `Unknown`.
            if let Some(val) = op.operands.get(0) {
                let limit = to_f32(val);
                // If you create an Op::SetMiterLimit, do that here:
                out_ops.push(Op::SetMiterLimit { limit: Pt(limit) });
            }
        }

        "j" => {
                    // Set line join style
            // "0 j" => miter join, "1 j" => round join, "2 j" => bevel
            if let Some(val) = op.operands.get(0) {
                let style_num = to_f32(val).round() as i64;
                let style = match style_num {
                    0 => crate::graphics::LineJoinStyle::Miter,
                    1 => crate::graphics::LineJoinStyle::Round,
                    2 => crate::graphics::LineJoinStyle::Bevel,
                    _ => crate::graphics::LineJoinStyle::Miter, // fallback
                };
                out_ops.push(Op::SetLineJoinStyle { join: style });
            }
        }

        "J" => {
            // Set line cap style
            // "0 J" => butt cap, "1 J" => round cap, "2 J" => projecting square
            if let Some(val) = op.operands.get(0) {
                let style_num = to_f32(val).round() as i64;
                let style = match style_num {
                    0 => crate::graphics::LineCapStyle::Butt,
                    1 => crate::graphics::LineCapStyle::Round,
                    2 => crate::graphics::LineCapStyle::ProjectingSquare,
                    _ => crate::graphics::LineCapStyle::Butt, // fallback
                };
                out_ops.push(Op::SetLineCapStyle { cap: style });
            }
        }

        "d" => {
            // Set dash pattern
            // "d [2 2] 0" => dash 2 on, 2 off, offset=0
            if op.operands.len() == 2 {
                // operand 0 is the dash array, operand 1 is the dash offset
                if let Some(arr_obj) = op.operands.get(0) {
                    // parse array of numbers
                    if let Ok(arr) = arr_obj.as_array() {
                        let pattern: Vec<i64> = arr.iter().map(|item| to_f32(item) as i64).collect();
                        let offset = to_f32(&op.operands[1]) as i64;
                        let dash = LineDashPattern::from_array(&pattern, offset);
                        out_ops.push(Op::SetLineDashPattern { dash });
                    }
                }
            }
        }

        // Fill color: "sc" or "scn"
        // Typically you see "1 1 1 sc" => white fill in DeviceRGB
        "sc" | "scn" => {
            // We interpret the number of operands to guess the color space.
            // e.g. 1 operand => grayscale, 3 => RGB, 4 => CMYK
            let floats = op.operands.iter().map(to_f32).collect::<Vec<_>>();
            match floats.len() {
                1 => {
                    // grayscale
                    out_ops.push(Op::SetFillColor {
                        col: Color::Greyscale(crate::Greyscale { percent: floats[0], icc_profile: None }),
                    });
                },
                3 => {
                    // rgb
                    out_ops.push(Op::SetFillColor {
                        col: Color::Rgb(crate::Rgb { r: floats[0], g: floats[1], b: floats[2], icc_profile: None }),
                    });
                },
                4 => {
                    // cmyk
                    out_ops.push(Op::SetFillColor {
                        col: Color::Cmyk(crate::Cmyk { c: floats[0], m: floats[1], y: floats[2], k: floats[3], icc_profile: None }),
                    });
                },
                _ => {
                    // fallback
                    out_ops.push(Op::Unknown {
                        key: op.operator.clone(),
                        value: op.operands.clone(),
                    });
                }
            }
        }

        // Stroke color: "SC" or "SCN"
        // e.g. "1 0 0 SC" => red stroke
        "SC" | "SCN" => {
            let floats = op.operands.iter().map(to_f32).collect::<Vec<_>>();
            match floats.len() {
                1 => {
                    out_ops.push(Op::SetOutlineColor {
                        col: Color::Greyscale(crate::Greyscale { percent: floats[0], icc_profile: None }),
                    });
                },
                3 => {
                    out_ops.push(Op::SetOutlineColor {
                        col: Color::Rgb(crate::Rgb { r: floats[0], g: floats[1], b: floats[2], icc_profile: None }),
                    });
                },
                4 => {
                    out_ops.push(Op::SetOutlineColor {
                        col: Color::Cmyk(crate::Cmyk { c: floats[0], m: floats[1], y: floats[2], k: floats[3], icc_profile: None }),
                    });
                },
                _ => {
                    out_ops.push(Op::Unknown {
                        key: op.operator.clone(),
                        value: op.operands.clone(),
                    });
                }
            }
        }

        // For completeness, you might also parse "cs", "CS" to track the chosen color space
        // or treat them as Unknown if you don't need them:
        "cs" | "CS" => {
            // sets the fill or stroke color space. Usually you'd store in state, or ignore:
            out_ops.push(Op::Unknown {
                key: op.operator.clone(),
                value: op.operands.clone(),
            });
        }

        // --- XObjects: /Do ---
        "Do" => {
            if let Some(name_str) = as_name(&op.operands.get(0).unwrap_or(&lopdf::Object::Null)) {
                let xobj_id = crate::XObjectId(name_str);
                // For simplicity, we ignore any transform that was previously set via `cm`.
                out_ops.push(Op::UseXObject {
                    id: xobj_id,
                    transform: crate::xobject::XObjectTransform::default(),
                });
            }
        }

        // Catch everything else
        other => {
            warnings.push(PdfWarnMsg::error(page, op_id, format!("Info: unhandled operator '{}'", other)));
        }
    }

    Ok(out_ops)
}


// Helper that finalizes subpaths and sets the specified winding order
fn finalize_current_path_special(
    state: &mut PageState,
    paint_mode: crate::graphics::PaintMode,
    winding: crate::graphics::WindingOrder,
) -> Option<Op>
{
    // If there's a partially built subpath, move it into `subpaths`
    if !state.current_subpath.is_empty() {
        let sub = std::mem::take(&mut state.current_subpath);
        state.subpaths.push(sub);
    }
    if state.subpaths.is_empty() {
        state.current_subpath_closed = false;
        return None;
    }

    let rings = std::mem::take(&mut state.subpaths);
    let polygon = crate::graphics::Polygon {
        rings,
        mode: paint_mode,
        winding_order: winding,
    };
    // reset
    state.current_subpath_closed = false;

    Some(Op::DrawPolygon { polygon })
}

// A small helper to produce a final shape if subpaths exist, e.g. on stroke or fill
fn finalize_current_path(
    state: &mut PageState,
    paint_mode: crate::graphics::PaintMode
) -> Option<Op> {
    if state.subpaths.is_empty() && state.current_subpath.is_empty() {
        return None;
    }
    // If there's a current_subpath not yet appended, push it in
    if !state.current_subpath.is_empty() {
        let sub = std::mem::take(&mut state.current_subpath);
        state.subpaths.push(sub);
    }
    let rings = std::mem::take(&mut state.subpaths);

    let polygon = crate::graphics::Polygon {
        rings,
        mode: paint_mode,
        // For simplicity, we do not handle even-odd fill vs nonzero, etc.
        winding_order: crate::graphics::WindingOrder::NonZero,
    };
    state.current_subpath_closed = false;

    Some(Op::DrawPolygon { polygon })
}

/// Parses a PDF rectangle from an Object (an array of four numbers).
fn parse_rect(obj: &Object) -> Result<crate::graphics::Rect, String> {
    if let Object::Array(arr) = obj {
        if arr.len() != 4 {
            return Err("Rectangle array does not have 4 elements".to_string());
        }
        let nums: Result<Vec<f32>, String> = arr.iter().map(|o| match o {
            Object::Integer(i) => Ok(*i as f32),
            Object::Real(r) => Ok(*r),
            _ => Err("Rectangle element is not a number".to_string()),
        }).collect();
        let nums = nums?;
        // In PDF the rectangle is given as [llx, lly, urx, ury].
        let x = nums[0];
        let y = nums[1];
        let urx = nums[2];
        let ury = nums[3];
        let width = urx - x;
        let height = ury - y;
        Ok(crate::graphics::Rect {
            x: crate::units::Pt(x),
            y: crate::units::Pt(y),
            width: crate::units::Pt(width),
            height: crate::units::Pt(height),
        })
    } else {
        Err("Rectangle is not an array".to_string())
    }
}

/// This function looks for an Info dictionary in the trailer and, if found,
/// extracts common metadata fields (Title, Author, Creator, Producer, Subject,
/// Identifier, CreationDate, ModDate, and Trapped).
fn parse_metadata(trailer: &LopdfDictionary) -> PdfDocumentInfo {
    
    let mut doc_info = PdfDocumentInfo {
        document_title: "".to_string(),
        author: "".to_string(),
        creator: "".to_string(),
        producer: "".to_string(),
        subject: "".to_string(),
        keywords: Vec::new(),
        trapped: false,
        version: 1,
        creation_date: OffsetDateTime::UNIX_EPOCH,
        modification_date: OffsetDateTime::UNIX_EPOCH,
        metadata_date: OffsetDateTime::UNIX_EPOCH,
        conformance: PdfConformance::default(),
        identifier: "".to_string(),
    };

    let info_dict = match trailer.get(b"Info").ok().and_then(|o| o.as_dict().ok()) {
        Some(s) => s,
        None => return doc_info,
    };

    if let Ok(Object::String(title_bytes, _)) = info_dict.get(b"Title") {
        doc_info.document_title = String::from_utf8(title_bytes.clone()).unwrap_or_default();
    }
    if let Ok(Object::String(author_bytes, _)) = info_dict.get(b"Author") {
        doc_info.author = String::from_utf8(author_bytes.clone()).unwrap_or_default();
    }
    if let Ok(Object::String(creator_bytes, _)) = info_dict.get(b"Creator") {
        doc_info.creator = String::from_utf8(creator_bytes.clone()).unwrap_or_default();
    }
    if let Ok(Object::String(producer_bytes, _)) = info_dict.get(b"Producer") {
        doc_info.producer = String::from_utf8(producer_bytes.clone()).unwrap_or_default();
    }
    if let Ok(Object::String(subject_bytes, _)) = info_dict.get(b"Subject") {
        doc_info.subject = String::from_utf8(subject_bytes.clone()).unwrap_or_default();
    }
    if let Ok(Object::String(identifier_bytes, _)) = info_dict.get(b"Identifier") {
        doc_info.identifier = String::from_utf8(identifier_bytes.clone()).unwrap_or_default();
    }
    if let Ok(Object::String(date_bytes, _)) = info_dict.get(b"CreationDate") {
        if let Ok(date_str) = String::from_utf8(date_bytes.clone()) {
            if let Ok(dt) = parse_pdf_date(&date_str) {
                doc_info.creation_date = dt;
            }
        }
    }
    if let Ok(Object::String(date_bytes, _)) = info_dict.get(b"ModDate") {
        if let Ok(date_str) = String::from_utf8(date_bytes.clone()) {
            if let Ok(dt) = parse_pdf_date(&date_str) {
                doc_info.modification_date = dt;
            }
        }
    }
    
    // Use the modification date as metadata date.
    doc_info.metadata_date = doc_info.modification_date;
    
    if let Ok(Object::Name(trapped_bytes)) = info_dict.get(b"Trapped") {
        let trapped_str = String::from_utf8(trapped_bytes.clone()).unwrap_or_default();
        doc_info.trapped = trapped_str == "True";
    }

    doc_info
}

/// A simple parser for PDF date strings (e.g. "D:20170505150224+02'00'")
fn parse_pdf_date(s: &str) -> Result<OffsetDateTime, String> {
    // Remove a leading "D:" if present.
    let s = if s.starts_with("D:") { &s[2..] } else { s };
    if s.len() < 14 {
        return Err("Date string too short".to_string());
    }
    let year: i32 = s[0..4].parse::<i32>().map_err(|e| e.to_string())?;
    let month: u8 = s[4..6].parse::<u8>().map_err(|e| e.to_string())?;
    let day: u8 = s[6..8].parse::<u8>().map_err(|e| e.to_string())?;
    let hour: u8 = s[8..10].parse::<u8>().map_err(|e| e.to_string())?;
    let minute: u8 = s[10..12].parse::<u8>().map_err(|e| e.to_string())?;
    let second: u8 = s[12..14].parse::<u8>().map_err(|e| e.to_string())?;
    let month = match month {
        1 => time::Month::January,
        2 => time::Month::February,
        3 => time::Month::March,
        4 => time::Month::April,
        5 => time::Month::May,
        6 => time::Month::June,
        7 => time::Month::July,
        8 => time::Month::August,
        9 => time::Month::September,
        10 => time::Month::October,
        11 => time::Month::November,
        12 => time::Month::December,
        _ => time::Month::January,
    };

    Ok(OffsetDateTime::new_in_offset(
        Date::from_calendar_date(year, month, day).map_err(|e| e.to_string())?,
        Time::from_hms(hour, minute, second).map_err(|e| e.to_string())?,
        UtcOffset::from_hms(0, 0, 0).map_err(|e| e.to_string())?,
    ))
}

/// Helper to parse an operand into f32
fn to_f32(obj: &Object) -> f32 {
    match obj {
        Object::Integer(i) => *i as f32,
        Object::Real(r) => *r,
        _ => 0.0,
    }
}

/// Helper to parse an operand as a PDF name
fn as_name(obj: &Object) -> Option<String> {
    if let Object::Name(ref bytes) = obj {
        Some(String::from_utf8_lossy(bytes).to_string())
    } else {
        None
    }
}
