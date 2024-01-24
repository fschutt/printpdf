use crate::{
    ExtendedGraphicsState, ExtendedGraphicsStateList, ExtendedGraphicsStateRef, LinkAnnotation, LinkAnnotationList,
    LinkAnnotationRef, OCGList, OCGRef, Pattern, PatternList, PatternRef, XObject, XObjectList, XObjectRef,
};
use lopdf;

/// Struct for storing the PDF Resources, to be used on a PDF page
#[derive(Default, Debug, Clone)]
pub struct PdfResources {
    /// External graphics objects
    pub xobjects: XObjectList,
    /// Patterns used on this page. Do not yet, use, placeholder.
    pub patterns: PatternList,
    /// Graphics states used on this page
    pub graphics_states: ExtendedGraphicsStateList,
    /// Layers / optional content ("Properties") in the resource dictionary
    pub layers: OCGList,
    /// Link Annotations used in this page
    pub link_annotations: LinkAnnotationList,
}

impl PdfResources {
    /// Creates a new PdfResources struct (resources for exactly one PDF page)
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a graphics state to the resources
    #[inline]
    pub fn add_graphics_state(&mut self, added_state: ExtendedGraphicsState) -> ExtendedGraphicsStateRef {
        self.graphics_states.add_graphics_state(added_state)
    }

    /// Adds an XObject to the page
    #[inline]
    pub fn add_xobject(&mut self, xobj: XObject) -> XObjectRef {
        self.xobjects.add_xobject(xobj)
    }

    /// __STUB__: Adds a pattern to the resources, to be used like a color
    #[inline]
    pub fn add_pattern(&mut self, pattern: Pattern) -> PatternRef {
        self.patterns.add_pattern(pattern)
    }

    /// __STUB__: Adds a link annotation to the resources
    #[inline]
    pub fn add_link_annotation(&mut self, link_annotation: LinkAnnotation) -> LinkAnnotationRef {
        self.link_annotations.add_link_annotation(link_annotation)
    }

    /// See `XObject::Into_with_document`.
    /// The resources also need access to the layers (the optional content groups), this should be a
    /// `Vec<lopdf::Object::Reference>` (to the actual OCG groups, which are added on the document level)

    pub fn into_with_document_and_layers(
        self,
        doc: &mut lopdf::Document,
        layers: Vec<lopdf::Object>,
    ) -> (lopdf::Dictionary, Vec<OCGRef>) {
        let mut dict = lopdf::Dictionary::new();

        let mut ocg_dict = self.layers;
        let mut ocg_references = Vec::<OCGRef>::new();

        let xobjects_dict: lopdf::Dictionary = self.xobjects.into_with_document(doc);
        let patterns_dict: lopdf::Dictionary = self.patterns.into();
        let graphics_state_dict: lopdf::Dictionary = self.graphics_states.into();
        let annotations_dict: lopdf::Dictionary = self.link_annotations.into();

        if !layers.is_empty() {
            for l in layers {
                ocg_references.push(ocg_dict.add_ocg(l));
            }

            let cur_ocg_dict_obj: lopdf::Dictionary = ocg_dict.into();

            if !cur_ocg_dict_obj.is_empty() {
                dict.set("Properties", lopdf::Object::Dictionary(cur_ocg_dict_obj));
            }
        }

        if !xobjects_dict.is_empty() {
            dict.set("XObject", lopdf::Object::Dictionary(xobjects_dict));
        }

        if !patterns_dict.is_empty() {
            dict.set("Pattern", lopdf::Object::Dictionary(patterns_dict));
        }

        if !graphics_state_dict.is_empty() {
            dict.set("ExtGState", lopdf::Object::Dictionary(graphics_state_dict));
        }

        if !annotations_dict.is_empty() {
            dict.set("Annots", lopdf::Object::Dictionary(annotations_dict));
        }

        (dict, ocg_references)
    }
}
