//! Writing the patterns of a resource scope.
//!
//! A tiling pattern is a stream (`/PatternType 1`) with the content of one cell and resources of
//! its own, written by the same code as the resources of a form. A shading pattern is a
//! dictionary (`/PatternType 2`) with the shading inline. A pattern that has no model
//! ([`Pattern::External`]) is written as it was read.
//!
//! The ops [`Op::SetFillPattern`] and [`Op::SetOutlinePattern`] select a pattern with
//! `/Pattern cs /Name scn`. An uncolored tiling pattern needs a color space that says which color
//! space its color is in, `[/Pattern /DeviceRGB]`. That array is a resource, so a scope that has
//! such an op gets a `/ColorSpace` dictionary with the entries of [`pattern_color_space_dict`].

use std::collections::BTreeMap;

use lopdf::{
    Dictionary as LoDictionary,
    Object::{self, Array, Dictionary, Integer, Name, Real},
};

use super::{
    form_xobject::{bbox_array, write_local_scope, SaveScope},
    translate_operations, PdfSaveOptions,
};
use crate::{
    form_walk::MAX_FORM_DEPTH, pattern::PatternPaintType, Color, Op, Pattern, PatternId,
    PdfResources, PdfWarnMsg, Rect,
};

/// Name of the `[/Pattern /DeviceGray]` color space resource.
const SPACE_GRAY: &str = "PdfPatternGray";
/// Name of the `[/Pattern /DeviceRGB]` color space resource.
const SPACE_RGB: &str = "PdfPatternRgb";
/// Name of the `[/Pattern /DeviceCMYK]` color space resource.
const SPACE_CMYK: &str = "PdfPatternCmyk";

/// The color space resource that selects a pattern with `color` as its underlying color:
/// the name to use with `cs`, and the base color space of the array.
pub(super) fn pattern_space(color: &Color) -> (&'static str, &'static str) {
    match color {
        Color::Rgb(_) => (SPACE_RGB, "DeviceRGB"),
        Color::Cmyk(_) | Color::SpotColor(_) => (SPACE_CMYK, "DeviceCMYK"),
        Color::Greyscale(_) => (SPACE_GRAY, "DeviceGray"),
    }
}

/// The `/ColorSpace` entries that the pattern ops of `streams` need: one `[/Pattern /Base]`
/// array for each color space of an underlying color. Empty when no op has such a color.
pub(super) fn pattern_color_space_dict(streams: &[&[Op]]) -> LoDictionary {
    let mut dict = LoDictionary::new();
    for ops in streams {
        for op in ops.iter() {
            if let Op::SetFillPattern { color: Some(color), .. }
            | Op::SetOutlinePattern { color: Some(color), .. } = op
            {
                let (name, base) = pattern_space(color);
                if !dict.has(name.as_bytes()) {
                    dict.set(
                        name,
                        Array(vec![Name(b"Pattern".to_vec()), Name(base.as_bytes().to_vec())]),
                    );
                }
            }
        }
    }
    dict
}

/// Writes every pattern of a scope as an object of `doc` and returns the `/Pattern` dictionary.
/// `scope` is the scope the patterns are in: a tiling pattern is nested one level deeper.
pub(crate) fn write_patterns(
    doc: &mut lopdf::Document,
    patterns: &BTreeMap<PatternId, Pattern>,
    scope: &SaveScope<'_>,
    opts: &PdfSaveOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> LoDictionary {
    let mut dict = LoDictionary::new();
    for (id, pattern) in patterns.iter() {
        let object = match pattern {
            Pattern::Tiling {
                paint_type,
                bbox,
                xstep,
                ystep,
                matrix,
                ops,
                resources,
            } => {
                if scope.depth >= MAX_FORM_DEPTH {
                    warnings.push(PdfWarnMsg::warning(
                        0,
                        0,
                        format!(
                            "pattern {} is nested deeper than {MAX_FORM_DEPTH} forms and \
                             patterns and is not written",
                            id.0
                        ),
                    ));
                    continue;
                }
                let tiling = Tiling {
                    id,
                    paint_type: *paint_type,
                    bbox,
                    xstep: *xstep,
                    ystep: *ystep,
                    matrix,
                    ops,
                    resources: resources.as_ref(),
                };
                let stream = write_tiling(&tiling, doc, scope, opts, warnings);
                doc.add_object(stream)
            }
            Pattern::Shading {
                shading,
                matrix,
                ext_gstate,
            } => {
                let mut pattern_dict = LoDictionary::new();
                pattern_dict.set("Type", Name(b"Pattern".to_vec()));
                pattern_dict.set("PatternType", Integer(2));
                pattern_dict.set("Shading", Dictionary(shading.to_dict()));
                pattern_dict.set("Matrix", matrix_object(matrix, &id.0, warnings));
                if let Some(state) = ext_gstate {
                    pattern_dict.set(
                        "ExtGState",
                        Dictionary(crate::graphics::extgstate_to_dict(state)),
                    );
                }
                doc.add_object(pattern_dict)
            }
            Pattern::External(item) => match item.to_lopdf_with_indirect_streams(doc) {
                Object::Reference(object_id) => object_id,
                object @ Object::Dictionary(_) => doc.add_object(object),
                _ => {
                    warnings.push(PdfWarnMsg::warning(
                        0,
                        0,
                        format!("pattern {} is neither a dictionary nor a stream; not written", id.0),
                    ));
                    continue;
                }
            },
        };
        dict.set(id.0.clone(), Object::Reference(object));
    }
    dict
}

/// The fields of a tiling pattern.
struct Tiling<'a> {
    id: &'a PatternId,
    paint_type: PatternPaintType,
    bbox: &'a Rect,
    xstep: f32,
    ystep: f32,
    matrix: &'a [f32; 6],
    ops: &'a [Op],
    resources: Option<&'a PdfResources>,
}

/// The stream of a tiling pattern: its dictionary, its `/Resources` and the content stream of
/// the cell.
fn write_tiling(
    tiling: &Tiling<'_>,
    doc: &mut lopdf::Document,
    parent: &SaveScope<'_>,
    opts: &PdfSaveOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> lopdf::Stream {
    let name = &tiling.id.0;

    // One text object per line instead of per glyph run, as for a page (see text_merge.rs).
    let merged;
    let ops: &[Op] = if opts.optimize {
        merged = crate::text_merge::merge_text_runs(tiling.ops);
        &merged
    } else {
        tiling.ops
    };

    // A pattern names only resources of its own (ISO 32000-1, 8.7.3.1).
    let empty = PdfResources::default();
    let local = tiling.resources.unwrap_or(&empty);
    let local_scope = write_local_scope(doc, local, ops, parent.depth.saturating_add(1), opts, warnings);

    let mut dict = LoDictionary::new();
    dict.set("Type", Name(b"Pattern".to_vec()));
    dict.set("PatternType", Integer(1));
    dict.set("PaintType", Integer(tiling.paint_type.number()));
    dict.set("TilingType", Integer(1));
    dict.set("BBox", bbox_array(&finite_rect(tiling.bbox)));
    let (xstep, ystep) = steps(tiling, name, warnings);
    dict.set("XStep", Real(xstep));
    dict.set("YStep", Real(ystep));
    dict.set("Matrix", matrix_object(tiling.matrix, name, warnings));
    dict.set("Resources", Dictionary(local_scope.resources));

    let content = translate_operations(
        ops,
        &local_scope.font_infos,
        &local.xobjects.map,
        opts.secure,
        warnings,
    );
    lopdf::Stream::new(dict, content).with_compression(true)
}

/// `[a b c d e f]`. A matrix with a value that is not a number is written as the identity.
fn matrix_object(matrix: &[f32; 6], name: &str, warnings: &mut Vec<PdfWarnMsg>) -> Object {
    let matrix = if matrix.iter().all(|v| v.is_finite()) {
        *matrix
    } else {
        warnings.push(PdfWarnMsg::warning(
            0,
            0,
            format!("the matrix of pattern {name} has a value that is not a number; the identity is written"),
        ));
        crate::pattern::IDENTITY_MATRIX
    };
    Array(matrix.iter().copied().map(Real).collect())
}

/// The rectangle with every value that is not a number replaced by 0.
fn finite_rect(rect: &Rect) -> Rect {
    let fix = |v: crate::Pt| if v.0.is_finite() { v } else { crate::Pt(0.0) };
    Rect {
        x: fix(rect.x),
        y: fix(rect.y),
        width: fix(rect.width),
        height: fix(rect.height),
        mode: rect.mode,
        winding_order: rect.winding_order,
    }
}

/// `/XStep` and `/YStep`. A step that is zero or not a number is replaced by the size of the
/// bounding box (1 when that is zero as well), because a viewer cannot repeat a cell that is
/// not moved.
fn steps(tiling: &Tiling<'_>, name: &str, warnings: &mut Vec<PdfWarnMsg>) -> (f32, f32) {
    let bbox = finite_rect(tiling.bbox);
    let mut fix = |step: f32, size: f32, which: &str| {
        if step.is_finite() && step != 0.0 {
            return step;
        }
        warnings.push(PdfWarnMsg::warning(
            0,
            0,
            format!("pattern {name}: {which} is {step}; the size of the bounding box is used"),
        ));
        let size = size.abs();
        if size > 0.0 {
            size
        } else {
            1.0
        }
    };
    (
        fix(tiling.xstep, bbox.width.0, "xstep"),
        fix(tiling.ystep, bbox.height.0, "ystep"),
    )
}
