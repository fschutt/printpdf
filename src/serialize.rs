use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};

use lopdf::{
    content::Operation as LoOp,
    Dictionary as LoDictionary,
    Object::{Array, Dictionary, Integer, Name, Null, Real, Reference, Stream, String as LoString},
    Stream as LoStream,
    StringFormat::{Hexadecimal, Literal},
};
use serde_derive::{Deserialize, Serialize};

use crate::{
    Actions, BuiltinFont, Color, ColorArray, Destination, FontId, IccProfileType, ImageOptimizationOptions, Line, LinkAnnotation, Op, PaintMode, ParsedFont, ParsedSubsetFont, PdfDocument, PdfDocumentInfo, PdfPage, PdfResources, PdfWarnMsg, Polygon, PrepFont, TextItem, XObject, XObjectId, color::IccProfile, font::{FontType, SubsetFont}
};

#[allow(non_upper_case_globals, dead_code)]
struct Glyph;
impl Glyph {
    pub const A: u16 = 0x0041;
    pub const AE: u16 = 0x00c6;
    pub const Aacute: u16 = 0x00c1;
    pub const Acircumflex: u16 = 0x00c2;
    pub const Adieresis: u16 = 0x00c4;
    pub const Agrave: u16 = 0x00c0;
    pub const Aring: u16 = 0x00c5;
    pub const Atilde: u16 = 0x00c3;
    pub const B: u16 = 0x0042;
    pub const C: u16 = 0x0043;
    pub const Ccedilla: u16 = 0x00c7;
    pub const D: u16 = 0x0044;
    pub const E: u16 = 0x0045;
    pub const Eacute: u16 = 0x00c9;
    pub const Ecircumflex: u16 = 0x00ca;
    pub const Edieresis: u16 = 0x00cb;
    pub const Egrave: u16 = 0x00c8;
    pub const Eth: u16 = 0x00d0;
    pub const Euro: u16 = 0x20ac;
    pub const F: u16 = 0x0046;
    pub const G: u16 = 0x0047;
    pub const H: u16 = 0x0048;
    pub const I: u16 = 0x0049;
    pub const Iacute: u16 = 0x00cd;
    pub const Icircumflex: u16 = 0x00ce;
    pub const Idieresis: u16 = 0x00cf;
    pub const Igrave: u16 = 0x00cc;
    pub const J: u16 = 0x004a;
    pub const K: u16 = 0x004b;
    pub const L: u16 = 0x004c;
    pub const M: u16 = 0x004d;
    pub const N: u16 = 0x004e;
    pub const Ntilde: u16 = 0x00d1;
    pub const O: u16 = 0x004f;
    pub const OE: u16 = 0x0152;
    pub const Oacute: u16 = 0x00d3;
    pub const Ocircumflex: u16 = 0x00d4;
    pub const Odieresis: u16 = 0x00d6;
    pub const Ograve: u16 = 0x00d2;
    pub const Oslash: u16 = 0x00d8;
    pub const Otilde: u16 = 0x00d5;
    pub const P: u16 = 0x0050;
    pub const Q: u16 = 0x0051;
    pub const R: u16 = 0x0052;
    pub const S: u16 = 0x0053;
    pub const Scaron: u16 = 0x0160;
    pub const T: u16 = 0x0054;
    pub const Thorn: u16 = 0x00de;
    pub const U: u16 = 0x0055;
    pub const Uacute: u16 = 0x00da;
    pub const Ucircumflex: u16 = 0x00db;
    pub const Udieresis: u16 = 0x00dc;
    pub const Ugrave: u16 = 0x00d9;
    pub const V: u16 = 0x0056;
    pub const W: u16 = 0x0057;
    pub const X: u16 = 0x0058;
    pub const Y: u16 = 0x0059;
    pub const Yacute: u16 = 0x00dd;
    pub const Ydieresis: u16 = 0x0178;
    pub const Z: u16 = 0x005a;
    pub const Zcaron: u16 = 0x017d;
    pub const a: u16 = 0x0061;
    pub const aacute: u16 = 0x00e1;
    pub const acircumflex: u16 = 0x00e2;
    pub const acute: u16 = 0x00b4;
    pub const adieresis: u16 = 0x00e4;
    pub const ae: u16 = 0x00e6;
    pub const agrave: u16 = 0x00e0;
    pub const ampersand: u16 = 0x0026;
    pub const aring: u16 = 0x00e5;
    pub const asciicircum: u16 = 0x005e;
    pub const asciitilde: u16 = 0x007e;
    pub const asterisk: u16 = 0x002a;
    pub const at: u16 = 0x0040;
    pub const atilde: u16 = 0x00e3;
    pub const b: u16 = 0x0062;
    pub const backslash: u16 = 0x005c;
    pub const bar: u16 = 0x007c;
    pub const braceleft: u16 = 0x007b;
    pub const braceright: u16 = 0x007d;
    pub const bracketleft: u16 = 0x005b;
    pub const bracketright: u16 = 0x005d;
    pub const brokenbar: u16 = 0x00a6;
    pub const bullet: u16 = 0x2022;
    pub const c: u16 = 0x0063;
    pub const ccedilla: u16 = 0x00e7;
    pub const cedilla: u16 = 0x00b8;
    pub const cent: u16 = 0x00a2;
    pub const circumflex: u16 = 0x02c6;
    pub const colon: u16 = 0x003a;
    pub const comma: u16 = 0x002c;
    pub const copyright: u16 = 0x00a9;
    pub const currency: u16 = 0x00a4;
    pub const d: u16 = 0x0064;
    pub const dagger: u16 = 0x2020;
    pub const daggerdbl: u16 = 0x2021;
    pub const degree: u16 = 0x00b0;
    pub const dieresis: u16 = 0x00a8;
    pub const divide: u16 = 0x00f7;
    pub const dollar: u16 = 0x0024;
    pub const e: u16 = 0x0065;
    pub const eacute: u16 = 0x00e9;
    pub const ecircumflex: u16 = 0x00ea;
    pub const edieresis: u16 = 0x00eb;
    pub const egrave: u16 = 0x00e8;
    pub const eight: u16 = 0x0038;
    pub const ellipsis: u16 = 0x2026;
    pub const emdash: u16 = 0x2014;
    pub const endash: u16 = 0x2013;
    pub const equal: u16 = 0x003d;
    pub const eth: u16 = 0x00f0;
    pub const exclam: u16 = 0x0021;
    pub const exclamdown: u16 = 0x00a1;
    pub const f: u16 = 0x0066;
    pub const five: u16 = 0x0035;
    pub const florin: u16 = 0x0192;
    pub const four: u16 = 0x0034;
    pub const g: u16 = 0x0067;
    pub const germandbls: u16 = 0x00df;
    pub const grave: u16 = 0x0060;
    pub const greater: u16 = 0x003e;
    pub const guillemotleft: u16 = 0x00ab;
    pub const guillemotright: u16 = 0x00bb;
    pub const guilsinglleft: u16 = 0x2039;
    pub const guilsinglright: u16 = 0x203a;
    pub const h: u16 = 0x0068;
    pub const hyphen: u16 = 0x002d;
    pub const i: u16 = 0x0069;
    pub const iacute: u16 = 0x00ed;
    pub const icircumflex: u16 = 0x00ee;
    pub const idieresis: u16 = 0x00ef;
    pub const igrave: u16 = 0x00ec;
    pub const j: u16 = 0x006a;
    pub const k: u16 = 0x006b;
    pub const l: u16 = 0x006c;
    pub const less: u16 = 0x003c;
    pub const logicalnot: u16 = 0x00ac;
    pub const m: u16 = 0x006d;
    pub const macron: u16 = 0x00af;
    pub const mu: u16 = 0x00b5;
    pub const multiply: u16 = 0x00d7;
    pub const n: u16 = 0x006e;
    pub const nine: u16 = 0x0039;
    pub const ntilde: u16 = 0x00f1;
    pub const numbersign: u16 = 0x0023;
    pub const o: u16 = 0x006f;
    pub const oacute: u16 = 0x00f3;
    pub const ocircumflex: u16 = 0x00f4;
    pub const odieresis: u16 = 0x00f6;
    pub const oe: u16 = 0x0153;
    pub const ograve: u16 = 0x00f2;
    pub const one: u16 = 0x0031;
    pub const onehalf: u16 = 0x00bd;
    pub const onequarter: u16 = 0x00bc;
    pub const onesuperior: u16 = 0x00b9;
    pub const ordfeminine: u16 = 0x00aa;
    pub const ordmasculine: u16 = 0x00ba;
    pub const oslash: u16 = 0x00f8;
    pub const otilde: u16 = 0x00f5;
    pub const p: u16 = 0x0070;
    pub const paragraph: u16 = 0x00b6;
    pub const parenleft: u16 = 0x0028;
    pub const parenright: u16 = 0x0029;
    pub const percent: u16 = 0x0025;
    pub const period: u16 = 0x002e;
    pub const periodcentered: u16 = 0x00b7;
    pub const perthousand: u16 = 0x2030;
    pub const plus: u16 = 0x002b;
    pub const plusminus: u16 = 0x00b1;
    pub const q: u16 = 0x0071;
    pub const question: u16 = 0x003f;
    pub const questiondown: u16 = 0x00bf;
    pub const quotedbl: u16 = 0x0022;
    pub const quotedblbase: u16 = 0x201e;
    pub const quotedblleft: u16 = 0x201c;
    pub const quotedblright: u16 = 0x201d;
    pub const quoteleft: u16 = 0x2018;
    pub const quoteright: u16 = 0x2019;
    pub const quotesinglbase: u16 = 0x201a;
    pub const quotesingle: u16 = 0x0027;
    pub const r: u16 = 0x0072;
    pub const registered: u16 = 0x00ae;
    pub const s: u16 = 0x0073;
    pub const scaron: u16 = 0x0161;
    pub const section: u16 = 0x00a7;
    pub const semicolon: u16 = 0x003b;
    pub const seven: u16 = 0x0037;
    pub const six: u16 = 0x0036;
    pub const slash: u16 = 0x002f;
    pub const space: u16 = 0x0020;
    pub const sterling: u16 = 0x00a3;
    pub const t: u16 = 0x0074;
    pub const thorn: u16 = 0x00fe;
    pub const three: u16 = 0x0033;
    pub const threequarters: u16 = 0x00be;
    pub const threesuperior: u16 = 0x00b3;
    pub const tilde: u16 = 0x02dc;
    pub const trademark: u16 = 0x2122;
    pub const two: u16 = 0x0032;
    pub const twosuperior: u16 = 0x00b2;
    pub const u: u16 = 0x0075;
    pub const uacute: u16 = 0x00fa;
    pub const ucircumflex: u16 = 0x00fb;
    pub const udieresis: u16 = 0x00fc;
    pub const ugrave: u16 = 0x00f9;
    pub const underscore: u16 = 0x005f;
    pub const v: u16 = 0x0076;
    pub const w: u16 = 0x0077;
    pub const x: u16 = 0x0078;
    pub const y: u16 = 0x0079;
    pub const yacute: u16 = 0x00fd;
    pub const ydieresis: u16 = 0x00ff;
    pub const yen: u16 = 0x00a5;
    pub const z: u16 = 0x007a;
    pub const zcaron: u16 = 0x017e;
    pub const zero: u16 = 0x0030;
}

type CodedCharacterSet = [Option<u16>; 256];
const WIN_ANSI_ENCODING: CodedCharacterSet = [
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Glyph::space),
    Some(Glyph::exclam),
    Some(Glyph::quotedbl),
    Some(Glyph::numbersign),
    Some(Glyph::dollar),
    Some(Glyph::percent),
    Some(Glyph::ampersand),
    Some(Glyph::quotesingle),
    Some(Glyph::parenleft),
    Some(Glyph::parenright),
    Some(Glyph::asterisk),
    Some(Glyph::plus),
    Some(Glyph::comma),
    Some(Glyph::hyphen),
    Some(Glyph::period),
    Some(Glyph::slash),
    Some(Glyph::zero),
    Some(Glyph::one),
    Some(Glyph::two),
    Some(Glyph::three),
    Some(Glyph::four),
    Some(Glyph::five),
    Some(Glyph::six),
    Some(Glyph::seven),
    Some(Glyph::eight),
    Some(Glyph::nine),
    Some(Glyph::colon),
    Some(Glyph::semicolon),
    Some(Glyph::less),
    Some(Glyph::equal),
    Some(Glyph::greater),
    Some(Glyph::question),
    Some(Glyph::at),
    Some(Glyph::A),
    Some(Glyph::B),
    Some(Glyph::C),
    Some(Glyph::D),
    Some(Glyph::E),
    Some(Glyph::F),
    Some(Glyph::G),
    Some(Glyph::H),
    Some(Glyph::I),
    Some(Glyph::J),
    Some(Glyph::K),
    Some(Glyph::L),
    Some(Glyph::M),
    Some(Glyph::N),
    Some(Glyph::O),
    Some(Glyph::P),
    Some(Glyph::Q),
    Some(Glyph::R),
    Some(Glyph::S),
    Some(Glyph::T),
    Some(Glyph::U),
    Some(Glyph::V),
    Some(Glyph::W),
    Some(Glyph::X),
    Some(Glyph::Y),
    Some(Glyph::Z),
    Some(Glyph::bracketleft),
    Some(Glyph::backslash),
    Some(Glyph::bracketright),
    Some(Glyph::asciicircum),
    Some(Glyph::underscore),
    Some(Glyph::grave),
    Some(Glyph::a),
    Some(Glyph::b),
    Some(Glyph::c),
    Some(Glyph::d),
    Some(Glyph::e),
    Some(Glyph::f),
    Some(Glyph::g),
    Some(Glyph::h),
    Some(Glyph::i),
    Some(Glyph::j),
    Some(Glyph::k),
    Some(Glyph::l),
    Some(Glyph::m),
    Some(Glyph::n),
    Some(Glyph::o),
    Some(Glyph::p),
    Some(Glyph::q),
    Some(Glyph::r),
    Some(Glyph::s),
    Some(Glyph::t),
    Some(Glyph::u),
    Some(Glyph::v),
    Some(Glyph::w),
    Some(Glyph::x),
    Some(Glyph::y),
    Some(Glyph::z),
    Some(Glyph::braceleft),
    Some(Glyph::bar),
    Some(Glyph::braceright),
    Some(Glyph::asciitilde),
    Some(Glyph::bullet),
    Some(Glyph::Euro),
    Some(Glyph::bullet),
    Some(Glyph::quotesinglbase),
    Some(Glyph::florin),
    Some(Glyph::quotedblbase),
    Some(Glyph::ellipsis),
    Some(Glyph::dagger),
    Some(Glyph::daggerdbl),
    Some(Glyph::circumflex),
    Some(Glyph::perthousand),
    Some(Glyph::Scaron),
    Some(Glyph::guilsinglleft),
    Some(Glyph::OE),
    Some(Glyph::bullet),
    Some(Glyph::Zcaron),
    Some(Glyph::bullet),
    Some(Glyph::bullet),
    Some(Glyph::quoteleft),
    Some(Glyph::quoteright),
    Some(Glyph::quotedblleft),
    Some(Glyph::quotedblright),
    Some(Glyph::bullet),
    Some(Glyph::endash),
    Some(Glyph::emdash),
    Some(Glyph::tilde),
    Some(Glyph::trademark),
    Some(Glyph::scaron),
    Some(Glyph::guilsinglright),
    Some(Glyph::oe),
    Some(Glyph::bullet),
    Some(Glyph::zcaron),
    Some(Glyph::Ydieresis),
    Some(Glyph::space),
    Some(Glyph::exclamdown),
    Some(Glyph::cent),
    Some(Glyph::sterling),
    Some(Glyph::currency),
    Some(Glyph::yen),
    Some(Glyph::brokenbar),
    Some(Glyph::section),
    Some(Glyph::dieresis),
    Some(Glyph::copyright),
    Some(Glyph::ordfeminine),
    Some(Glyph::guillemotleft),
    Some(Glyph::logicalnot),
    Some(Glyph::hyphen),
    Some(Glyph::registered),
    Some(Glyph::macron),
    Some(Glyph::degree),
    Some(Glyph::plusminus),
    Some(Glyph::twosuperior),
    Some(Glyph::threesuperior),
    Some(Glyph::acute),
    Some(Glyph::mu),
    Some(Glyph::paragraph),
    Some(Glyph::periodcentered),
    Some(Glyph::cedilla),
    Some(Glyph::onesuperior),
    Some(Glyph::ordmasculine),
    Some(Glyph::guillemotright),
    Some(Glyph::onequarter),
    Some(Glyph::onehalf),
    Some(Glyph::threequarters),
    Some(Glyph::questiondown),
    Some(Glyph::Agrave),
    Some(Glyph::Aacute),
    Some(Glyph::Acircumflex),
    Some(Glyph::Atilde),
    Some(Glyph::Adieresis),
    Some(Glyph::Aring),
    Some(Glyph::AE),
    Some(Glyph::Ccedilla),
    Some(Glyph::Egrave),
    Some(Glyph::Eacute),
    Some(Glyph::Ecircumflex),
    Some(Glyph::Edieresis),
    Some(Glyph::Igrave),
    Some(Glyph::Iacute),
    Some(Glyph::Icircumflex),
    Some(Glyph::Idieresis),
    Some(Glyph::Eth),
    Some(Glyph::Ntilde),
    Some(Glyph::Ograve),
    Some(Glyph::Oacute),
    Some(Glyph::Ocircumflex),
    Some(Glyph::Otilde),
    Some(Glyph::Odieresis),
    Some(Glyph::multiply),
    Some(Glyph::Oslash),
    Some(Glyph::Ugrave),
    Some(Glyph::Uacute),
    Some(Glyph::Ucircumflex),
    Some(Glyph::Udieresis),
    Some(Glyph::Yacute),
    Some(Glyph::Thorn),
    Some(Glyph::germandbls),
    Some(Glyph::agrave),
    Some(Glyph::aacute),
    Some(Glyph::acircumflex),
    Some(Glyph::atilde),
    Some(Glyph::adieresis),
    Some(Glyph::aring),
    Some(Glyph::ae),
    Some(Glyph::ccedilla),
    Some(Glyph::egrave),
    Some(Glyph::eacute),
    Some(Glyph::ecircumflex),
    Some(Glyph::edieresis),
    Some(Glyph::igrave),
    Some(Glyph::iacute),
    Some(Glyph::icircumflex),
    Some(Glyph::idieresis),
    Some(Glyph::eth),
    Some(Glyph::ntilde),
    Some(Glyph::ograve),
    Some(Glyph::oacute),
    Some(Glyph::ocircumflex),
    Some(Glyph::otilde),
    Some(Glyph::odieresis),
    Some(Glyph::divide),
    Some(Glyph::oslash),
    Some(Glyph::ugrave),
    Some(Glyph::uacute),
    Some(Glyph::ucircumflex),
    Some(Glyph::udieresis),
    Some(Glyph::yacute),
    Some(Glyph::thorn),
    Some(Glyph::ydieresis),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(rename = "camelCase")]
pub struct PdfSaveOptions {
    /// If set to true (default), compresses streams and
    /// prunes unreferenced PDF objects. Set to false for debugging
    #[serde(default = "default_optimize")]
    pub optimize: bool,
    /// Whether to include the entire font or to subset it.
    /// Default is set to true because some CJK fonts can be massive.
    #[serde(default = "default_subset_fonts")]
    pub subset_fonts: bool,
    /// Whether to ignore unknown operations. If set to true
    /// (default), will skip any unknown PDF operations when serializing the file.
    #[serde(default = "default_secure")]
    pub secure: bool,
    /// Image optimization options
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_optimization: Option<ImageOptimizationOptions>,
}

const fn default_optimize() -> bool {
    true
}
const fn default_subset_fonts() -> bool {
    true
}
const fn default_secure() -> bool {
    true
}

impl Default for PdfSaveOptions {
    fn default() -> Self {
        Self {
            optimize: default_optimize(),
            subset_fonts: default_subset_fonts(),
            secure: default_secure(),
            image_optimization: Some(ImageOptimizationOptions::default()),
        }
    }
}

// Initializes the image resources and the document
//
// Note: this function may become async later on!
pub fn init_doc_and_resources(
    pdf: &PdfDocument,
    opts: &PdfSaveOptions,
) -> (lopdf::Document, lopdf::Dictionary) {
    let mut doc = lopdf::Document::with_version("1.3");
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;

    let mut global_xobject_dict = LoDictionary::new();
    for (k, v) in pdf.resources.xobjects.map.iter() {
        global_xobject_dict.set(
            k.0.clone(),
            crate::xobject::add_xobject_to_document(v, &mut doc, opts.image_optimization.as_ref()),
        );
    }

    (doc, global_xobject_dict)
}

pub fn serialize_pdf<W: Write>(
    pdf: &PdfDocument,
    opts: &PdfSaveOptions,
    mut writer: &mut W,
    warnings: &mut Vec<PdfWarnMsg>,
) -> () {
    let mut doc = to_lopdf_doc(pdf, opts, warnings);
    if opts.optimize {
        doc.compress();
    }

    let _ = doc.save_to(&mut writer);
}

pub fn to_lopdf_doc(
    pdf: &PdfDocument,
    opts: &PdfSaveOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> lopdf::Document {
    let (mut doc, global_xobject_dict) = init_doc_and_resources(pdf, opts);
    let pages_id = doc.new_object_id();
    let mut catalog = LoDictionary::from_iter(vec![
        ("Type", "Catalog".into()),
        ("PageLayout", "OneColumn".into()),
        ("PageMode", "UseNone".into()),
        ("Pages", Reference(pages_id)),
    ]);

    // (Optional): Add OutputIntents to catalog
    if pdf.metadata.info.conformance.must_have_icc_profile() {
        /// Default ICC profile, necessary if `PdfMetadata::must_have_icc_profile()` return true
        const ICC_PROFILE_ECI_V2: &[u8] = include_bytes!("./res/CoatedFOGRA39.icc");
        const ICC_PROFILE_LICENSE: &str = include_str!("./res/CoatedFOGRA39.icc.LICENSE.txt");

        let icc_profile_descr = "Commercial and special offset print acccording to ISO \
                                 12647-2:2004 / Amd 1, paper type 1 or 2 (matte or gloss-coated \
                                 offset paper, 115 g/m2), screen ruling 60/cm";
        let icc_profile_str = "Coated FOGRA39 (ISO 12647-2:2004)";
        let icc_profile_short = LoString("FOGRA39".into(), Literal);
        let registry = LoString("http://www.color.org".into(), Literal);
        let icc = IccProfile::new(ICC_PROFILE_ECI_V2.to_vec(), IccProfileType::Cmyk)
            .with_alternate_profile(false)
            .with_range(true);
        let icc_profile_id = doc.add_object(Stream(icc_to_stream(&icc)));
        let output_intents = LoDictionary::from_iter(vec![
            ("S", Name("GTS_PDFX".into())),
            (
                "OutputCondition",
                LoString(icc_profile_descr.into(), Literal),
            ),
            ("License", LoString(ICC_PROFILE_LICENSE.into(), Literal)),
            ("Type", Name("OutputIntent".into())),
            ("OutputConditionIdentifier", icc_profile_short),
            ("RegistryName", registry),
            ("Info", LoString(icc_profile_str.into(), Literal)),
            ("DestinationOutputProfile", Reference(icc_profile_id)),
        ]);
        catalog.set("OutputIntents", Array(vec![Dictionary(output_intents)]));
    }

    // (Optional): Add XMP Metadata to catalog
    if pdf.metadata.info.conformance.must_have_xmp_metadata() {
        let xmp_obj = Stream(LoStream::new(
            LoDictionary::from_iter(vec![("Type", "Metadata".into()), ("Subtype", "XML".into())]),
            pdf.metadata.xmp_metadata_string().as_bytes().to_vec(),
        ));
        let metadata_id = doc.add_object(xmp_obj);
        catalog.set("Metadata", Reference(metadata_id));
    }

    // (Optional): Add "OCProperties" (layers) to catalog
    // Build a mapping from each layer's internal id to a single OCG object ID.
    let layer_ids = if !pdf.resources.layers.map.is_empty() {
        let map = pdf
            .resources
            .layers
            .map
            .iter()
            .map(|(id, layer)| {
                let usage_ocg_dict = LoDictionary::from_iter(vec![
                    ("Type", Name("OCG".into())),
                    (
                        "CreatorInfo",
                        Dictionary(LoDictionary::from_iter(vec![
                            ("Creator", LoString(layer.creator.clone().into(), Literal)),
                            ("Subtype", Name(layer.usage.to_string().into())),
                        ])),
                    ),
                ]);
                let usage_ocg_dict_ref = doc.add_object(Dictionary(usage_ocg_dict));
                let intent_arr = Array(vec![Name("View".into()), Name("Design".into())]);
                let intent_arr_ref = doc.add_object(intent_arr);
                let pdf_id = doc.add_object(Dictionary(LoDictionary::from_iter(vec![
                    ("Type", Name("OCG".into())),
                    ("Name", LoString(layer.name.to_string().into(), Literal)),
                    ("Intent", Reference(intent_arr_ref)),
                    ("Usage", Reference(usage_ocg_dict_ref)),
                ])));
                (id.clone(), pdf_id)
            })
            .collect::<BTreeMap<_, _>>();
        let flattened_ocg_list = map.values().map(|s| Reference(*s)).collect::<Vec<_>>();
        catalog.set(
            "OCProperties",
            Dictionary(LoDictionary::from_iter(vec![
                ("OCGs", Array(flattened_ocg_list.clone())),
                (
                    "D",
                    Dictionary(LoDictionary::from_iter(vec![
                        ("Order", Array(flattened_ocg_list.clone())),
                        ("RBGroups", Array(vec![])),
                        ("ON", Array(flattened_ocg_list)),
                    ])),
                ),
            ])),
        );
        Some(map)
    } else {
        None
    };

    // Build fonts dictionary
    let mut global_font_dict = LoDictionary::new();
    let prepared_fonts = prepare_fonts(&pdf.resources, &pdf.pages, warnings);
    for (font_id, prepared) in prepared_fonts.iter() {
        let font_dict = add_font_to_pdf(&mut doc, font_id, prepared);
        let font_dict_id = doc.add_object(font_dict);
        global_font_dict.set(font_id.0.clone(), Reference(font_dict_id));
    }

    let prepared_subsetfonts = prepare_subsetfonts(&pdf.resources, &pdf.pages, warnings);
    for (font_id, prepared) in prepared_subsetfonts.iter() {
        let font_dict = add_subsetfont_to_pdf(&mut doc, font_id, prepared);
        let font_dict_id = doc.add_object(font_dict);
        global_font_dict.set(font_id.0.clone(), Reference(font_dict_id));
    }

    for internal_font in get_used_internal_fonts(&pdf.pages) {
        let font_dict = builtin_font_to_dict(&internal_font);
        let font_dict_id = doc.add_object(font_dict);
        global_font_dict.set(internal_font.get_pdf_id(), Reference(font_dict_id));
    }
    let global_font_dict_id = doc.add_object(global_font_dict);

    let global_xobject_dict_id = doc.add_object(global_xobject_dict);

    let mut global_extgstate_dict = LoDictionary::new();
    for (k, v) in pdf.resources.extgstates.map.iter() {
        global_extgstate_dict.set(k.0.clone(), crate::graphics::extgstate_to_dict(v));
    }
    let global_extgstate_dict_id = doc.add_object(global_extgstate_dict);

    let page_ids_reserved = pdf
        .pages
        .iter()
        .map(|_| doc.new_object_id())
        .collect::<Vec<_>>();

    // Render pages
    let page_ids = pdf
        .pages
        .iter()
        .zip(page_ids_reserved.iter())
        .map(|(page, page_id)| {
            let mut page_resources = LoDictionary::new();

            // Instead of re-creating new OCG dictionaries here,
            // re-use the objects from the global layer_ids mapping.
            if let Some(ref layer_ids) = layer_ids {
                let page_layers = page
                    .ops
                    .iter()
                    .filter_map(|op| {
                        if let Op::BeginLayer { layer_id } = op {
                            layer_ids
                                .get(layer_id)
                                .map(|ocg_obj_id| (layer_id.0.clone(), *ocg_obj_id))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if !page_layers.is_empty() {
                    page_resources.set(
                        "Properties",
                        LoDictionary::from_iter(
                            page_layers
                                .iter()
                                .map(|(name, ocg_obj_id)| (name.as_str(), Reference(*ocg_obj_id))),
                        ),
                    );
                }
            }

            // Gather annotations
            let mut links = Vec::new();
            for op in &page.ops {
                if let Op::LinkAnnotation { link } = op {
                    links.push(Dictionary(link_annotation_to_dict(
                        link,
                        &page_ids_reserved,
                    )))
                }
            }

            page_resources.set("Font", Reference(global_font_dict_id));
            page_resources.set("XObject", Reference(global_xobject_dict_id));
            page_resources.set("ExtGState", Reference(global_extgstate_dict_id));

            let layer_stream = translate_operations(
                &page.ops,
                &prepared_fonts,
                &prepared_subsetfonts,
                &pdf.resources.xobjects.map,
                opts.secure,
                warnings,
            ); // Vec<u8>
            let merged_layer_stream =
                LoStream::new(LoDictionary::new(), layer_stream);

            let page_obj = LoDictionary::from_iter(vec![
                ("Type", "Page".into()),
                ("MediaBox", page.get_media_box()),
                ("TrimBox", page.get_trim_box()),
                ("CropBox", page.get_crop_box()),
                ("Parent", Reference(pages_id)),
                ("Resources", Reference(doc.add_object(page_resources))),
                ("Contents", Reference(doc.add_object(merged_layer_stream))),
                ("Annots", Array(links)),
            ]);

            doc.set_object(*page_id, page_obj);

            *page_id
        })
        .collect::<Vec<_>>();

    // Now that the page objs are rendered, resolve which bookmarks reference which page objs
    if !pdf.bookmarks.map.is_empty() {
        let bookmarks_id = doc.new_object_id();
        let mut bookmarks_sorted = pdf.bookmarks.map.iter().collect::<Vec<_>>();
        bookmarks_sorted.sort_by(|(_, v), (_, v2)| (v.page, &v.name).cmp(&(v2.page, &v2.name)));
        let bookmarks_sorted = bookmarks_sorted
            .into_iter()
            .filter_map(|(k, v)| {
                let page_obj_id = page_ids.get(v.page.saturating_sub(1)).cloned()?;
                Some((k, &v.name, page_obj_id))
            })
            .collect::<Vec<_>>();

        let bookmark_ids = bookmarks_sorted
            .iter()
            .map(|(id, name, page_id)| {
                let newid = doc.new_object_id();
                (id, name, page_id, newid)
            })
            .collect::<Vec<_>>();

        let first = bookmark_ids.first().map(|s| s.3).unwrap();
        let last = bookmark_ids.last().map(|s| s.3).unwrap();
        for (i, (_id, name, pageid, self_id)) in bookmark_ids.iter().enumerate() {
            let prev = if i == 0 {
                None
            } else {
                bookmark_ids.get(i - 1).map(|s| s.3)
            };
            let next = bookmark_ids.get(i + 1).map(|s| s.3);
            let dest = Array(vec![Reference(*(*pageid)), "XYZ".into(), Null, Null, Null]);
            let mut dict = LoDictionary::from_iter(vec![
                ("Parent", Reference(bookmarks_id)),
                ("Title", encode_text_to_utf16be(name)),
                ("Dest", dest),
            ]);
            if let Some(prev) = prev {
                dict.set("Prev", Reference(prev));
            }
            if let Some(next) = next {
                dict.set("Next", Reference(next));
            }
            doc.set_object(*self_id, dict);
        }

        let bookmarks_list = LoDictionary::from_iter(vec![
            ("Type", "Outlines".into()),
            ("Count", Integer(pdf.bookmarks.map.len() as i64)),
            ("First", Reference(first)),
            ("Last", Reference(last)),
        ]);

        doc.set_object(bookmarks_id, bookmarks_list);
        catalog.set("Outlines", Reference(bookmarks_id));
        catalog.set("PageMode", LoString("UseOutlines".into(), Literal));
    }

    doc.set_object(
        pages_id,
        LoDictionary::from_iter(vec![
            ("Type", "Pages".into()),
            ("Count", Integer(page_ids.len() as i64)),
            (
                "Kids",
                Array(page_ids.iter().map(|q| Reference(*q)).collect::<Vec<_>>()),
            ),
        ]),
    );

    let catalog_id = doc.add_object(catalog);
    let document_info_id = doc.add_object(Dictionary(docinfo_to_dict(&pdf.metadata.info)));
    let instance_id = crate::utils::random_character_string_32();
    let document_id = crate::utils::random_character_string_32();

    doc.trailer.set("Root", Reference(catalog_id));
    doc.trailer.set("Info", Reference(document_info_id));
    doc.trailer.set(
        "ID",
        Array(vec![
            LoString(document_id.as_bytes().to_vec(), Literal),
            LoString(instance_id.as_bytes().to_vec(), Literal),
        ]),
    );

    doc
}
pub fn serialize_pdf_into_bytes(
    pdf: &PdfDocument,
    opts: &PdfSaveOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut writer = std::io::BufWriter::new(&mut bytes);
    serialize_pdf(pdf, opts, &mut writer, warnings);
    std::mem::drop(writer);
    bytes
}
fn get_used_internal_fonts(pages: &[PdfPage]) -> BTreeSet<BuiltinFont> {
    pages
        .iter()
        .flat_map(|p| {
            p.ops.iter().filter_map(|op| match op {
                Op::WriteTextBuiltinFont { font, .. } => Some(*font),
                _ => None,
            })
        })
        .collect()
}

fn builtin_font_to_dict(font: &BuiltinFont) -> LoDictionary {
    LoDictionary::from_iter(vec![
        ("Type", Name("Font".into())),
        ("Subtype", Name("Type1".into())),
        ("BaseFont", Name(font.get_id().into())),
        ("Encoding", Name("WinAnsiEncoding".into())),
    ])
}

pub(crate) fn translate_operations(
    ops: &[Op],
    fonts: &BTreeMap<FontId, PreparedFont>,
    subsetfonts: &BTreeMap<FontId, PreparedSubsetFont>,
    xobjects: &BTreeMap<XObjectId, XObject>,
    secure: bool,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Vec<u8> {
    let mut content = Vec::new();

    for op in ops {
        match op {
            Op::SetRenderingIntent { intent } => {
                content.push(LoOp::new("ri", vec![Name(intent.get_id().into())]));
            }
            Op::SetColorSpaceFill { id } => {
                content.push(LoOp::new("cs", vec![Name(id.clone().into())]));
            }
            Op::SetColorSpaceStroke { id } => {
                content.push(LoOp::new("CS", vec![Name(id.clone().into())]));
            }
            Op::SetHorizontalScaling { percent } => {
                content.push(LoOp::new("Tz", vec![Real(*percent)]));
            }
            Op::AddLineBreak => {
                content.push(LoOp::new("T*", vec![]));
            }
            Op::Marker { id } => {
                content.push(LoOp::new("MP", vec![Name(id.clone().into())]));
            }
            Op::BeginLayer { layer_id } => {
                content.push(LoOp::new(
                    "BDC",
                    vec![Name("OC".into()), Name(layer_id.0.clone().into())],
                ));
            }
            Op::EndLayer => {
                content.push(LoOp::new("EMC", vec![]));
            }
            Op::SaveGraphicsState => {
                content.push(LoOp::new("q", vec![]));
            }
            Op::RestoreGraphicsState => {
                content.push(LoOp::new("Q", vec![]));
            }
            Op::LoadGraphicsState { gs } => {
                content.push(LoOp::new("gs", vec![Name(gs.0.as_bytes().to_vec())]));
            }
            Op::StartTextSection => {
                content.push(LoOp::new("BT", vec![]));
            }
            Op::EndTextSection => {
                content.push(LoOp::new("ET", vec![]));
            }
            Op::WriteTextBuiltinFont { items, font } => {
                encode_text_items_to_pdf::<PreparedFont>(items, None, None, Some(font), &mut content);
            }
            Op::WriteText { items, font } => {
                if let Some(prepared_font) = fonts.get(font) {
                    encode_text_items_to_pdf(items, Some(prepared_font), None, None, &mut content);
                }
                if let Some(prepared_subsetfont) = subsetfonts.get(font) {
                    encode_text_items_to_pdf::<PreparedFont>(items, None, Some(prepared_subsetfont), None, &mut content);
                }
            }
            Op::WriteCodepoints { font, cp } => {
                if let Some(prepared_font) = fonts.get(font) {
                    let mapping = &prepared_font.subset.glyph_mapping;
                    let codepoints = cp.iter().map(|(gid, _)| {
                        mapping
                            .get(gid)
                            .map(|&(src_gid, _)| (src_gid, 0))
                            .unwrap_or((0, 0))
                    });
                    encode_codepoints_to_pdf(codepoints, &mut content);
                }
            }
            Op::WriteCodepointsWithKerning { font, cpk } => {
                if let Some(prepared_font) = fonts.get(font) {
                    let mapping = &prepared_font.subset.glyph_mapping;
                    let codepoints = cpk.iter().map(|(kern, gid, _)| {
                        mapping
                            .get(gid)
                            .map(|&(subset_gid, _)| (subset_gid, *kern))
                            .unwrap_or((0, 0))
                    });
                    encode_codepoints_to_pdf(codepoints, &mut content);
                }
            }
            Op::SetLineHeight { lh } => {
                content.push(LoOp::new("TL", vec![Real(lh.0)]));
            }
            Op::SetWordSpacing { pt } => {
                content.push(LoOp::new("Tw", vec![Real(pt.0)]));
            }
            Op::SetFontSize { size, font } => {
                content.push(LoOp::new(
                    "Tf",
                    vec![font.0.clone().into(), (size.0).into()],
                ));
            }
            Op::SetFontSizeBuiltinFont { size, font } => {
                content.push(LoOp::new(
                    "Tf",
                    vec![font.get_pdf_id().into(), (size.0).into()],
                ));
            }
            Op::SetTextCursor { pos } => {
                content.push(LoOp::new("Td", vec![pos.x.0.into(), pos.y.0.into()]));
            }
            Op::SetFillColor { col } => {
                let ci = match &col {
                    Color::Rgb(_) => "rg",
                    Color::Cmyk(_) | Color::SpotColor(_) => "k",
                    Color::Greyscale(_) => "g",
                };

                if col.is_out_of_range() {
                    warnings.push(PdfWarnMsg::error(
                        0,
                        0,
                        format!(
                            "PDF color {col:?} is out of range, must be normalized to 0.0 - 1.0"
                        ),
                    ));
                }
                let cvec = col.into_vec().into_iter().map(Real).collect();
                content.push(LoOp::new(ci, cvec));
            }
            Op::SetOutlineColor { col } => {
                let ci = match &col {
                    Color::Rgb(_) => "RG",
                    Color::Cmyk(_) | Color::SpotColor(_) => "K",
                    Color::Greyscale(_) => "G",
                };
                if col.is_out_of_range() {
                    warnings.push(PdfWarnMsg::error(
                        0,
                        0,
                        format!(
                            "PDF color {col:?} is out of range, must be normalized to 0.0 - 1.0"
                        ),
                    ));
                }
                let cvec = col.into_vec().into_iter().map(Real).collect();
                content.push(LoOp::new(ci, cvec));
            }
            Op::SetOutlineThickness { pt } => {
                content.push(LoOp::new("w", vec![Real(pt.0)]));
            }
            Op::SetLineDashPattern { dash } => {
                let dash_array_ints = dash.as_array().into_iter().map(Integer).collect();
                content.push(LoOp::new(
                    "d",
                    vec![Array(dash_array_ints), Integer(dash.offset)],
                ));
            }
            Op::SetLineJoinStyle { join } => {
                content.push(LoOp::new("j", vec![Integer(join.id())]));
            }
            Op::SetMiterLimit { limit } => {
                content.push(LoOp::new("M", vec![Real(limit.0)]));
            }
            Op::SetLineCapStyle { cap } => {
                content.push(LoOp::new("J", vec![Integer(cap.id())]));
            }
            Op::SetTextRenderingMode { mode } => {
                content.push(LoOp::new("Tr", vec![Integer(mode.id())]));
            }
            Op::SetCharacterSpacing { multiplier } => {
                content.push(LoOp::new("Tc", vec![Real(*multiplier)]));
            }
            Op::SetLineOffset { multiplier } => {
                content.push(LoOp::new("Ts", vec![Real(*multiplier)]));
            }
            Op::DrawLine { line } => {
                content.append(&mut line_to_stream_ops(line));
            }
            Op::DrawPolygon { polygon } => {
                content.append(&mut polygon_to_stream_ops(polygon));
            }
            Op::DrawRectangle { rectangle } => {
                content.append(&mut rectangle_to_stream_ops(rectangle));
            }
            Op::SetTransformationMatrix { matrix } => {
                content.push(LoOp::new(
                    "cm",
                    matrix.as_array().iter().copied().map(Real).collect(),
                ));
            }
            Op::SetTextMatrix { matrix } => {
                content.push(LoOp::new(
                    "Tm",
                    matrix.as_array().iter().copied().map(Real).collect(),
                ));
            }
            Op::LinkAnnotation { link: _ } => {}
            Op::UseXobject { id, transform } => {
                use crate::matrix::CurTransMat;
                let mut t = CurTransMat::Identity;
                for q in
                    transform.get_ctms(xobjects.get(id).and_then(|xobj| xobj.get_width_height()))
                {
                    t = CurTransMat::Raw(CurTransMat::combine_matrix(t.as_array(), q.as_array()));
                }

                content.push(LoOp::new("q", vec![]));
                content.push(LoOp::new(
                    "cm",
                    t.as_array().into_iter().map(Real).collect(),
                ));
                content.push(LoOp::new("Do", vec![Name(id.0.as_bytes().to_vec())]));
                content.push(LoOp::new("Q", vec![]));
            }
            Op::BeginInlineImage => {
                content.push(LoOp::new("BI", vec![]));
            }
            Op::BeginInlineImageData => {
                content.push(LoOp::new("ID", vec![]));
            }
            Op::EndInlineImage => {
                content.push(LoOp::new("EI", vec![]));
            }
            Op::BeginMarkedContent { tag } => {
                content.push(LoOp::new("BMC", vec![Name(tag.clone().into())]));
            }
            Op::BeginMarkedContentWithProperties { tag, properties } => {
                content.push(LoOp::new(
                    "BDC",
                    vec![Name(tag.clone().into()), properties.to_lopdf()]
                ));
            }
            Op::BeginOptionalContent { layer_id } => {
                content.push(LoOp::new(
                    "BDC",
                    vec![Name("OC".into()), Name(layer_id.0.clone().into())],
                ));
            }
            Op::DefineMarkedContentPoint { tag, properties } => {
                let props = Array(properties.iter().map(|item| item.to_lopdf()).collect());
                content.push(LoOp::new("DP", vec![Name(tag.clone().into()), props]));
            }
            Op::EndMarkedContent { .. } | Op::EndMarkedContentWithProperties { .. } | Op::EndOptionalContent { .. } => {
                content.push(LoOp::new("EMC", vec![]));
            }
            Op::BeginCompatibilitySection => {
                content.push(LoOp::new("BX", vec![]));
            }
            Op::EndCompatibilitySection => {
                content.push(LoOp::new("EX", vec![]));
            }
            Op::MoveToNextLineShowText { text } => {
                content.push(LoOp::new(
                    "'",
                    vec![LoString(text.as_bytes().to_vec(), Hexadecimal)],
                ));
            }
            Op::SetSpacingMoveAndShowText {
                word_spacing,
                char_spacing,
                text,
            } => {
                content.push(LoOp::new(
                    "\"",
                    vec![
                        Real(*word_spacing),
                        Real(*char_spacing),
                        LoString(text.as_bytes().to_vec(), Hexadecimal),
                    ],
                ));
            }
            Op::MoveTextCursorAndSetLeading { tx, ty } => {
                content.push(LoOp::new("TD", vec![Real(*tx), Real(*ty)]));
            }
            Op::Unknown { key, value } => {
                // Skip unknown operators for security reasons.
                if !secure {
                    content.push(LoOp::new(
                        key.as_str(),
                        value.iter().map(|s| s.to_lopdf()).collect(),
                    ));
                }
            }
        }
    }

    lopdf::content::Content {
        operations: content,
    }
    .encode()
    .unwrap_or_default()
}

// Helper function to encode text items to PDF operations
fn encode_text_items_to_pdf<T: PrepFont>(
    items: &[TextItem],
    prepared_font: Option<&T>,
    prepared_subsetfont: Option<&PreparedSubsetFont>,
    builtin_font: Option<&BuiltinFont>,
    content: &mut Vec<LoOp>,
) {
    // Skip if no items
    if items.is_empty() {
        return;
    }

    // Process text items into PDF objects for TJ/Tj operator
    let mut tj_array = Vec::new();

    for item in items {
        match item {
            TextItem::Text(text) => {
                if let Some(font) = prepared_font {
                    // For custom fonts, convert each character to its subset glyph ID
                    let bytes = if true {
                        text.chars()
                            .flat_map(|c| {
                                font.lgi(c as u32)
                                    .and_then(|src_gdi| font.index_to_cid(src_gdi as u16))
                                    .unwrap_or(0)
                                    .to_be_bytes()
                            })
                            .collect()
                    } else {
                        // This branch is for reference/comparison but not used
                        // It would try to use lopdf::Document::encode_text if it supported
                        // UnicodeMapEncoding
                        vec![]
                    };

                    // Custom fonts must use hexadecimal encoding in PDF
                    tj_array.push(LoString(bytes, Hexadecimal));
                } else if let Some(font) = prepared_subsetfont {
                    // For embedded subset fonts, convert each character to its subset glyph ID
                    let bytes = if true {
                        match font.original.font_type {
                            FontType::ParsedEmbeddedType0( .. ) => {
                                // Type0 embedded subset fonts use two bytes per character
                                text.chars()
                                    .flat_map(|c| {
                                        font.original.cmap.as_ref().unwrap().mappings
                                            .iter()
                                            .find(|(_, unicodechar)| {
                                                let c = c as u32;
                                                unicodechar.contains(&c)
                                            })
                                            .map(|(cid, _)| *cid as u16)
                                            .unwrap_or(0)
                                            .to_be_bytes()
                                    })
                                    .collect()
                            },
                            FontType::ParsedEmbeddedType1C( .. ) => {
                                // Type1C embedded subset fonts use one byte per character
                                text.chars()
                                    .map(|c| {
                                        font.original.cmap.as_ref().unwrap().mappings
                                            .iter()
                                            .find(|(_, unicodechar)| {
                                                let c = c as u32;
                                                unicodechar.contains(&c)
                                            })
                                            .map(|(cid, _)| *cid as u8)
                                            .unwrap_or(0)
                                    })
                                    .collect()
                            },
                            _ => unimplemented!(),
                        }
                    } else {
                        // This branch is for reference/comparison but not used
                        // It would try to use lopdf::Document::encode_text if it supported
                        // UnicodeMapEncoding
                        vec![]
                    };

                    // Custom fonts must use hexadecimal encoding in PDF
                    tj_array.push(LoString(bytes, Hexadecimal));
                } else if builtin_font.is_some() {
                    // For built-in fonts, use WinAnsiEncoding
                    let bytes = lopdf::Document::encode_text(
                        &lopdf::Encoding::OneByteEncoding(&WIN_ANSI_ENCODING),
                        text,
                    );

                    // Choose appropriate string format based on content
                    let string_format = if needs_hex_encoding(&bytes) {
                        Hexadecimal
                    } else {
                        Literal
                    };

                    tj_array.push(LoString(bytes, string_format));
                }
            }
            TextItem::Offset(offset) => {
                tj_array.push(Real(*offset));
            }
        }
    }

    // Choose appropriate operator based on complexity
    if tj_array.len() == 1 && !items.iter().any(|i| matches!(i, TextItem::Offset(_))) {
        // Single text item with no kerning - use simpler Tj
        content.push(LoOp::new("Tj", vec![tj_array.swap_remove(0)]));
    } else {
        // Multiple items or has kerning offsets - use TJ
        content.push(LoOp::new("TJ", vec![Array(tj_array)]));
    }
}

// Helper function to encode codepoints to PDF operations
fn encode_codepoints_to_pdf(codepoints: impl Iterator<Item = (u16, i64)>, content: &mut Vec<LoOp>) {
    let mut tj_array = Vec::new();
    let mut any_kerning = false;

    for (codepoint, kerning) in codepoints {
        if kerning != 0 {
            any_kerning = true;
            tj_array.push(Real(kerning as f32));
        }

        tj_array.push(LoString(codepoint.to_be_bytes().to_vec(), Hexadecimal));
    }

    match tj_array.len() {
        0 => {}
        1 if !any_kerning => {
            content.push(LoOp::new("Tj", vec![tj_array.swap_remove(0)]));
        }
        _ => {
            content.push(LoOp::new("TJ", vec![Array(tj_array)]));
        }
    }
}

// Helper function to determine if bytes need hexadecimal encoding
fn needs_hex_encoding(bytes: &[u8]) -> bool {
    bytes.iter().any(|&b| {
        // Bytes that require hex encoding:
        // - Control characters
        // - Non-ASCII characters
        // - Special characters like (, ), \, etc.
        b < 32 || b > 126 || b == b'(' || b == b')' || b == b'\\' || b == b'%'
    })
}

pub(crate) struct PreparedFont {
    original: ParsedFont,
    pub(crate) subset: SubsetFont,
    cid_to_unicode_map: String,
    vertical_writing: bool, // default: false
    ascent: i64,
    descent: i64,
    // max_height: i64,
    // total_width: i64,
    // encode widths / heights so that they fit into what PDF expects
    // see page 439 in the PDF 1.7 reference
    // basically widths_list will contain objects like this:
    // 20 [21, 99, 34, 25]
    // which means that the character with the GID 20 has a width of 21 units
    // and the character with the GID 21 has a width of 99 units
    widths_list: Vec<lopdf::Object>,
}

impl PreparedFont {
    pub fn new(
        font_id: &FontId,
        font: &ParsedFont,
        glyph_ids: BTreeMap<u16, char>,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Option<Self> {
        let subset = font.subset(&glyph_ids);

        let subset = match subset {
            Ok(subset) => subset,
            Err(e) => {
                warnings.push(PdfWarnMsg::error(
                    0,
                    0,
                    format!("failed to subset font: {e}"),
                ));
                return None;
            }
        };

        let font = ParsedFont::from_bytes(&subset.bytes, 0, warnings)?;
        assert_eq!(font.original_bytes.len(), subset.bytes.len());

        let new_glyph_ids: Vec<(u16, char)> = glyph_ids
            .iter()
            .filter_map(|(orig_gid, _)| subset.glyph_mapping.get(orig_gid).copied())
            .collect();

        let cid_to_unicode_map = font.generate_cmap_string(font_id, &new_glyph_ids);

        let widths = match font.font_type {
            FontType::TrueType => font.get_normalized_widths_ttf(&new_glyph_ids),
            _ => {
                let gid_to_cid_map = font.generate_gid_to_cid_map(&new_glyph_ids);
                font.get_normalized_widths_cff(&gid_to_cid_map)
            }
        };

        Some(PreparedFont {
            ascent: font.font_metrics.ascender as i64,
            descent: font.font_metrics.descender as i64,
            vertical_writing: false, // !font.vmtx_data.is_empty(),
            widths_list: widths,

            original: font,
            subset,
            cid_to_unicode_map,
        })
    }
}
impl PrepFont for PreparedFont {
    fn lgi(&self, codepoint: u32) -> Option<u32> {
        self.original.lgi(codepoint) // .lookup_glyph_index(codepoint).map(Into::into)
    }

    fn index_to_cid(&self, index: u16) -> Option<u16> {
        self.original.index_to_cid(index)
    }
}

pub(crate) struct PreparedSubsetFont {
    original: ParsedSubsetFont,
}

const DEFAULT_CHARACTER_WIDTH: i64 = 1000;

fn line_to_stream_ops(line: &Line) -> Vec<LoOp> {
    /// Move to point
    pub const OP_PATH_CONST_MOVE_TO: &str = "m";
    /// Straight line to point
    pub const OP_PATH_CONST_LINE_TO: &str = "l";
    /// Cubic bezier with three control points
    pub const OP_PATH_CONST_4BEZIER: &str = "c";
    /// Stroke path
    pub const OP_PATH_PAINT_STROKE: &str = "S";
    /// Close path
    pub const OP_PATH_CLOSE: &str = "h";

    let mut operations = Vec::new();
    let points = &line.points;

    if points.is_empty() {
        return operations;
    }

    // Start with a move to the first point
    operations.push(LoOp::new(
        OP_PATH_CONST_MOVE_TO,
        vec![points[0].p.x.into(), points[0].p.y.into()],
    ));

    // Process remaining points
    let mut i = 1;
    while i < points.len() {
        let current = &points[i];

        if current.bezier {
            // Current point is a bezier handle
            // For a cubic bezier, we need two control points and an end point
            if i + 2 < points.len() {
                let control1 = current;
                let control2 = &points[i + 1];
                let end_point = &points[i + 2];

                // Check if second control point is also a bezier handle
                if control2.bezier {
                    // Two bezier handles followed by an end point
                    operations.push(LoOp::new(
                        OP_PATH_CONST_4BEZIER,
                        vec![
                            control1.p.x.into(),
                            control1.p.y.into(),
                            control2.p.x.into(),
                            control2.p.y.into(),
                            end_point.p.x.into(),
                            end_point.p.y.into(),
                        ],
                    ));
                    i += 3; // Skip past the control points and end point
                } else {
                    // Only one bezier handle - treat as a line to be safe
                    operations.push(LoOp::new(
                        OP_PATH_CONST_LINE_TO,
                        vec![current.p.x.into(), current.p.y.into()],
                    ));
                    i += 1;
                }
            } else {
                // Not enough points left for a bezier curve
                operations.push(LoOp::new(
                    OP_PATH_CONST_LINE_TO,
                    vec![current.p.x.into(), current.p.y.into()],
                ));
                i += 1;
            }
        } else {
            // Regular point - draw a straight line
            operations.push(LoOp::new(
                OP_PATH_CONST_LINE_TO,
                vec![current.p.x.into(), current.p.y.into()],
            ));
            i += 1;
        }
    }

    // Add final operations
    if line.is_closed {
        // Close the path before stroking
        operations.push(LoOp::new(OP_PATH_CLOSE, vec![]));
        operations.push(LoOp::new(OP_PATH_PAINT_STROKE, vec![]));
    } else {
        // Just stroke without closing
        operations.push(LoOp::new(OP_PATH_PAINT_STROKE, vec![]));
    }

    operations
}

fn polygon_to_stream_ops(poly: &Polygon) -> Vec<LoOp> {
    /// Move to point
    pub const OP_PATH_CONST_MOVE_TO: &str = "m";
    /// Straight line to point
    pub const OP_PATH_CONST_LINE_TO: &str = "l";
    /// Cubic bezier with three control points
    pub const OP_PATH_CONST_4BEZIER: &str = "c";
    /// End path without filling or stroking
    pub const OP_PATH_PAINT_END: &str = "n";

    let mut operations = Vec::new();

    if poly.rings.is_empty() {
        return operations;
    }

    for ring in &poly.rings {
        let points = &ring.points;

        if points.is_empty() {
            continue;
        }

        // Start with a move to the first point
        operations.push(LoOp::new(
            OP_PATH_CONST_MOVE_TO,
            vec![points[0].p.x.into(), points[0].p.y.into()],
        ));

        // Process remaining points
        let mut i = 1;
        while i < points.len() {
            let current = &points[i];

            if current.bezier {
                // Current point is a bezier handle
                // For a cubic bezier, we need two control points and an end point
                if i + 2 < points.len() {
                    let control1 = current;
                    let control2 = &points[i + 1];
                    let end_point = &points[i + 2];

                    // Check if second control point is also a bezier handle
                    if control2.bezier {
                        // Two bezier handles followed by an end point
                        operations.push(LoOp::new(
                            OP_PATH_CONST_4BEZIER,
                            vec![
                                control1.p.x.into(),
                                control1.p.y.into(),
                                control2.p.x.into(),
                                control2.p.y.into(),
                                end_point.p.x.into(),
                                end_point.p.y.into(),
                            ],
                        ));
                        i += 3; // Skip past the control points and end point
                    } else {
                        // Only one bezier handle - treat as a line to be safe
                        operations.push(LoOp::new(
                            OP_PATH_CONST_LINE_TO,
                            vec![current.p.x.into(), current.p.y.into()],
                        ));
                        i += 1;
                    }
                } else {
                    // Not enough points left for a bezier curve
                    operations.push(LoOp::new(
                        OP_PATH_CONST_LINE_TO,
                        vec![current.p.x.into(), current.p.y.into()],
                    ));
                    i += 1;
                }
            } else {
                // Regular point - draw a straight line
                operations.push(LoOp::new(
                    OP_PATH_CONST_LINE_TO,
                    vec![current.p.x.into(), current.p.y.into()],
                ));
                i += 1;
            }
        }
    }

    // Explicitly close the path with 'h' before applying painting operations
    operations.push(LoOp::new("h", vec![]));

    // Apply the painting operation based on the mode
    match poly.mode {
        PaintMode::Clip => {
            operations.push(LoOp::new(poly.winding_order.get_clip_op(), vec![]));
            // End the path with 'n' only after clipping
            operations.push(LoOp::new(OP_PATH_PAINT_END, vec![]));
        }
        PaintMode::Fill => {
            operations.push(LoOp::new(poly.winding_order.get_fill_op(), vec![]));
        }
        PaintMode::Stroke => {
            // Use 'S' (stroke) rather than 's' (close and stroke) since we already closed with 'h'
            operations.push(LoOp::new("S", vec![]));
        }
        PaintMode::FillStroke => {
            operations.push(LoOp::new(
                poly.winding_order.get_fill_stroke_close_op(),
                vec![],
            ));
        }
    }

    operations
}

fn rectangle_to_stream_ops(rectangle: &crate::Rect) -> Vec<LoOp> {
    let mut operations = Vec::new();

    // x, y, with, height
    operations.push(LoOp::new(
        "re",
        vec![
            rectangle.x.into(),
            rectangle.y.into(),
            rectangle.width.into(),
            rectangle.height.into()
        ],
    ));

    match rectangle.winding_order {
        Some(crate::WindingOrder::NonZero) => operations.push(LoOp::new("W", vec![])),
        Some(crate::WindingOrder::EvenOdd) => operations.push(LoOp::new("W*", vec![])),
        None => {},
    }

    // close the path
    operations.push(LoOp::new("n", vec![]));

    operations
}

pub(crate) fn prepare_fonts(
    resources: &PdfResources,
    pages: &[PdfPage],
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<FontId, PreparedFont> {
    let mut fonts_in_pdf = BTreeMap::new();

    for (font_id, font) in resources.fonts.map.iter() {
        let glyph_ids = font.get_used_glyph_ids(font_id, pages);
        if glyph_ids.is_empty() {
            continue; // unused font
        }

        let prepared_font = match PreparedFont::new(font_id, font, glyph_ids, warnings) {
            Some(s) => s,
            None => continue,
        };

        fonts_in_pdf.insert(font_id.clone(), prepared_font);
    }

    fonts_in_pdf
}

pub(crate) fn prepare_subsetfonts(
    resources: &PdfResources,
    _pages: &[PdfPage],
    _warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<FontId, PreparedSubsetFont> {
    let mut fonts_in_pdf = BTreeMap::new();

    for (font_id, font) in resources.subsetfonts.map.iter() {
        let prepared_font = PreparedSubsetFont{
            original: font.clone(),
        };

        fonts_in_pdf.insert(font_id.clone(), prepared_font);
    }

    fonts_in_pdf
}


fn add_font_to_pdf(
    doc: &mut lopdf::Document,
    font_id: &FontId,
    prepared: &PreparedFont,
) -> LoDictionary {
    let font_name = prepared
        .original
        .font_name
        .clone()
        .unwrap_or(font_id.0.clone());

    // font ids are US-Ascii only, so `chars()` will always be on a character boundary
    // this will make the font as subsetted
    let face_name = format!("{}+{}", font_id.0.clone().chars().take(6).collect::<String>(), font_name);

    let vertical = prepared.vertical_writing;

    let (sub_type, font_tuple) = match &prepared.original.font_type {
        FontType::OpenTypeCFF(buf) => {
            // WARNING: Font stream MAY NOT be compressed
            let font_stream = LoStream::new(
                LoDictionary::from_iter(vec![("Subtype", Name("CIDFontType0C".into()))]),
                buf.clone(),
            )
            .with_compression(false);

            (
                "CIDFontType0",
                ("FontFile3", Reference(doc.add_object(font_stream))),
            )
        }
        FontType::OpenTypeCFF2 => {
            unimplemented!()
        }
        FontType::TrueType => {
            // WARNING: Font stream MAY NOT be compressed
            let font_stream =
                LoStream::new(LoDictionary::new(), prepared.subset.bytes.clone().into())
                    .with_compression(false);

            (
                "CIDFontType2",
                ("FontFile2", Reference(doc.add_object(font_stream))),
            )
        }
        _ => {
            unimplemented!()
        }
    };

    LoDictionary::from_iter(vec![
        ("Type", Name("Font".into())),
        ("Subtype", Name("Type0".into())),
        ("BaseFont", Name(face_name.clone().into_bytes())),
        (
            "Encoding",
            if vertical {
                Name("Identity-V".into())
            } else {
                Name("Identity-H".into())
            },
        ),
        (
            "ToUnicode",
            Reference(doc.add_object(LoStream::new(
                LoDictionary::new(),
                prepared.cid_to_unicode_map.as_bytes().to_vec(),
            ))),
        ),
        (
            "DescendantFonts",
            Array(vec![Dictionary(LoDictionary::from_iter(vec![
                ("Type", Name("Font".into())),
                ("BaseFont", Name(face_name.clone().into_bytes())),
                ("Subtype", Name(sub_type.into())),
                (
                    "CIDSystemInfo",
                    Dictionary(LoDictionary::from_iter(vec![
                        ("Registry", LoString("Adobe".into(), Literal)),
                        ("Ordering", LoString("Identity".into(), Literal)),
                        ("Supplement", Integer(0)),
                    ])),
                ),
                (
                    if vertical { "W2" } else { "W" },
                    Array(prepared.widths_list.clone()),
                ),
                (
                    if vertical { "DW2" } else { "DW" },
                    Integer(DEFAULT_CHARACTER_WIDTH),
                ),
                (
                    "FontDescriptor",
                    Reference(
                        doc.add_object(LoDictionary::from_iter(vec![
                            ("Type", Name("FontDescriptor".into())),
                            ("FontName", Name(font_name.clone().into_bytes())),
                            ("Ascent", Integer(prepared.ascent)),
                            ("Descent", Integer(prepared.descent)),
                            (
                                "CapHeight",
                                Integer(
                                    prepared.original.font_metrics.s_cap_height.unwrap_or(0) as i64
                                ),
                            ),
                            ("ItalicAngle", Integer(0)),
                            ("Flags", Integer(32)),
                            ("StemV", Integer(80)),
                            font_tuple,
                            (
                                "FontBBox",
                                Array(vec![
                                    Integer(prepared.original.font_metrics.x_min as i64),
                                    Integer(prepared.original.font_metrics.y_min as i64),
                                    Integer(prepared.original.font_metrics.x_max as i64),
                                    Integer(prepared.original.font_metrics.y_max as i64),
                                ]),
                            ),
                        ])),
                    ),
                ),
            ]))]),
        ),
    ])
}

fn add_subsetfont_to_pdf(
    doc: &mut lopdf::Document,
    font_id: &FontId,
    prepared: &PreparedSubsetFont,
) -> LoDictionary {
    let font_name = prepared
        .original
        .font_name
        .clone()
        .unwrap_or(font_id.0.clone());

    // previously embedded subset fonts found during parsing already have the correct face_name
    let face_name = font_name.clone();

    let use_single_byte_for_cmap: bool;
    let (sub_type, font_tuple) = match &prepared.original.font_type {
        FontType::ParsedEmbeddedType0(buf) => {
            // WARNING: Font stream MAY NOT be compressed
            let font_stream = LoStream::new(
                LoDictionary::new(),
                buf.clone(),
            )
            .with_compression(false);
            use_single_byte_for_cmap = false;
            (
                "Type0",
                ("FontFile2", Reference(doc.add_object(font_stream))),
            )
        },
        FontType::ParsedEmbeddedType1C(buf) => {
            // WARNING: Font stream MAY NOT be compressed
            let font_stream = LoStream::new(
                LoDictionary::from_iter(vec![("Subtype", Name("Type1C".into()))]),
                buf.clone(),
            )
            .with_compression(false);
            use_single_byte_for_cmap = true;
            (
                "Type1",
                ("FontFile3", Reference(doc.add_object(font_stream))),
            )
        },
        _ => unimplemented!()
    };

    let mut font_vec = vec![
        ("Type", Name("Font".into())),
        ("Subtype", Name(sub_type.into())),
        ("BaseFont", Name(face_name.clone().into_bytes())),
    ];
    if let Some(ref cmap_bytes) = prepared.original.cmap_bytes {
        font_vec.push((
            "ToUnicode",
            Reference(doc.add_object(LoStream::new(
                LoDictionary::new(),
                cmap_bytes.clone(),
            )))
        ));
    } else if let Some(ref cmap) = prepared.original.cmap {
        font_vec.push((
            "ToUnicode",
            Reference(doc.add_object(LoStream::new(
                LoDictionary::new(),
                cmap.to_cmap_string(&face_name, use_single_byte_for_cmap).as_bytes().to_vec(),
            )))
        ));
    }
    if let Some(ref encoding) = prepared.original.font_properties.encoding {
        font_vec.push(( "Encoding", Name(encoding.clone().into_bytes())));
    }
    if let Some(ref custom_encoding) = prepared.original.font_properties.custom_encoding {
        let mut custom_encoding_vec = vec![
            ("Type", Name("Encoding".into())),
        ];
        if let Some(ref base_encoding) = custom_encoding.base_encoding {
            custom_encoding_vec.push(( "BaseEncoding", Name(base_encoding.clone().into_bytes())));
        }
        if let Some(ref differences) = custom_encoding.differences {
            custom_encoding_vec.push((
                "Differences",
                Array(differences.clone()),
            ));
        }
        font_vec.push((
            "Encoding",
            Reference(
                doc.add_object(LoDictionary::from_iter(custom_encoding_vec)),
            ),
        ));
    }
    if let Some(first_char) = prepared.original.font_properties.first_char {
        font_vec.push(( "FirstChar", Integer(first_char)));
    }
    if let Some(last_char) = prepared.original.font_properties.last_char {
        font_vec.push(( "LastChar", Integer(last_char)));
    }
    if let Some(ref widths) = prepared.original.font_properties.widths {
        font_vec.push((
            "Widths",
            Array(widths.clone()),
        ));
    }

    let mut font_descriptor_vec = vec![
        ("Type", Name("FontDescriptor".into())),
        ("FontName", Name(font_name.clone().into_bytes())),
        font_tuple,
    ];
    if let Some(ref stemv) = prepared.original.font_descriptor_properties.charset {
        font_descriptor_vec.push(( "CharSet", LoString(stemv.clone().into_bytes(), Literal)));
    }
    if let Some(ref font_family) = prepared.original.font_descriptor_properties.font_family {
        font_descriptor_vec.push(( "FontFamily", LoString(font_family.clone().into_bytes(), Literal)));
    }
    if let Some(ref font_stretch) = prepared.original.font_descriptor_properties.font_stretch {
        font_descriptor_vec.push(( "FontStretch", Name(font_stretch.clone().into_bytes())));
    }
    if let Some(cap_height) = prepared.original.font_descriptor_properties.cap_height {
        font_descriptor_vec.push(( "CapHeight", Integer(cap_height)));
    }
    if let Some(ascent) = prepared.original.font_descriptor_properties.ascent {
        font_descriptor_vec.push(( "Ascent", Integer(ascent)));
    }
    if let Some(descent) = prepared.original.font_descriptor_properties.descent {
        font_descriptor_vec.push(( "Descent", Integer(descent)));
    }
    if let Some(italic_angle) = prepared.original.font_descriptor_properties.italic_angle {
        font_descriptor_vec.push(( "ItalicAngle", Integer(italic_angle)));
    }
    if let Some(flags) = prepared.original.font_descriptor_properties.flags {
        font_descriptor_vec.push(( "Flags", Integer(flags)));
    }
    if let Some(font_weight) = prepared.original.font_descriptor_properties.font_weight {
        font_descriptor_vec.push(( "FontWeight", Integer(font_weight)));
    }
    if let Some(stemv) = prepared.original.font_descriptor_properties.stemv {
        font_descriptor_vec.push(( "StemV", Integer(stemv)));
    }
    if let Some(xheight) = prepared.original.font_descriptor_properties.xheight {
        font_descriptor_vec.push(( "XHeight", Integer(xheight)));
    }
    if let Some(ref font_bbox) = prepared.original.font_descriptor_properties.font_bbox {
        font_descriptor_vec.push(( "FontBBox", Array(font_bbox.clone())));
    }
    if let Some(ref cid_set) = prepared.original.font_descriptor_properties.cid_set {
        font_descriptor_vec.push((
            "CIDSet",
            Reference(doc.add_object(LoStream::new(lopdf::Dictionary::new(), cid_set.clone())))
        ));
    }

    if sub_type == "Type1" {
        font_vec.push((
            "FontDescriptor",
            Reference(
                doc.add_object(LoDictionary::from_iter(font_descriptor_vec)),
            ),
        ));
    } else if sub_type == "Type0" {
        if let Some(ref descendant_fonts) = prepared.original.font_properties.descendant_fonts {
            if !descendant_fonts.is_empty() {
                let mut descendant_fonts_vec = vec![
                    ("Type", Name("Font".into())),
                ];
                if let Some(ref base_font) = descendant_fonts[0].base_font {
                    descendant_fonts_vec.push(( "BaseFont", Name(base_font.clone().into_bytes())));
                }
                if let Some(ref subtype) = descendant_fonts[0].subtype {
                    descendant_fonts_vec.push(( "Subtype", Name(subtype.clone().into_bytes())));
                }
                if let Some(ref cid_to_gid_map) = descendant_fonts[0].cid_to_gid_map {
                    descendant_fonts_vec.push(( "CIDToGIDMap", Name(cid_to_gid_map.clone().into_bytes())));
                }
                if let Some(ref dw) = descendant_fonts[0].dw {
                    descendant_fonts_vec.push(( "DW", Integer(*dw)));
                }

                if let Some(ref cid_system_info) = descendant_fonts[0].cid_system_info {
                    let mut cid_system_info_vec = vec![];
                    if let Some(ref ordering) = cid_system_info.ordering {
                        cid_system_info_vec.push(( "Ordering", LoString(ordering.clone().into_bytes(), Literal)));
                    }
                    if let Some(ref registry) = cid_system_info.registry {
                        cid_system_info_vec.push(( "Registry", LoString(registry.clone().into_bytes(), Literal)));
                    }
                    if let Some(ref supplement) = cid_system_info.supplement {
                        cid_system_info_vec.push(( "DW", Integer(*supplement)));
                    }
                    descendant_fonts_vec.push((
                        "CIDSystemInfo",
                        Reference(
                            doc.add_object(LoDictionary::from_iter(cid_system_info_vec)),
                        ),
                    ));
                }

                descendant_fonts_vec.push((
                    "FontDescriptor",
                    Reference(
                        doc.add_object(LoDictionary::from_iter(font_descriptor_vec)),
                    ),
                ));
                font_vec.push((
                    "DescendantFonts",
                    Array(vec![Reference(
                        doc.add_object(LoDictionary::from_iter(descendant_fonts_vec)),
                    )]),
                ));
            }
        }
    }

    LoDictionary::from_iter(font_vec)
}

fn docinfo_to_dict(m: &PdfDocumentInfo) -> LoDictionary {
    let trapping = if m.trapped { "True" } else { "False" };
    let gts_pdfx_version = m.conformance.get_identifier_string();

    let info_mod_date = crate::utils::to_pdf_time_stamp_metadata(&m.modification_date);
    let info_create_date = crate::utils::to_pdf_time_stamp_metadata(&m.creation_date);

    let creation_date = LoString(info_create_date.into_bytes(), Literal);
    let identifier = LoString(m.identifier.as_bytes().to_vec(), Literal);

    let mut dict_vec = vec![
        ("Trapped", trapping.into()),
        ("CreationDate", creation_date),
        ("ModDate", LoString(info_mod_date.into_bytes(), Literal)),
        (
            "GTS_PDFXVersion",
            LoString(gts_pdfx_version.into(), Literal),
        ),
        ("Identifier", identifier),
    ];
    if !m.document_title.is_empty() {
        dict_vec.push(("Title", encode_text_to_utf16be(&m.document_title)));
    }
    if !m.author.is_empty() {
        dict_vec.push(("Author", encode_text_to_utf16be(&m.author)));
    }
    if !m.creator.is_empty() {
        dict_vec.push(("Creator", encode_text_to_utf16be(&m.creator)));
    }
    if !m.producer.is_empty() {
        dict_vec.push(("Producer", encode_text_to_utf16be(&m.producer)));
    }
    if !m.subject.is_empty() {
        dict_vec.push(("Subject", encode_text_to_utf16be(&m.subject)));
    }
    if !m.keywords.is_empty() {
        dict_vec.push(("Keywords", encode_text_to_utf16be(&m.keywords.join(","))));
    }
    LoDictionary::from_iter(dict_vec)
}

fn icc_to_stream(val: &IccProfile) -> LoStream {
    use lopdf::{Dictionary as LoDictionary, Object::*, Stream as LoStream};

    let (num_icc_fields, alternate) = match val.icc_type {
        IccProfileType::Cmyk => (4, "DeviceCMYK"),
        IccProfileType::Rgb => (3, "DeviceRGB"),
        IccProfileType::Greyscale => (1, "DeviceGray"),
    };

    let mut stream_dict = LoDictionary::from_iter(vec![
        ("N", Integer(num_icc_fields)),
        ("Length", Integer(val.icc.len() as i64)),
    ]);

    if val.has_alternate {
        stream_dict.set("Alternate", Name(alternate.into()));
    }

    if val.has_range {
        stream_dict.set(
            "Range",
            Array(vec![
                Real(0.0),
                Real(1.0),
                Real(0.0),
                Real(1.0),
                Real(0.0),
                Real(1.0),
                Real(0.0),
                Real(1.0),
            ]),
        );
    }

    LoStream::new(stream_dict, val.icc.clone())
}

fn link_annotation_to_dict(la: &LinkAnnotation, page_ids: &[lopdf::ObjectId]) -> LoDictionary {
    let ll = la.rect.lower_left();
    let ur = la.rect.upper_right();

    let mut dict: LoDictionary = LoDictionary::new();
    dict.set("Type", Name("Annot".into()));
    dict.set("Subtype", Name("Link".into()));
    dict.set(
        "Rect",
        Array(vec![Real(ll.x.0), Real(ll.y.0), Real(ur.x.0), Real(ur.y.0)]),
    );
    dict.set("A", Dictionary(actions_to_dict(&la.actions, page_ids)));
    dict.set(
        "Border",
        Array(la.border.to_array().into_iter().map(Real).collect()),
    );
    dict.set(
        "C",
        Array(
            color_array_to_f32(&la.color)
                .into_iter()
                .map(Real)
                .collect(),
        ),
    );
    dict.set("H", Name(la.highlighting.get_id().into()));
    dict
}

fn actions_to_dict(a: &Actions, page_ids: &[lopdf::ObjectId]) -> LoDictionary {
    let mut dict = LoDictionary::new();
    dict.set("S", Name(a.get_action_type_id().into()));
    match a {
        Actions::Goto(destination) => {
            dict.set("D", destination_to_obj(destination, page_ids));
        }
        Actions::Uri(uri) => {
            dict.set("URI", LoString(uri.clone().into_bytes(), Literal));
        }
    }
    dict
}

fn destination_to_obj(d: &Destination, page_ids: &[lopdf::ObjectId]) -> lopdf::Object {
    match d {
        Destination::Xyz {
            page,
            left,
            top,
            zoom,
        } => Array(vec![
            page_ids
                .get(page.saturating_sub(1))
                .copied()
                .map(Reference)
                .unwrap_or(Null),
            Name("XYZ".into()),
            left.map(Real).unwrap_or(Null),
            top.map(Real).unwrap_or(Null),
            zoom.map(Real).unwrap_or(Null),
        ]),
    }
}

fn color_array_to_f32(c: &ColorArray) -> Vec<f32> {
    match c {
        ColorArray::Transparent => Vec::new(),
        ColorArray::Gray(arr) => arr.to_vec(),
        ColorArray::Rgb(arr) => arr.to_vec(),
        ColorArray::Cmyk(arr) => arr.to_vec(),
    }
}

// Encode text to UTF-16BE with BOM
fn encode_text_to_utf16be(text: &str) -> lopdf::Object {
    if text.is_empty() {
        return lopdf::Object::string_literal("");
    }

    // Byte Order Mark
    let mut bytes = vec![0xFE, 0xFF];

    // Encode as UTF-16BE
    for c in text.encode_utf16() {
        bytes.push((c >> 8) as u8);
        bytes.push((c & 0xFF) as u8);
    }

    // Return as a Hex String
    lopdf::Object::String(bytes, Hexadecimal)
}
