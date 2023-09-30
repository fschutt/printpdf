// clippy lints when serializing PDF strings, in this case its wrong
#![cfg_attr(feature = "cargo-clippy", allow(clippy::string_lit_as_bytes))]

use crate::OffsetDateTime;
#[cfg(feature = "embedded_images")]
use crate::rgba_to_rgb;
use crate::{ColorBits, ColorSpace, CurTransMat, Px};

#[cfg(feature = "embedded_images")]
use image_crate::{DynamicImage, GenericImageView, ImageDecoder, ImageError, ColorType};
use lopdf;
use lopdf::Stream as LoPdfStream;
use std::collections::HashMap;

/* Parent: Resources dictionary of the page */
/// External object that gets reference outside the PDF content stream
/// Gets constructed similar to the `ExtGState`, then inserted into the `/XObject` dictionary
/// on the page. You can instantiate `XObjects` with the `/Do` operator. The `layer.add_xobject()`
/// (or better yet, the `layer.add_image()`, `layer.add_form()`) methods will do this for you.
#[derive(Debug, Clone)]
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
    External(LoPdfStream),
}

impl From<ImageXObject> for XObject {
    fn from(i: ImageXObject) -> XObject {
        XObject::Image(i)
    }
}

impl XObject {
    #[cfg(any(debug_assertions, feature = "less-optimization"))]
    #[inline]
    fn compress_stream(stream: lopdf::Stream) -> lopdf::Stream {
        stream
    }

    #[cfg(all(not(debug_assertions), not(feature = "less-optimization")))]
    #[inline]
    fn compress_stream(mut stream: lopdf::Stream) -> lopdf::Stream {
        let _ = stream.compress();
        stream
    }
}

impl From<XObject> for lopdf::Object {
    fn from(val: XObject) -> Self {
        match val {
            XObject::Image(image) => lopdf::Object::Stream(XObject::compress_stream(image.into())),
            XObject::Form(form) => {
                let cur_form: FormXObject = *form;
                lopdf::Object::Stream(XObject::compress_stream(cur_form.into()))
            }
            XObject::PostScript(ps) => lopdf::Object::Stream(XObject::compress_stream(ps.into())),
            XObject::External(stream) => lopdf::Object::Stream(XObject::compress_stream(stream)),
        }
    }
}

/// List of `XObjects`
#[derive(Debug, Default, Clone)]
pub struct XObjectList {
    objects: HashMap<String, XObject>,
}

impl XObjectList {
    /// Creates a new XObjectList
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new XObject to the list
    pub fn add_xobject(&mut self, xobj: XObject) -> XObjectRef {
        let len = self.objects.len();
        let xobj_ref = XObjectRef::new(len);
        self.objects.insert(xobj_ref.name.clone(), xobj);
        xobj_ref
    }

    /// Same as `Into<lopdf::Dictionary>`, but since the dictionary
    /// items in an XObject dictionary are streams and must be added to
    /// the document as __references__, this function needs an additional
    /// access to the PDF document so that we can add the streams first and
    /// then track the references to them.

    pub fn into_with_document(self, doc: &mut lopdf::Document) -> lopdf::Dictionary {
        self.objects
            .into_iter()
            .map(|(name, object)| {
                let obj: lopdf::Object = object.into();
                let obj_ref = doc.add_object(obj);
                (name, lopdf::Object::Reference(obj_ref))
            })
            .collect()
    }
}

/// Named reference to an `XObject`
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct XObjectRef {
    pub(crate) name: String,
}

impl XObjectRef {
    /// Creates a new reference from a number
    pub fn new(index: usize) -> Self {
        Self {
            name: format!("X{index}"),
        }
    }
}

/* todo: inline images? (icons, logos, etc.) */
/* todo: JPXDecode filter */

#[derive(Debug, Clone)]
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
    pub smask: Option<SMask>,
    /* /BBox << dictionary >> */
    /* todo: find out if this is really required */
    /// Required bounds to clip the image, in unit space
    /// Default value: Identity matrix (`[1 0 0 1 0 0]`) - used when value is `None`
    pub clipping_bbox: Option<CurTransMat>,
}

impl ImageXObject {
    // /// Creates a new ImageXObject
    // pub fn new(
    //     width: Px,
    //     height: Px,
    //     color_space: ColorSpace,
    //     bits: ColorBits,
    //     interpolate: bool,
    //     image_filter: Option<ImageFilter>,
    //     bbox: Option<CurTransMat>,
    //     data: Vec<u8>,
    // ) -> Self {
    //     Self {
    //         width,
    //         height,
    //         color_space,
    //         bits_per_component: bits,
    //         interpolate,
    //         image_data: data,
    //         image_filter,
    //         clipping_bbox: bbox,
    //     }
    // }

    #[cfg(feature = "embedded_images")]
    pub fn try_from<'a, T: ImageDecoder<'a>>(image: T) -> Result<Self, ImageError> {
        use image_crate::error::{LimitError, LimitErrorKind};
        use std::usize;

        let dim = image.dimensions();
        let color_type = image.color_type();
        let num_image_bytes = image.total_bytes();

        if num_image_bytes > usize::MAX as u64 {
            return Err(ImageError::Limits(LimitError::from_kind(
                LimitErrorKind::InsufficientMemory,
            )));
        }

        let mut image_data = vec![0; num_image_bytes as usize];
        image.read_image(&mut image_data)?;

        let (processed_color_type, processed_image_data, smask) = preprocess_image_with_alpha(
            color_type,
            image_data,
            dim
        );

        let color_bits = ColorBits::from(processed_color_type);
        let color_space = ColorSpace::from(processed_color_type);

        Ok(Self {
            width: Px(dim.0 as usize),
            height: Px(dim.1 as usize),
            color_space,
            bits_per_component: color_bits,
            image_data: processed_image_data,
            interpolate: true,
            image_filter: None,
            clipping_bbox: None,
            smask,
        })
    }

    #[cfg(feature = "embedded_images")]
    pub fn from_dynamic_image(image: &DynamicImage) -> Self {
        let dim = image.dimensions();
        let color_type = image.color();
        let data = image.as_bytes().to_vec();

        let (processed_color_type, processed_image_data, smask) = preprocess_image_with_alpha(
            color_type,
            data,
            dim
        );

        let color_bits = ColorBits::from(processed_color_type);
        let color_space = ColorSpace::from(processed_color_type);

        Self {
            width: Px(dim.0 as usize),
            height: Px(dim.1 as usize),
            color_space,
            bits_per_component: color_bits,
            image_data: processed_image_data,
            interpolate: true,
            image_filter: None,
            clipping_bbox: None,
            smask,
        }
    }
}

impl From<ImageXObject> for lopdf::Stream {
    fn from(img: ImageXObject) -> lopdf::Stream {
        use lopdf::Object::*;

        let cs: &'static str = img.color_space.into();
        let bbox: lopdf::Object = img.clipping_bbox.unwrap_or(CurTransMat::Identity).into();

        let smask = if let Some(mask) = img.smask {
            // This is using the "Soft-Mask Images" approach. See page 447 of the adobe PDF 1.4 reference
            XObject::Image(ImageXObject {
                width: img.width,
                height: img.width,
                color_space: ColorSpace::Greyscale,
                bits_per_component: ColorBits::Bit8,
                interpolate: false,
                image_data: mask.matte.into_iter().map(|i| i as u8).collect(),
                image_filter: None,
                smask: None,
                clipping_bbox: None,
            })
            .into()
        } else {
            Null
        };

        let mut dict = lopdf::Dictionary::from_iter(vec![
            ("Type", Name("XObject".as_bytes().to_vec())),
            ("Subtype", Name("Image".as_bytes().to_vec())),
            ("Width", Integer(img.width.0 as i64)),
            ("Height", Integer(img.height.0 as i64)),
            ("Interpolate", img.interpolate.into()),
            ("BitsPerComponent", Integer(img.bits_per_component.into())),
            ("ColorSpace", Name(cs.as_bytes().to_vec())),
            ("SMask", smask),
            ("BBox", bbox),
        ]);

        if let Some(filter) = img.image_filter {
            let params = match filter {
                // TODO technically we could use multiple filters,
                // DCT as an exception!
                ImageFilter::DCT => {
                    vec![
                        ("Filter", Array(vec![Name("DCTDecode".as_bytes().to_vec())])),
                        // not necessary, unless missing in the jpeg header
                        (
                            "DecodeParams",
                            Dictionary(lopdf::dictionary!("ColorTransform" => Integer(0))),
                        ),
                    ]
                }
                _ => unimplemented!("Encountered filter type is not supported"),
            };

            params
                .into_iter()
                .for_each(|param| dict.set(param.0, param.1));
        }

        lopdf::Stream::new(dict, img.image_data)
    }
}

/// Named reference to an image
#[derive(Debug)]
pub struct ImageXObjectRef {
    #[allow(dead_code)]
    name: String,
}

/// Describes the format the image bytes are compressed with.
#[derive(Debug, Copy, Clone)]
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
#[derive(Debug, Clone)]
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

impl From<FormXObject> for lopdf::Stream {
    fn from(val: FormXObject) -> Self {
        use lopdf::Object::*;

        let dict = lopdf::Dictionary::from_iter(vec![
            ("Type", Name("XObject".as_bytes().to_vec())),
            ("Subtype", Name("Form".as_bytes().to_vec())),
            ("FormType", Integer(val.form_type.into())),
        ]);

        lopdf::Stream::new(dict, val.bytes)
    }
}

/*
    <<
        /Type /XObject
        /Subtype /Form
        /FormType 1
        /BBox [ 0 0 1000 1000 ]
        /Matrix [ 1 0 0 1 0 0 ]
        /Resources << /ProcSet [ /PDF ] >>
        /Length 58
    >>
*/

#[derive(Debug, Clone)]
pub struct FormXObjectRef {
    #[allow(dead_code)]
    name: String,
}

#[derive(Debug, Copy, Clone)]
pub enum FormType {
    /// The only form type ever declared by Adobe
    /* Integer(1) */
    Type1,
}

impl From<FormType> for i64 {
    fn from(val: FormType) -> Self {
        match val {
            FormType::Type1 => 1,
        }
    }
}

/*
    see page 341

    /Type /Image1
    /Subtype /Image
    /Width 350
    /Height 200
    /ColorSpace /DeviceRGB                      % required, except for JPXDecode, not allowed for image masks
    /BitsPerComponent 8                         % if ImageMask is true, optional or 1.

                                                % Optional stuff below

    /Intent /RelativeColormetric
    /ImageMask false                            % Mask and ColorSpace should not be specified
    /Mask << >>                                 % Stream or array of colors to be masked
    /Decode [0 1]                               % weird
    /Interpolate true
    /Alternate []                               % array of alternative images
    /SMask << >>                                % (Optional; PDF 1.4) A subsidiary image XObject defining a soft-mask image
                                                % (see “Soft-Mask Images” on page 553) to be used as a source of mask shape
                                                % or mask opacity values in the transparent imaging model. The alpha source
                                                % parameter in the graphics state determines whether the mask values are
                                                % interpreted as shape or opacity.

                                                % If present, this entry overrides the current soft mask in the graphics state,
                                                % as well as the image’s Mask entry, if any. (However, the other
                                                % transparency-related graphics state parameters—blend mode and alpha
                                                % constant—remain in effect.) If SMask is absent, the image has no associated
                                                % soft mask (although the current soft mask in the graphics state may still apply).

    /SMaskInData                                % 0, 1, 2
    /Matte                                      % if present, the SMask must have the same w / h as the picture.
*/

/* Parent: XObject with /Subtype /Image */
/// `SMask` dictionary. A soft mask (or `SMask`) is a greyscale image
/// that is used to mask another image
#[derive(Debug, Clone)]
pub struct SMask {
    /* /Type /XObject */
    /* /Subtype /Image */
    /* /ColorSpace /DeviceGray */

    /* /Intent (is ignored, don't set) */
    /* /ImageMask (fals or not set, is ignored, don't set) */
    /* /Mask (must be absent) */
    /* /SMask (must be absent) */
    /* /Decode (must be [0 1]) */
    /* /Alternates (ignored, don't set) */
    /* /Name (ignored, don't set) */
    /* /StructParent (ignored, don't set) */
    /* /ID (ignored, don't set) */
    /* /OPI (ignored, don't set) */
    /// If `self.matte` is set to true, this entry must be the same
    /// width as the parent image. If not, the `SMask` is resampled to the parent unit square
    pub width: i64,
    /// See width
    pub height: i64,
    /* /Interpolate (optional, set to true)*/
    pub interpolate: bool,
    /// Bits per component, required (warning: this is a grayscale image)
    pub bits_per_component: i64,
    /// Vec of component values
    pub matte: Vec<i64>,
}

// in the PDF content stream, reference an XObject like this

/*
    q                                           % Save graphics state
    1 0 0 1 100 200 cm                          % Translate
    0. 7071 0. 7071 −0. 7071 0. 7071 0 0 cm     % Rotate
    150 0 0 80 0 0 cm                           % Scale
    /Image1 Do                                  % Paint image
    Q                                           % Restore graphics state
*/

#[derive(Debug, Copy, Clone)]
pub struct GroupXObject {
    /* /Type /Group */
    /* /S /Transparency */ /* currently the only valid GroupXObject */
}

#[derive(Debug, Copy, Clone)]
pub enum GroupXObjectType {
    /// Transparency group XObject
    TransparencyGroup,
}

/// PDF 1.4 and higher
/// Contains a PDF file to be embedded in the current PDF
#[derive(Debug)]
pub struct ReferenceXObject {
    /// (Required) The file containing the target document. (?)
    pub file: Vec<u8>,
    /// Page number to embed
    pub page: i64,
    /// Optional, should be the document ID and version ID from the metadata
    pub id: [i64; 2],
}

/* parent: Catalog dictionary, I think, not sure */
/// Optional content group, for PDF layers. Only available in PDF 1.4
/// but (I think) lower versions of PDF allow this, too. Used to create
/// Adobe Illustrator-like layers in PDF
#[derive(Debug)]
pub struct OptionalContentGroup {
    /* /Type /OCG */
    /* /Name (Layer 1) */
    /// (Required) The name of the optional content group, suitable for
    /// presentation in a viewer application’s user interface.
    pub name: String,
    /* /Intent [/View /Design] */
    /// (Optional) A single intent name or an array containing any
    /// combination of names. PDF 1.5 defines two names, View and Design,
    /// that indicate the intended use of the graphics in the group.
    /// Future versions may define others. A processing application can choose
    /// to use only groups that have a specific intent and ignore others.
    /// Default value: View. See “Intent” on page 368 for more information.
    pub intent: Vec<OCGIntent>,
    /* /Usage << dictionary >> */
    /// (Optional) A usage dictionary describing the nature of the content controlled
    /// by the group. It may be used by features that automatically control the state
    /// of the group based on outside factors. See “Usage and Usage Application
    /// Dictionaries” on page 380 for more information.
    pub usage: Option<lopdf::Dictionary>,
}

/// Intent to use for the optional content groups
#[derive(Debug, Copy, Clone)]
pub enum OCGIntent {
    View,
    Design,
}

/// TODO, very low priority
#[derive(Debug, Clone)]
pub struct PostScriptXObject {
    /// __(Optional)__ A stream whose contents are to be used in
    /// place of the PostScript XObject’s stream when the target
    /// PostScript interpreter is known to support only LanguageLevel 1
    #[allow(dead_code)]
    level1: Option<Vec<u8>>,
}

impl From<PostScriptXObject> for lopdf::Stream {
    fn from(_val: PostScriptXObject) -> Self {
        // todo!
        lopdf::Stream::new(lopdf::Dictionary::new(), Vec::new())
    }
}

#[cfg(feature = "embedded_images")]
fn preprocess_image_with_alpha(color_type: ColorType, image_data: Vec<u8>, dim: (u32, u32)) -> (ColorType, Vec<u8>, Option<SMask>) {
    match color_type {
        image_crate::ColorType::Rgba8 => {
            let (rgb, alpha) = rgba_to_rgb(image_data);
            let smask = Some(SMask {
                bits_per_component: 8,
                width: dim.0 as i64,
                height: dim.1 as i64,
                interpolate: false,
                matte: alpha.into_iter().map(|b| b as i64).collect(),
            });
            (ColorType::Rgb8, rgb, smask)
        },
        _ => (color_type, image_data, None)
    }
}