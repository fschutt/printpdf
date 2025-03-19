use std::collections::BTreeMap;

use lopdf::StringFormat;
use serde_derive::{Deserialize, Serialize};

use crate::{
    date::OffsetDateTime,
    deserialize::PageState,
    image::RawImage,
    matrix::CurTransMat,
    units::{Pt, Px},
    ImageOptimizationOptions, Op,
};

/* Parent: Resources dictionary of the page */
/// External object that gets reference outside the PDF content stream
/// Gets constructed similar to the `ExtGState`, then inserted into the `/XObject` dictionary
/// on the page. You can instantiate `XObjects` with the `/Do` operator. The `layer.add_xobject()`
/// (or better yet, the `layer.add_image()`, `layer.add_form()`) methods will do this for you.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum XObject {
    /// Image XObject, for images
    Image(RawImage),
    /// Form XObject, NOT A PDF FORM, this just allows repeatable content
    /// on a page
    Form(FormXObject),
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

impl XObject {
    pub fn get_width_height(&self) -> Option<(Px, Px)> {
        match self {
            XObject::Image(raw_image) => Some((Px(raw_image.width), Px(raw_image.height))),
            XObject::Form(form_xobject) => form_xobject.size,
            XObject::External(external_xobject) => {
                Some((external_xobject.width?, external_xobject.height?))
            }
        }
    }
}

// translates the xobject to a document object ID
pub(crate) fn add_xobject_to_document(
    xobj: &XObject,
    doc: &mut lopdf::Document,
    image_opts: Option<&ImageOptimizationOptions>,
) -> lopdf::ObjectId {
    // in the PDF content stream, reference an XObject like this
    match xobj {
        XObject::Image(i) => {
            let stream = crate::image::image_to_stream(i.clone(), doc, image_opts);
            doc.add_object(stream)
        }
        XObject::Form(f) => {
            let stream = form_xobject_to_stream(f, doc);
            doc.add_object(stream)
        }
        XObject::External(external_xobject) => {
            let stream = external_xobject.stream.into_lopdf();
            doc.add_object(stream)
        }
    }
}

/// External XObject, invoked by `/Do` graphics operator
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalXObject {
    /// External stream of graphics operations
    pub stream: ExternalStream,
    /// Optional width
    #[serde(default)]
    pub width: Option<Px>,
    /// Optional height
    #[serde(default)]
    pub height: Option<Px>,
    /// Optional DPI of the object
    #[serde(default)]
    pub dpi: Option<f32>,
}

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalStream {
    /// Stream description, for simplicity a simple map, corresponds to PDF dict
    pub dict: BTreeMap<String, DictItem>,
    /// Stream content
    pub content: Vec<u8>,
    /// Whether the stream can be compressed
    pub compress: bool,
}

impl ExternalStream {
    pub(crate) fn into_lopdf(&self) -> lopdf::Stream {
        lopdf::Stream::new(build_dict(&self.dict), self.content.clone())
            .with_compression(self.compress)
    }
    pub fn decompressed_content(&self) -> Vec<u8> {
        self.into_lopdf()
            .decompressed_content()
            .unwrap_or(self.content.clone())
    }

    /// Decode a stream of `Op` from a string (usually to debug PDF issues)
    pub fn decode_ops(s: &str) -> Result<Vec<Op>, String> {
        Self::get_ops_internal(s.as_bytes())
    }

    /// If the stream is decodable as PDF operations, return the operations of the stream
    pub fn get_ops(&self) -> Result<Vec<Op>, String> {
        Self::get_ops_internal(&self.decompressed_content())
    }

    fn get_ops_internal(s: &[u8]) -> Result<Vec<Op>, String> {
        // Decode the content stream into a vector of lopdf operations.
        let content = lopdf::content::Content::decode(&s)
            .map_err(|e| format!("Failed to decode content stream: {}", e))?;

        // Convert lopdf operations to printpdf Ops.
        let mut page_state = PageState::default();
        let mut printpdf_ops = Vec::new();

        for (op_id, op) in content.operations.iter().enumerate() {
            let parsed_op = crate::deserialize::parse_op(
                0,
                op_id,
                &op,
                &mut page_state,
                &mut BTreeMap::new(),
                &mut BTreeMap::new(),
                &mut Vec::new(),
            )?;
            printpdf_ops.extend(parsed_op.into_iter());
        }

        Ok(printpdf_ops)
    }
}

/// Simplified dict item for external streams
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum DictItem {
    Array(Vec<DictItem>),
    String { data: Vec<u8>, literal: bool },
    Bytes(Vec<u8>),
    Bool(bool),
    Float(f32),
    Int(i64),
    Real(f32),
    Name(Vec<u8>),
    Ref { obj: u32, gen: u16 },
    Dict { map: BTreeMap<String, DictItem> },
    Stream { stream: ExternalStream },
    Null,
}

impl DictItem {
    pub fn to_lopdf(&self) -> lopdf::Object {
        use lopdf::{Object, StringFormat};
        match self {
            DictItem::Array(items) => {
                let objs = items.iter().map(|item| item.to_lopdf()).collect();
                Object::Array(objs)
            }
            DictItem::String { data, literal } => {
                let format = if *literal {
                    StringFormat::Literal
                } else {
                    StringFormat::Hexadecimal
                };
                Object::String(data.clone(), format)
            }
            DictItem::Bytes(data) => {
                // Treat bytes as a hexadecimal string.
                Object::String(data.clone(), StringFormat::Hexadecimal)
            }
            DictItem::Bool(b) => Object::Boolean(*b),
            DictItem::Float(f) => Object::Real(*f),
            DictItem::Int(i) => Object::Integer(*i),
            DictItem::Real(f) => Object::Real(*f),
            DictItem::Name(name) => Object::Name(name.clone()),
            DictItem::Ref { obj, gen } => Object::Reference((*obj, *gen)),
            DictItem::Dict { map } => {
                let dict = map
                    .iter()
                    .map(|(k, v)| (k.as_bytes().to_vec(), v.to_lopdf()))
                    .collect();
                Object::Dictionary(dict)
            }
            DictItem::Stream { stream } => {
                let stream_obj = stream.into_lopdf();
                Object::Stream(stream_obj)
            }
            DictItem::Null => Object::Null,
        }
    }

    pub fn from_lopdf(o: &lopdf::Object) -> Self {
        use lopdf::Object;
        match o {
            Object::Null => DictItem::Null,
            Object::Boolean(t) => DictItem::Bool(*t),
            Object::Integer(i) => DictItem::Int(*i),
            Object::Real(r) => DictItem::Real(*r),
            Object::Name(items) => DictItem::Name(items.clone()),
            Object::String(items, string_format) => DictItem::String {
                data: items.clone(),
                literal: *string_format == StringFormat::Literal,
            },
            Object::Array(objects) => {
                DictItem::Array(objects.iter().map(DictItem::from_lopdf).collect())
            }
            Object::Dictionary(dictionary) => DictItem::Dict {
                map: dictionary
                    .iter()
                    .map(|s| {
                        (
                            String::from_utf8_lossy(&s.0).to_string(),
                            DictItem::from_lopdf(s.1),
                        )
                    })
                    .collect(),
            },
            Object::Stream(stream) => DictItem::Stream {
                stream: ExternalStream {
                    compress: stream.allows_compression,
                    content: stream.content.clone(),
                    dict: stream
                        .dict
                        .iter()
                        .map(|s| {
                            (
                                String::from_utf8_lossy(&s.0).to_string(),
                                DictItem::from_lopdf(s.1),
                            )
                        })
                        .collect(),
                },
            },
            Object::Reference((a, b)) => DictItem::Ref { obj: *a, gen: *b },
        }
    }
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
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormXObject {
    // /Type /XObject
    // /Subtype /Form
    // /FormType Integer
    /// Form type (currently only Type1)
    pub form_type: FormType,
    /// Optional width / height, affects the width / height on instantiation
    pub size: Option<(Px, Px)>,
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
    pub resources: Option<BTreeMap<String, DictItem>>,
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
    /// PDF file, and for which the form XObject serves as a proxy (see Section 4.9.3, “Reference
    /// XObjects”).
    pub ref_dict: Option<BTreeMap<String, DictItem>>,
    /* /Metadata [stream] */
    /// (Optional; PDF 1.4) A metadata stream containing metadata for the form XObject
    /// (see Section 10.2.2, “Metadata Streams”).
    pub metadata: Option<BTreeMap<String, DictItem>>,
    /* /PieceInfo << dictionary >> */
    /// (Optional; PDF 1.3) A page-piece dictionary associated with the form XObject
    /// (see Section 10.4, “Page-Piece Dictionaries”).
    pub piece_info: Option<BTreeMap<String, DictItem>>,
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
    /// __(Required if the form XObject contains marked-content sequences that are structural
    /// content items; PDF 1.3)__ The integer key of the form XObject’s entry in the structural
    /// parent tree (see “Finding Structure Elements from Content Items” on page 868).
    ///
    /// __Note:__ At most one of the entries StructParent or StructParents may be present. A form
    /// XObject can be either a content item in its entirety or a container for marked-content
    /// sequences that are content items, but not both.
    pub struct_parents: Option<i64>,
    /* /OPI << dictionary >> */
    /// (Optional; PDF 1.2) An OPI version dictionary for the form XObject
    /// (see Section 10.10.6, “Open Prepress Interface (OPI)”).
    pub opi: Option<BTreeMap<String, DictItem>>,
    /// (Optional; PDF 1.5) An optional content group or optional content membership dictionary
    /// (see Section 4.10, “Optional Content”) specifying the optional content properties for
    /// the form XObject. Before the form is processed, its visibility is determined based on
    /// this entry. If it is determined to be invisible, the entire form is skipped, as if there
    /// were no Do operator to invoke it.
    pub oc: Option<BTreeMap<String, DictItem>>,
    /* /Name /MyName */
    /// __(Required in PDF 1.0; optional otherwise)__ The name by which this form XObject is
    /// referenced in the XObject subdictionary of the current resource dictionary
    /// (see Section 3.7.2, “Resource Dictionaries”).
    /// __Note:__ This entry is obsolescent and its use is no longer recommended.
    /// (See implementation note 55 in Appendix H.)
    pub name: Option<String>,
}

fn form_xobject_to_stream(f: &FormXObject, doc: &mut lopdf::Document) -> lopdf::Stream {
    use lopdf::Object::{String as LoString, *};

    let mut dict = lopdf::Dictionary::from_iter(vec![
        ("Type", Name("XObject".into())),
        ("Subtype", Name("Form".into())),
        ("FormType", Name(f.form_type.get_id().into())),
    ]);

    if let Some(matrix) = f.matrix.as_ref() {
        dict.set(
            "Matrix",
            Array(matrix.as_array().into_iter().map(Real).collect()),
        );
    }

    if let Some(res) = f.resources.as_ref() {
        dict.set("Resources", build_dict(res));
    }

    if let Some(g) = f.group.as_ref() {
        let group_dict = lopdf::Dictionary::from_iter(vec![
            ("Type", Name("Group".into())),
            ("S", Name(g.group_type.get_id().into())),
        ]);

        dict.set("Group", Dictionary(group_dict));
    }

    if let Some(r) = f.ref_dict.as_ref() {
        dict.set("Ref", build_dict(&r));
    }

    if let Some(r) = f.metadata.as_ref() {
        dict.set("Metadata", doc.add_object(build_dict(&r)));
    }

    if let Some(r) = f.piece_info.as_ref() {
        dict.set("PieceInfo", doc.add_object(build_dict(&r)));
    }

    if let Some(r) = f.last_modified.as_ref() {
        dict.set(
            "LastModified",
            LoString(
                crate::utils::to_pdf_time_stamp_metadata(r).into_bytes(),
                lopdf::StringFormat::Literal,
            ),
        );
    }

    if let Some(r) = f.opi.as_ref() {
        dict.set("OPI", build_dict(&r));
    }

    if let Some(r) = f.oc.as_ref() {
        dict.set("OC", build_dict(&r));
    }

    if let Some(r) = f.name.as_ref() {
        dict.set(
            "Name",
            LoString(r.clone().into(), lopdf::StringFormat::Literal),
        );
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

pub fn build_dict(r: &BTreeMap<String, DictItem>) -> lopdf::Dictionary {
    lopdf::Dictionary::from_iter(r.iter().map(|(k, v)| (k.clone(), v.to_lopdf())))
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupXObject {
    #[serde(default)]
    pub group_type: GroupXObjectType,
}

#[derive(Debug, Default, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupXObjectType {
    /// Transparency group XObject (currently the only valid GroupXObject type)
    #[default]
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
#[derive(Debug, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct ReferenceXObject {
    /// (Required) The file containing the target document. (?)
    pub file: Vec<u8>,
    /// Page number to embed
    pub page: i64,
    /// Optional, should be the document ID and version ID from the metadata
    pub id: [i64; 2],
}

/// TODO, very low priority
#[derive(Debug, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct PostScriptXObject {
    /// __(Optional)__ A stream whose contents are to be used in
    /// place of the PostScript XObject’s stream when the target
    /// PostScript interpreter is known to support only LanguageLevel 1
    #[allow(dead_code)]
    pub level1: Option<Vec<u8>>,
}

/// Transform that is applied immediately before the
/// image gets painted. Does not affect anything other
/// than the image.
#[derive(Debug, Copy, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct XObjectTransform {
    #[serde(default)]
    pub translate_x: Option<Pt>,
    #[serde(default)]
    pub translate_y: Option<Pt>,
    /// Rotate (clockwise), in degree angles
    #[serde(default)]
    pub rotate: Option<XObjectRotation>,
    #[serde(default)]
    pub scale_x: Option<f32>,
    #[serde(default)]
    pub scale_y: Option<f32>,
    /// If set to None, will be set to 300.0 for images
    #[serde(default)]
    pub dpi: Option<f32>,
}

impl XObjectTransform {
    pub fn get_ctms(&self, wh: Option<(Px, Px)>) -> Vec<CurTransMat> {
        let mut transforms = Vec::new();
        let dpi = self.dpi.unwrap_or(300.0);

        if let Some((w, h)) = wh {
            // PDF maps an image to a 1x1 square, we have to
            // adjust the transform matrix to fix the distortion

            // Image at the given dpi should 1px = 1pt
            transforms.push(CurTransMat::Scale(w.into_pt(dpi).0, h.into_pt(dpi).0));
        }

        if self.scale_x.is_some() || self.scale_y.is_some() {
            let scale_x = self.scale_x.unwrap_or(1.0);
            let scale_y = self.scale_y.unwrap_or(1.0);
            transforms.push(CurTransMat::Scale(scale_x, scale_y));
        }

        if let Some(rotate) = self.rotate.as_ref() {
            transforms.push(CurTransMat::Translate(
                Pt(-rotate.rotation_center_x.into_pt(dpi).0),
                Pt(-rotate.rotation_center_y.into_pt(dpi).0),
            ));
            transforms.push(CurTransMat::Rotate(rotate.angle_ccw_degrees));
            transforms.push(CurTransMat::Translate(
                rotate.rotation_center_x.into_pt(dpi),
                rotate.rotation_center_y.into_pt(dpi),
            ));
        }

        if self.translate_x.is_some() || self.translate_y.is_some() {
            transforms.push(CurTransMat::Translate(
                self.translate_x.unwrap_or(Pt(0.0)),
                self.translate_y.unwrap_or(Pt(0.0)),
            ));
        }

        transforms
    }

    /// Combines the transformation matrices produced by `get_ctms` (with no width/height
    /// adjustment) into one final transformation and returns it in SVG's matrix format.
    pub fn as_svg_transform(&self) -> String {
        // Get the list of transformation matrices (using None for the width/height info)
        let ctms = self.get_ctms(None);

        // Start with the identity transformation.
        let mut combined = CurTransMat::Identity;

        // Combine each transform in order.
        for t in ctms {
            // Assume combine_matrix takes two 6-element arrays and returns the product.
            let new_arr = CurTransMat::combine_matrix(combined.as_array(), t.as_array());
            combined = CurTransMat::Raw(new_arr);
        }

        // Get the final matrix as an array.
        let arr = combined.as_array();
        // SVG expects a matrix in the form "matrix(a b c d e f)"
        format!(
            "matrix({} {} {} {} {} {})",
            arr[0], arr[1], arr[2], arr[3], arr[4], arr[5]
        )
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XObjectRotation {
    #[serde(default)]
    pub angle_ccw_degrees: f32,
    #[serde(default)]
    pub rotation_center_x: Px,
    #[serde(default)]
    pub rotation_center_y: Px,
}
