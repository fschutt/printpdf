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
    pub color: ColorArray,
    pub actions: Actions,
    pub highlighting: HighlightingMode,
}

impl LinkAnnotation {
    /// Creates a new LinkAnnotation
    pub fn new(
        rect: Rect,
        actions: Actions,
        border: Option<BorderArray>,
        color: Option<ColorArray>,
        highlighting: Option<HighlightingMode>,
    ) -> Self {
        Self {
            rect,
            border: border.unwrap_or_default(),
            color: color.unwrap_or_default(),
            actions,
            highlighting: highlighting.unwrap_or_default(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum BorderArray {
    Solid([f32; 3]),
    Dashed([f32; 3], DashPhase),
}

impl BorderArray {
    pub fn to_array(&self) -> Vec<f32> {
        match self {
            BorderArray::Solid(s) => s.to_vec(),
            BorderArray::Dashed(s, dash_phase) => {
                let mut s = s.to_vec();
                s.push(dash_phase.phase);
                s
            }
        }
    }
}

/*

    impl Into<Object> for DashPhase {
        fn into(self) -> Object {
            Object::Array(vec![
                Object::Array(self.dash_array.into_iter().map(|x| Object::Real(x.into())).collect()),
                Object::Real(self.phase.into()),
            ])
        }
    }
*/

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

/*
    GoTo Go to a destination in the current document. “Go-To Actions” on page 654
    GoToR (“Go-to remote”) Go to a destination in another document. “Remote Go-To Actions” on page 655
    GoToE (“Go-to embedded”; PDF 1.6) Go to a destination in an embedded file. “Embedded Go-To Actions” on page 655
    Launch Launch an application, usually to open a file. “Launch Actions” on page 659
    Thread Begin reading an article thread. “Thread Actions” on page 661
    URI Resolve a uniform resource identifier. “URI Actions” on page 662
    Sound (PDF 1.2) Play a sound. “Sound Actions” on page 663
    Movie (PDF 1.2) Play a movie. “Movie Actions” on page 664
    Hide (PDF 1.2) Set an annotation’s Hidden flag. “Hide Actions” on page 665
    Named (PDF 1.2) Execute an action predefined by the viewer application. “Named Actions” on page 666
    SubmitForm (PDF 1.2) Send data to a uniform resource locator. “Submit-Form Actions” on page 703
    ResetForm (PDF 1.2) Set fields to their default values. “Reset-Form Actions” on page 707
    ImportData (PDF 1.2) Import field values from a file. “Import-Data Actions” on page 708
    JavaScript (PDF 1.3) Execute a JavaScript script. “JavaScript Actions” on page 709
    SetOCGState (PDF 1.5) Set the states of optional content groups. “Set-OCG-State Actions” on page 667
    Rendition (PDF 1.5) Controls the playing of multimedia content. “Rendition Actions” on page 668
    Trans (PDF 1.5) Updates the display of a document, using a transition dictionary. “Transition Actions” on page 670
    GoTo3DView (PDF 1.6) Set the current view of a 3D annotation “Go-To-3D-View Actions” on page 670
*/
#[derive(Debug, PartialEq, Clone)]
pub enum Actions {
    GoTo(Destination),
    URI(String),
}

impl Actions {
    /// 8.5.3 Action Types: PDF supports the standard action types listed in Table 8.48.
    ///
    /// The following sections describe each of these types in detail.
    /// Plug-in extensions may add new action types.
    pub fn get_action_type_id(&self) -> &'static str {
        match self {
            Actions::GoTo(_) => "GoTo",
            Actions::URI(_) => "URI",
        }
    }

    pub fn go_to(destination: Destination) -> Self {
        Self::GoTo(destination)
    }

    pub fn uri(uri: String) -> Self {
        Self::URI(uri)
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum HighlightingMode {
    None,
    #[default]
    Invert,
    Outline,
    Push,
}

impl HighlightingMode {
    pub fn get_id(&self) -> &'static str {
        use self::HighlightingMode::*;
        match self {
            None => "N",
            Invert => "I",
            Outline => "O",
            Push => "P",
        }
    }
}
