use crate::{image::RawImage, matrix::CurTransMat, units::{Pt, Px}, OffsetDateTime, PdfDocument};

/* Parent: Resources dictionary of the page */
/// External object that gets reference outside the PDF content stream
/// Gets constructed similar to the `ExtGState`, then inserted into the `/XObject` dictionary
/// on the page. You can instantiate `XObjects` with the `/Do` operator. The `layer.add_xobject()`
/// (or better yet, the `layer.add_image()`, `layer.add_form()`) methods will do this for you.
#[derive(Debug, PartialEq, Clone)]
pub enum XObject {
    /// Image XObject, for images
    Image(RawImage),
    /// Form XObject, NOT A PDF FORM, this just allows repeatable content
    /// on a page
    Form(Box<FormXObject>),
    /// XObject embedded from an external stream
    ///
    /// This is mainly used to add XObjects to the resources that the library
    /// doesn't support natively (such as gradients, patterns, etc).
    ///
    /// The only thing this does is to ensure that this stream is set on
    /// the /Resources dictionary of the page. The `XObjectRef` returned
    /// by `add_xobject()` is the unique name that can be used to invoke
    /// the `/Do` operator (by the `use_xobject`)
    External(ExternalXObject),
}

// translates the xobject to a document object ID
pub(crate) fn add_xobject_to_document(xobj: &XObject, doc: &mut lopdf::Document) -> lopdf::ObjectId {

    // in the PDF content stream, reference an XObject like this
    match xobj {
        XObject::Image(i) => {
            let stream = crate::image::image_to_stream(i.clone(), doc);
            doc.add_object(stream)
        },
        XObject::Form(f) => {
            let stream = form_xobject_to_stream(f, doc);
            doc.add_object(stream)
        },
        XObject::External(external_xobject) => {
            use lopdf::Object::Integer;
            let mut stream = external_xobject.stream.clone();
            if let Some(w) = external_xobject.width {
                stream.dict.set("Width", Integer(w.into_pt(300.0).0.round() as i64));
            }
            if let Some(h) = external_xobject.height {
                stream.dict.set("Width", Integer(h.into_pt(300.0).0.round() as i64));
            }
            doc.add_object(stream)
        },
    }    
}

/// External XObject, invoked by `/Do` graphics operator
#[derive(Debug, PartialEq, Clone)]
pub struct ExternalXObject {
    /// External stream of graphics operations
    pub stream: lopdf::Stream,
    /// Optional width
    pub width: Option<Px>,
    /// Optional height
    pub height: Option<Px>,
}

/// Describes the format the image bytes are compressed with.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ImageFilter {
    /// ???
    Ascii85,
    /// Lempel Ziv Welch compression, i.e. zip
    Lzw,
    /// Discrete Cosinus Transform, JPEG Baseline.
    DCT,
    /// JPEG2000 aka JPX wavelet based compression.
    JPX,
}


/// __THIS IS NOT A PDF FORM!__ A form `XObject` can be nearly everything.
/// PDF allows you to reuse content for the graphics stream in a `FormXObject`.
/// A `FormXObject` is basically a layer-like content stream and can contain anything
/// as long as it's a valid strem. A `FormXObject` is intended to be used for reapeated
/// content on one page.
#[derive(Debug, PartialEq, Clone)]
pub struct FormXObject {
    /* /Type /XObject */
    /* /Subtype /Form */

    /* /FormType Integer */
    /// Form type (currently only Type1)
    pub form_type: FormType,
    /// The actual content of this FormXObject
    pub bytes: Vec<u8>,
    /* /Matrix [Integer , 6] */
    /// Optional matrix, maps the form into user space
    pub matrix: Option<CurTransMat>,
    /* /Resources << dictionary >> */
    /// (Optional but strongly recommended; PDF 1.2) A dictionary specifying
    /// any resources (such as fonts and images) required by the form XObject
    /// (see Section 3.7, “Content Streams and Resources”).
    ///
    /// In PDF 1.1 and earlier, all named resources used in the form XObject must be
    /// included in the resource dictionary of each page object on which the form
    /// XObject appears, regardless of whether they also appear in the resource
    /// dictionary of the form XObject. It can be useful to specify these resources
    /// in the form XObject’s resource dictionary as well, to determine which resources
    /// are used inside the form XObject. If a resource is included in both dictionaries,
    /// it should have the same name in both locations.
    ///  /// In PDF 1.2 and later versions, form XObjects can be independent of the content
    /// streams in which they appear, and this is strongly recommended although not
    /// required. In an independent form XObject, the resource dictionary of the form
    /// XObject is required and contains all named resources used by the form XObject.
    /// These resources are not promoted to the outer content stream’s resource
    /// dictionary, although that stream’s resource dictionary refers to the form XObject.
    pub resources: Option<lopdf::Dictionary>,
    /* /Group << dictionary >> */
    /// (Optional; PDF 1.4) A group attributes dictionary indicating that the contents of the
    /// form XObject are to be treated as a group and specifying the attributes of that group
    /// (see Section 4.9.2, “Group XObjects”).
    ///
    /// Note: If a Ref entry (see below) is present, the group attributes also apply to the
    /// external page imported by that entry, which allows such an imported page to be treated
    /// as a group without further modification.
    pub group: Option<GroupXObject>,
    /* /Ref << dictionary >> */
    /// (Optional; PDF 1.4) A reference dictionary identifying a page to be imported from another
    /// PDF file, and for which the form XObject serves as a proxy (see Section 4.9.3, “Reference XObjects”).
    pub ref_dict: Option<lopdf::Dictionary>,
    /* /Metadata [stream] */
    /// (Optional; PDF 1.4) A metadata stream containing metadata for the form XObject
    /// (see Section 10.2.2, “Metadata Streams”).
    pub metadata: Option<lopdf::Stream>,
    /* /PieceInfo << dictionary >> */
    /// (Optional; PDF 1.3) A page-piece dictionary associated with the form XObject
    /// (see Section 10.4, “Page-Piece Dictionaries”).
    pub piece_info: Option<lopdf::Dictionary>,
    /* /LastModified (date) */
    /// (Required if PieceInfo is present; optional otherwise; PDF 1.3) The date and time
    /// (see Section 3.8.3, “Dates”) when the form XObject’s contents were most recently
    /// modified. If a page-piece dictionary (PieceInfo) is present, the modification date
    /// is used to ascertain which of the application data dictionaries it contains correspond
    /// to the current content of the form (see Section 10.4, “Page-Piece Dictionaries”).
    pub last_modified: Option<OffsetDateTime>,
    /* /StructParent integer */
    /// (Required if the form XObject is a structural content item; PDF 1.3) The integer key of
    /// the form XObject’s entry in the structural parent tree (see “Finding Structure Elements
    /// from Content Items” on page 868).
    pub struct_parent: Option<i64>,
    /* /StructParents integer */
    /// __(Required if the form XObject contains marked-content sequences that are structural content
    /// items; PDF 1.3)__ The integer key of the form XObject’s entry in the structural parent tree
    /// (see “Finding Structure Elements from Content Items” on page 868).
    ///
    /// __Note:__ At most one of the entries StructParent or StructParents may be present. A form
    /// XObject can be either a content item in its entirety or a container for marked-content sequences
    /// that are content items, but not both.
    pub struct_parents: Option<i64>,
    /* /OPI << dictionary >> */
    /// (Optional; PDF 1.2) An OPI version dictionary for the form XObject
    /// (see Section 10.10.6, “Open Prepress Interface (OPI)”).
    pub opi: Option<lopdf::Dictionary>,
    /// (Optional; PDF 1.5) An optional content group or optional content membership dictionary
    /// (see Section 4.10, “Optional Content”) specifying the optional content properties for
    /// the form XObject. Before the form is processed, its visibility is determined based on
    /// this entry. If it is determined to be invisible, the entire form is skipped, as if there
    /// were no Do operator to invoke it.
    pub oc: Option<lopdf::Dictionary>,
    /* /Name /MyName */
    /// __(Required in PDF 1.0; optional otherwise)__ The name by which this form XObject is referenced
    /// in the XObject subdictionary of the current resource dictionary
    /// (see Section 3.7.2, “Resource Dictionaries”).
    /// __Note:__ This entry is obsolescent and its use is no longer recommended.
    /// (See implementation note 55 in Appendix H.)
    pub name: Option<String>,
}

fn form_xobject_to_stream(f: &FormXObject, doc: &mut lopdf::Document) -> lopdf::Stream {

    use lopdf::Object::*;
    use lopdf::Object::String as LoString;

    let mut dict = lopdf::Dictionary::from_iter(vec![
        ("Type", Name("XObject".into())),
        ("Subtype", Name("Form".into())),
        ("FormType", Name(f.form_type.get_id().into())),
    ]);

    if let Some(matrix) = f.matrix.as_ref() {
        dict.set("Matrix", Array(matrix.as_array().into_iter().map(Real).collect()));
    }

    if let Some(res) = f.resources.as_ref() {
        dict.set("Resources", res.clone());
    }

    if let Some(g) = f.group.as_ref() {

        let group_dict = lopdf::Dictionary::from_iter(vec![
            ("Type", Name("Group".into())),
            ("S", Name(g.grouptype.get_id().into())),
        ]);
        
        dict.set("Group", Dictionary(group_dict));
    }

    if let Some(r) = f.ref_dict.as_ref() {
        dict.set("Ref", r.clone());
    }

    if let Some(r) = f.metadata.as_ref() {
        dict.set("Metadata", doc.add_object(r.clone()));
    }

    if let Some(r) = f.piece_info.as_ref() {
        dict.set("PieceInfo", doc.add_object(r.clone()));
    }

    if let Some(r) = f.last_modified.as_ref() {
        dict.set("LastModified", LoString(crate::utils::to_pdf_time_stamp_metadata(r).into_bytes(), lopdf::StringFormat::Literal));
    }

    if let Some(r) = f.opi.as_ref() {
        dict.set("OPI", r.clone());
    }

    if let Some(r) = f.oc.as_ref() {
        dict.set("OC", r.clone());
    }

    if let Some(r) = f.name.as_ref() {
        dict.set("Name", LoString(r.clone().into(), lopdf::StringFormat::Literal));
    }

    if let Some(sp) = &f.struct_parents {
        dict.set("StructParents", Integer(*sp));
    } else if let Some(sp) = &f.struct_parent {
        dict.set("StructParent", Integer(*sp));
    }

    let mut stream = lopdf::Stream::new(dict, f.bytes.clone()).with_compression(true);
    let _ = stream.compress();
    stream
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum FormType {
    /// The only form type ever declared by Adobe
    /* Integer(1) */
    Type1,
}

impl FormType {
    fn get_id(&self) -> &'static str {
        match self {
            FormType::Type1 => "Type1",
        }
    }
}

/// `/Type /Group`` (PDF reference section 4.9.2)
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct GroupXObject {
    pub grouptype: GroupXObjectType,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum GroupXObjectType {
    /// Transparency group XObject (currently the only valid GroupXObject type)
    TransparencyGroup,
}

impl GroupXObjectType {
    pub fn get_id(&self) -> &'static str {
        match self {
            GroupXObjectType::TransparencyGroup => "Transparency",
        }
    }
}

/// PDF 1.4 and higher
/// Contains a PDF file to be embedded in the current PDF
#[derive(Debug, PartialEq, Clone)]
pub struct ReferenceXObject {
    /// (Required) The file containing the target document. (?)
    pub file: Vec<u8>,
    /// Page number to embed
    pub page: i64,
    /// Optional, should be the document ID and version ID from the metadata
    pub id: [i64; 2],
}

/// TODO, very low priority
#[derive(Debug, PartialEq, Clone)]
pub struct PostScriptXObject {
    /// __(Optional)__ A stream whose contents are to be used in
    /// place of the PostScript XObject’s stream when the target
    /// PostScript interpreter is known to support only LanguageLevel 1
    #[allow(dead_code)]
    level1: Option<Vec<u8>>,
}

/// Transform that is applied immediately before the
/// image gets painted. Does not affect anything other
/// than the image.
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct XObjectTransform {
    pub translate_x: Option<Pt>,
    pub translate_y: Option<Pt>,
    /// Rotate (clockwise), in degree angles
    pub rotate: Option<XObjectRotation>,
    pub scale_x: Option<f32>,
    pub scale_y: Option<f32>,
    /// If set to None, will be set to 300.0 for images
    pub dpi: Option<f32>,
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct XObjectRotation {
    pub angle_ccw_degrees: f32,
    pub rotation_center_x: Pt,
    pub rotation_center_y: Pt,
}
