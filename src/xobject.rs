//! XObjects: bitmaps and blocks of vector graphics that a page draws with the `Do` operator.
//!
//! A page's content stream only holds drawing operators. Anything bigger, such as a bitmap, or
//! vector graphics that appear several times, is stored once as an *XObject* ("external object":
//! a stream plus a dictionary) and listed in the page's `/Resources`. The content stream then
//! names it and paints it with `Do` (ISO 32000-1, 8.8).
//!
//! An [`XObject`] is one of three things:
//!
//! - [`XObject::Image`]: a bitmap, held as a [`RawImage`] (pixels, width, height, pixel layout).
//!   See the [`image_types`](crate::image_types) module.
//! - [`XObject::Form`]: a [`FormXObject`], a content stream of its own that can be painted any
//!   number of times. It has nothing to do with interactive PDF forms.
//! - [`XObject::External`]: an [`ExternalXObject`], any other stream, kept as a plain dictionary
//!   plus bytes. SVG graphics (`Svg::parse`, `svg` feature) and every XObject read from an
//!   existing PDF that is not a decodable bitmap end up here.
//!
//! # Using one
//!
//! Register the object with [`PdfDocument::add_image`](crate::PdfDocument::add_image) or
//! [`PdfDocument::add_xobject`](crate::PdfDocument::add_xobject). Both return an [`XObjectId`](crate::XObjectId).
//! Paint it with [`Op::UseXobject`] and an [`XObjectTransform`] that says
//! where it goes. A complete example is on [`RawImage`].
//!
//! # Size and units
//!
//! A PDF paints an XObject into a 1 by 1 unit square at the origin of the current coordinate
//! system. When an XObject has a size in pixels ([`XObject::get_width_height`]),
//! [`XObjectTransform`] first scales the unit square to that size converted to points at its
//! `dpi` (300 when unset, so 1 pixel is 0.24 pt). Positions are in [`Pt`] from the bottom left
//! corner of the page.

use std::collections::BTreeMap;

use lopdf::StringFormat;
use serde_derive::{Deserialize, Serialize};

use crate::{
    date::OffsetDateTime,
    deserialize::PageState,
    image_types::{RawImage, ImageOptimizationOptions},
    matrix::CurTransMat,
    units::{Pt, Px},
    Op,
};

/// An object that lives outside the content stream and is painted with `Do`
/// (ISO 32000-1, 8.8). The document keeps them in [`PdfResources`](crate::PdfResources).
///
/// Serialized as `{"type": "image" | "form" | "external", "data": ...}`.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum XObject {
    /// A bitmap (`/Subtype /Image`, ISO 32000-1, 8.9). Encoded when the document is saved.
    Image(RawImage),
    /// Reusable vector content (`/Subtype /Form`, ISO 32000-1, 8.10). Not an interactive form.
    Form(FormXObject),
    /// A stream kept as it is, for XObjects this crate does not model, such as SVG graphics.
    External(ExternalXObject),
}

impl XObject {
    /// Width and height in pixels, if the XObject has them: always for an image, when set for
    /// a form or an external XObject. [`XObjectTransform`] uses it to scale the unit square.
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

// Adds the XObject to `doc` as an indirect object and returns its id.
pub(crate) fn add_xobject_to_document(
    xobj: &XObject,
    doc: &mut lopdf::Document,
    _image_opts: Option<&ImageOptimizationOptions>,
) -> lopdf::ObjectId {
    match xobj {
        XObject::Image(_i) => {
            #[cfg(feature = "images")]
            {
                let stream = crate::image::image_to_stream(_i.clone(), doc, _image_opts);
                doc.add_object(stream)
            }
            #[cfg(not(feature = "images"))]
            {
                // Raw pixels with FlateDecode need no image codec.
                let stream = crate::image_types::raw_image_to_basic_stream(_i, doc);
                doc.add_object(stream)
            }
        }
        XObject::Form(f) => {
            let stream = form_xobject_to_stream(f, doc);
            doc.add_object(stream)
        }
        XObject::External(external_xobject) => {
            // Streams nested in the dictionary (functions, ICC profiles, soft masks, ...)
            // become indirect objects, as PDF requires (ISO 32000-1, 7.3.8.1).
            let stream = external_xobject
                .stream
                .into_lopdf_with_indirect_streams(doc);
            doc.add_object(stream)
        }
    }
}

/// An XObject kept as a raw stream: a dictionary plus bytes, painted with `Do`.
///
/// Made by `Svg::parse` (`svg` feature) and when reading a PDF. The `stream` dictionary should
/// carry `/Type /XObject` and `/Subtype`, and for a form also `/BBox`.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalXObject {
    /// The dictionary and content of the stream.
    pub stream: ExternalStream,
    /// Width in pixels. With `height`, the size [`XObjectTransform`] scales the unit square to.
    #[serde(default)]
    pub width: Option<Px>,
    /// Height in pixels.
    #[serde(default)]
    pub height: Option<Px>,
    /// Resolution the pixel size refers to, in dots per inch. Informational: placing uses the
    /// `dpi` of the [`XObjectTransform`], not this one.
    #[serde(default)]
    pub dpi: Option<f32>,
}

/// A PDF stream: a dictionary and bytes.
#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalStream {
    /// The stream dictionary, keyed by name without the leading `/`.
    pub dict: BTreeMap<String, DictItem>,
    /// The bytes as they are stored. When the dictionary has a `/Filter`, they are encoded.
    pub content: Vec<u8>,
    /// Whether the writer may compress `content` (set false when it is already encoded).
    pub compress: bool,
}

impl ExternalStream {
    pub(crate) fn into_lopdf(&self) -> lopdf::Stream {
        lopdf::Stream::new(build_dict(&self.dict), self.content.clone())
            .with_compression(self.compress)
    }

    /// Like `into_lopdf`, but a [`DictItem::Stream`] nested at any depth in the dictionary is
    /// added to `doc` as its own object and replaced by a reference (ISO 32000-1, 7.3.8.1).
    pub(crate) fn into_lopdf_with_indirect_streams(
        &self,
        doc: &mut lopdf::Document,
    ) -> lopdf::Stream {
        let dict = lopdf::Dictionary::from_iter(
            self.dict
                .iter()
                .map(|(k, v)| (k.clone(), v.to_lopdf_with_indirect_streams(doc))),
        );
        lopdf::Stream::new(dict, self.content.clone()).with_compression(self.compress)
    }

    /// The content with its `/Filter`s decoded, or the stored bytes when they cannot be decoded.
    pub fn decompressed_content(&self) -> Vec<u8> {
        self.into_lopdf()
            .decompressed_content()
            .unwrap_or(self.content.clone())
    }

    /// Parses the text of a content stream into [`Op`]s.
    ///
    /// # Errors
    ///
    /// Returns a message when the text is not a valid content stream.
    pub fn decode_ops(s: &str) -> Result<Vec<Op>, String> {
        Self::get_ops_internal(s.as_bytes())
    }

    /// Decodes the content (see [`ExternalStream::decompressed_content`]) and parses it into
    /// [`Op`]s.
    ///
    /// # Errors
    ///
    /// Returns a message when the content is not a valid content stream.
    pub fn get_ops(&self) -> Result<Vec<Op>, String> {
        Self::get_ops_internal(&self.decompressed_content())
    }

    fn get_ops_internal(s: &[u8]) -> Result<Vec<Op>, String> {
        let content = lopdf::content::Content::decode(s)
            .map_err(|e| format!("Failed to decode content stream: {}", e))?;

        let mut page_state = PageState::default();
        let mut printpdf_ops = Vec::new();

        for (op_id, op) in content.operations.iter().enumerate() {
            let parsed_op = crate::deserialize::parse_op(
                0,
                op_id,
                op,
                &mut page_state,
                &BTreeMap::new(),
                &mut Vec::new(),
            )?;
            printpdf_ops.extend(parsed_op);
        }

        Ok(printpdf_ops)
    }
}

/// A PDF object inside a stream dictionary: a simplified, serializable copy of
/// [`lopdf::Object`].
///
/// Serialized as `{"type": "<variant in kebab-case>", "data": ...}`.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum DictItem {
    /// An array, `[ ... ]`.
    Array(Vec<DictItem>),
    /// A string. `literal` selects the `( ... )` form over the hexadecimal `< ... >` form.
    String {
        /// The bytes of the string.
        data: Vec<u8>,
        /// True for the literal form, false for hexadecimal.
        literal: bool,
    },
    /// Bytes, written as a hexadecimal string.
    Bytes(Vec<u8>),
    /// `true` or `false`.
    Bool(bool),
    /// A real number (written as a PDF real, like [`DictItem::Real`]).
    Float(f32),
    /// An integer.
    Int(i64),
    /// A real number.
    Real(f32),
    /// A name, `/Name`, as bytes without the slash.
    Name(Vec<u8>),
    /// An indirect reference `obj gen R`.
    Ref {
        /// Object number.
        obj: u32,
        /// Generation number.
        gen: u16,
    },
    /// A dictionary, `<< ... >>`.
    Dict {
        /// The entries, keyed by name without the slash.
        map: BTreeMap<String, DictItem>,
    },
    /// A stream with its own dictionary.
    Stream {
        /// The stream.
        stream: ExternalStream,
    },
    /// `null`.
    Null,
}

impl DictItem {
    /// Converts to a [`lopdf::Object`]. A nested [`DictItem::Stream`] stays an inline stream.
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

    /// Like [`DictItem::to_lopdf`], but nested streams become indirect objects of `doc`
    /// and are replaced by references (ISO 32000-1, 7.3.8.1).
    pub(crate) fn to_lopdf_with_indirect_streams(
        &self,
        doc: &mut lopdf::Document,
    ) -> lopdf::Object {
        use lopdf::Object;
        match self {
            DictItem::Array(items) => Object::Array(
                items
                    .iter()
                    .map(|item| item.to_lopdf_with_indirect_streams(doc))
                    .collect(),
            ),
            DictItem::Dict { map } => Object::Dictionary(lopdf::Dictionary::from_iter(
                map.iter().map(|(k, v)| {
                    (k.as_bytes().to_vec(), v.to_lopdf_with_indirect_streams(doc))
                }),
            )),
            DictItem::Stream { stream } => {
                let stream_obj = stream.into_lopdf_with_indirect_streams(doc);
                Object::Reference(doc.add_object(stream_obj))
            }
            other => other.to_lopdf(),
        }
    }

    /// Converts a [`lopdf::Object`]. References stay references, they are not resolved.
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
                            String::from_utf8_lossy(s.0).to_string(),
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
                                String::from_utf8_lossy(s.0).to_string(),
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

/// Filters (ISO 32000-1, 7.4) that can encode image bytes. Not used by the crate itself; see
/// [`ImageCompression`](crate::ImageCompression) to choose how images are written.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ImageFilter {
    /// `ASCII85Decode`: binary as printable ASCII, no compression.
    Ascii85,
    /// `LZWDecode`: lossless Lempel-Ziv-Welch compression.
    Lzw,
    /// `DCTDecode`: lossy JPEG (baseline).
    DCT,
    /// `JPXDecode`: lossy or lossless JPEG 2000.
    JPX,
}

/// Reusable vector content with its own content stream, resources and coordinate system
/// (ISO 32000-1, 8.10). Paint it with [`Op::UseXobject`].
///
/// This is not an interactive PDF form (AcroForm). Fields documented as "written" end up in the
/// stream dictionary when the document is saved.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormXObject {
    /// `/FormType`. [`FormType::Type1`] is the only form type there is.
    pub form_type: FormType,
    /// Width and height in pixels. Not written: [`XObjectTransform`] uses it to scale the
    /// unit square to the size of the form (see [`XObject::get_width_height`]).
    pub size: Option<(Px, Px)>,
    /// The content stream: PDF operators as bytes, compressed when the document is saved.
    pub bytes: Vec<u8>,
    /// `/Matrix`: maps the coordinates of the form into the space it is painted in
    /// (ISO 32000-1, 8.10.2).
    pub matrix: Option<CurTransMat>,
    /// `/Resources`: the fonts, images, graphics states and so on that `bytes` names.
    /// Nested streams are written as indirect objects (ISO 32000-1, 7.8.3).
    pub resources: Option<BTreeMap<String, DictItem>>,
    /// `/Group`: treats the form as a transparency group (ISO 32000-1, 11.6.6).
    pub group: Option<GroupXObject>,
    /// `/Ref`: marks the form as a stand-in for a page of another PDF file
    /// (ISO 32000-1, 8.10.4).
    pub ref_dict: Option<BTreeMap<String, DictItem>>,
    /// `/Metadata` (ISO 32000-1, 14.3.2). Written as an indirect dictionary object.
    pub metadata: Option<BTreeMap<String, DictItem>>,
    /// `/PieceInfo`: data of the application that made the form (ISO 32000-1, 14.5).
    pub piece_info: Option<BTreeMap<String, DictItem>>,
    /// `/LastModified`: when the form was last changed. Needed when `piece_info` is set
    /// (ISO 32000-1, 14.5).
    pub last_modified: Option<OffsetDateTime>,
    /// `/StructParent`: key of the form in the structure parent tree, when the whole form is
    /// one structure content item (ISO 32000-1, 14.7.4.4). Not written when `struct_parents`
    /// is set.
    pub struct_parent: Option<i64>,
    /// `/StructParents`: key of the form in the structure parent tree, when it contains
    /// marked-content sequences that are content items (ISO 32000-1, 14.7.4.4). Takes
    /// precedence over `struct_parent`: a form has at most one of the two.
    pub struct_parents: Option<i64>,
    /// `/OPI`: Open Prepress Interface dictionary (ISO 32000-1, 14.11.7).
    pub opi: Option<BTreeMap<String, DictItem>>,
    /// `/OC`: optional content group or membership dictionary. A form that is hidden by it is
    /// skipped as if its `Do` were absent (ISO 32000-1, 8.11).
    pub oc: Option<BTreeMap<String, DictItem>>,
    /// `/Name`: the name the form is listed under in the resources. Obsolete since PDF 1.1
    /// (ISO 32000-1, 8.10.2), written as a string.
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
        // Resources may contain streams (patterns, shadings, nested XObjects), which must be
        // written as indirect objects.
        let res_dict = lopdf::Dictionary::from_iter(
            res.iter()
                .map(|(k, v)| (k.clone(), v.to_lopdf_with_indirect_streams(doc))),
        );
        dict.set("Resources", res_dict);
    }

    if let Some(g) = f.group.as_ref() {
        let group_dict = lopdf::Dictionary::from_iter(vec![
            ("Type", Name("Group".into())),
            ("S", Name(g.group_type.get_id().into())),
        ]);

        dict.set("Group", Dictionary(group_dict));
    }

    if let Some(r) = f.ref_dict.as_ref() {
        dict.set("Ref", build_dict(r));
    }

    if let Some(r) = f.metadata.as_ref() {
        dict.set("Metadata", doc.add_object(build_dict(r)));
    }

    if let Some(r) = f.piece_info.as_ref() {
        dict.set("PieceInfo", doc.add_object(build_dict(r)));
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
        dict.set("OPI", build_dict(r));
    }

    if let Some(r) = f.oc.as_ref() {
        dict.set("OC", build_dict(r));
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

    let stream = lopdf::Stream::new(dict, f.bytes.clone()).with_compression(true);
    stream
}

/// Converts a map of [`DictItem`]s to a [`lopdf::Dictionary`]. Nested streams stay inline.
pub fn build_dict(r: &BTreeMap<String, DictItem>) -> lopdf::Dictionary {
    lopdf::Dictionary::from_iter(r.iter().map(|(k, v)| (k.clone(), v.to_lopdf())))
}

/// The `/FormType` of a form XObject (ISO 32000-1, 8.10.2).
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormType {
    /// The only form type defined.
    Type1,
}

impl FormType {
    fn get_id(&self) -> &'static str {
        match self {
            FormType::Type1 => "Type1",
        }
    }
}

/// The `/Group` dictionary of a form: the attributes of a group of objects that are
/// composited together (ISO 32000-1, 8.10.3 and 11.6.6).
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupXObject {
    /// The kind of group, written as `/S`.
    #[serde(default)]
    pub group_type: GroupXObjectType,
}

/// The group subtype of a [`GroupXObject`].
#[derive(Debug, Default, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupXObjectType {
    /// A transparency group (`/S /Transparency`), the only group type there is.
    #[default]
    TransparencyGroup,
}

impl GroupXObjectType {
    /// The PDF name of the group type, without the slash.
    pub fn get_id(&self) -> &'static str {
        match self {
            GroupXObjectType::TransparencyGroup => "Transparency",
        }
    }
}

/// A reference to a page of another PDF file (ISO 32000-1, 8.10.4). Not used by the crate.
#[derive(Debug, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct ReferenceXObject {
    /// The target document.
    pub file: Vec<u8>,
    /// The page of the target document.
    pub page: i64,
    /// Document id and version id of the target, taken from its metadata.
    pub id: [i64; 2],
}

/// A PostScript XObject (ISO 32000-1, 8.8.2). Not used by the crate.
#[derive(Debug, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct PostScriptXObject {
    /// Content for interpreters that only support PostScript level 1.
    #[allow(dead_code)]
    pub level1: Option<Vec<u8>>,
}

/// Where and how large an XObject is painted. All fields are optional; the default paints it at
/// the bottom left corner of the page, at its pixel size at 300 dpi.
///
/// The matrices are applied in this order: scale the unit square to the pixel size of the
/// XObject (at `dpi`), `scale_x` and `scale_y`, rotate around `rotate`'s center, then move by
/// `translate_x` and `translate_y`. Only the XObject is affected, the rest of the page is not.
///
/// # Examples
///
/// ```
/// use printpdf::*;
///
/// // A 64 x 64 pixel grey image, painted 128 pt wide, 200 pt from the left and bottom edge.
/// let image = RawImage {
///     pixels: RawImageData::U8(vec![128; 64 * 64]),
///     width: 64,
///     height: 64,
///     data_format: RawImageFormat::R8,
///     tag: Vec::new(),
/// };
/// let mut doc = PdfDocument::new("Placing an image");
/// let id = doc.add_image(&image);
/// let op = Op::UseXobject {
///     id,
///     transform: XObjectTransform {
///         dpi: Some(72.0), // at 72 dpi, 1 pixel is 1 pt
///         scale_x: Some(2.0),
///         scale_y: Some(2.0),
///         translate_x: Some(Pt(200.0)),
///         translate_y: Some(Pt(200.0)),
///         ..Default::default()
///     },
/// };
/// let page = PdfPage::new(Mm(210.0), Mm(297.0), vec![op]);
/// let bytes = doc.with_pages(vec![page]).save(&PdfSaveOptions::default(), &mut Vec::new());
/// assert!(bytes.starts_with(b"%PDF"));
/// ```
#[derive(Debug, Copy, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct XObjectTransform {
    /// Distance to the right, in points, from the origin of the current coordinate system
    /// (the left edge of the page). Applied last.
    #[serde(default)]
    pub translate_x: Option<Pt>,
    /// Distance upwards, in points, from the origin (the bottom edge of the page). Applied last.
    #[serde(default)]
    pub translate_y: Option<Pt>,
    /// Rotation, counter-clockwise, around a point of the XObject.
    #[serde(default)]
    pub rotate: Option<XObjectRotation>,
    /// Horizontal scale factor on top of the size scale, 1.0 for none.
    #[serde(default)]
    pub scale_x: Option<f32>,
    /// Vertical scale factor on top of the size scale, 1.0 for none.
    #[serde(default)]
    pub scale_y: Option<f32>,
    /// Dots per inch that turn the pixel size of the XObject into points: 1 pixel is
    /// `72 / dpi` pt. Also converts the rotation center. 300 when unset.
    #[serde(default)]
    pub dpi: Option<f32>,
    /// Skips the scale from the unit square to the pixel size. Set it for content that already
    /// places itself with its own `cm` operators, such as XObjects read from a PDF.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_auto_scale: bool,
}

impl XObjectTransform {
    /// The matrices that place the XObject, in the order they apply (see the type docs).
    ///
    /// `wh` is the pixel size of the XObject ([`XObject::get_width_height`]); with `None`, the
    /// scale from the unit square to the pixel size is left out.
    pub fn get_ctms(&self, wh: Option<(Px, Px)>) -> Vec<CurTransMat> {
        let mut transforms = Vec::new();
        let dpi = self.dpi.unwrap_or(300.0);

        if let Some((w, h)) = wh {
            // PDF paints an image into a 1x1 square, so scale it to its size in points.
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

    /// The matrices of [`XObjectTransform::get_ctms`] without the size scale, combined into one
    /// SVG `matrix(a b c d e f)` string.
    pub fn as_svg_transform(&self) -> String {
        let ctms = self.get_ctms(None);
        let mut combined = CurTransMat::Identity;

        for t in ctms {
            let new_arr = CurTransMat::combine_matrix(combined.as_array(), t.as_array());
            combined = CurTransMat::Raw(new_arr);
        }

        let arr = combined.as_array();
        format!(
            "matrix({} {} {} {} {} {})",
            arr[0], arr[1], arr[2], arr[3], arr[4], arr[5]
        )
    }
}

/// A rotation of an XObject around a center point.
#[derive(Debug, Copy, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XObjectRotation {
    /// Angle in degrees, counter-clockwise.
    #[serde(default)]
    pub angle_ccw_degrees: f32,
    /// Horizontal position of the center in pixels, measured from the left edge of the
    /// XObject and converted to points with the `dpi` of the [`XObjectTransform`].
    #[serde(default)]
    pub rotation_center_x: Px,
    /// Vertical position of the center in pixels, measured from the bottom edge of the XObject.
    #[serde(default)]
    pub rotation_center_y: Px,
}
