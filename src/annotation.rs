//! Bookmarks, page and link annotations

use crate::graphics::Rect;

#[derive(Debug, PartialEq, Clone)]
pub struct PageAnnotation {
    /// Name of the bookmark annotation (i.e. "Chapter 5")
    pub name: String,
    /// Which page to jump to (i.e "page 10" = 10)
    pub page: usize,
}

#[derive(Debug, PartialEq, Clone)]
pub struct LinkAnnotation {
    pub rect: Rect,
    pub border: BorderArray,
    pub c: ColorArray,
    pub a: Actions,
    pub h: HighlightingMode,
}

impl LinkAnnotation {
    /// Creates a new LinkAnnotation
    pub fn new(
        rect: Rect,
        border: Option<BorderArray>,
        c: Option<ColorArray>,
        a: Actions,
        h: Option<HighlightingMode>,
    ) -> Self {
        Self {
            rect,
            border: border.unwrap_or_default(),
            c: c.unwrap_or_default(),
            a,
            h: h.unwrap_or_default(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum BorderArray {
    Solid([f32; 3]),
    Dashed([f32; 3], DashPhase),
}

impl Default for BorderArray {
    fn default() -> Self {
        BorderArray::Solid([0.0, 0.0, 1.0])
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DashPhase {
    pub dash_array: Vec<f32>,
    pub phase: f32,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ColorArray {
    Transparent,
    Gray([f32; 1]),
    RGB([f32; 3]),
    CMYK([f32; 4]),
}

impl Default for ColorArray {
    fn default() -> Self {
        ColorArray::RGB([0.0, 1.0, 1.0])
    }
}

#[derive(Debug, PartialEq, Clone)]
#[non_exhaustive]
pub enum Destination {
    /// Display `page` with coordinates `top` and `left` positioned at the upper-left corner of the
    /// window and the contents of the page magnified by `zoom`.
    ///
    /// A value of `None` for any parameter indicates to leave the current value unchanged, and a
    /// `zoom` value of 0 has the same meaning as `None`.
    XYZ {
        page: usize,
        left: Option<f32>,
        top: Option<f32>,
        zoom: Option<f32>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Actions {
    GoTo(Destination),
    URI(String),
}

impl Actions {
    pub fn go_to(destination: Destination) -> Self {
        Self::GoTo(destination)
    }

    pub fn uri(uri: String) -> Self {
        Self::URI(uri)
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HighlightingMode {
    None,
    Invert,
    Outline,
    Push,
}

impl Default for HighlightingMode {
    fn default() -> Self {
        HighlightingMode::Invert
    }
}
