use crate::{color::{ColorBits, ColorSpace}, matrix::CurTransMat, units::Px, OffsetDateTime};

/* Parent: Resources dictionary of the page */
/// External object that gets reference outside the PDF content stream
/// Gets constructed similar to the `ExtGState`, then inserted into the `/XObject` dictionary
/// on the page. You can instantiate `XObjects` with the `/Do` operator. The `layer.add_xobject()`
/// (or better yet, the `layer.add_image()`, `layer.add_form()`) methods will do this for you.
#[derive(Debug, PartialEq, Clone)]
pub enum XObject {
    /* /Subtype /Image */
    /// Image XObject, for images
    Image(ImageXObject),
    /* /Subtype /Form */
    /// Form XObject, for PDF forms
    Form(Box<FormXObject>),
    /* /Subtype /PS */
    /// Embedded PostScript XObject, for legacy applications
    /// You can embed PostScript in a PDF, it is not recommended
    PostScript(PostScriptXObject),
    /// XObject embedded from an external stream
    ///
    /// This is mainly used to add XObjects to the resources that the library
    /// doesn't support natively (such as gradients, patterns, etc).
    ///
    /// The only thing this does is to ensure that this stream is set on
    /// the /Resources dictionary of the page. The `XObjectRef` returned
    /// by `add_xobject()` is the unique name that can be used to invoke
    /// the `/Do` operator (by the `use_xobject`)
    External(lopdf::Stream),
}


#[derive(Debug, PartialEq, Clone)]
pub struct ImageXObject {
    /// Width of the image (original width, not scaled width)
    pub width: Px,
    /// Height of the image (original height, not scaled height)
    pub height: Px,
    /// Color space (Greyscale, RGB, CMYK)
    pub color_space: ColorSpace,
    /// Bits per color component (1, 2, 4, 8, 16) - 1 for black/white, 8 Greyscale / RGB, etc.
    /// If using a JPXDecode filter (for JPEG images), this can be inferred from the image data
    pub bits_per_component: ColorBits,
    /// Should the image be interpolated when scaled?
    pub interpolate: bool,
    /// The actual data from the image
    pub image_data: Vec<u8>,
    /// Decompression filter for `image_data`, if `None` assumes uncompressed raw pixels in the expected color format.
    pub image_filter: Option<ImageFilter>,
    // SoftMask for transparency, if `None` assumes no transparency. See page 444 of the adope pdf 1.4 reference
    pub smask: Option<lopdf::ObjectId>,
    /* /BBox << dictionary >> */
    /* todo: find out if this is really required */
    /// Required bounds to clip the image, in unit space
    /// Default value: Identity matrix (`[1 0 0 1 0 0]`) - used when value is `None`
    pub clipping_bbox: Option<CurTransMat>,
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

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum FormType {
    /// The only form type ever declared by Adobe
    /* Integer(1) */
    Type1,
}


#[derive(Debug, PartialEq, Copy, Clone)]
pub struct GroupXObject {
    /* /Type /Group */
    /* /S /Transparency */ /* currently the only valid GroupXObject */
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum GroupXObjectType {
    /// Transparency group XObject
    TransparencyGroup,
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

