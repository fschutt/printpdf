//! No-panic tests for the content stream interpreter: `parse_op` and the path, text and state
//! handling it drives. Every operator gets the wrong number and the wrong types of operands, the
//! state operators get unbalanced and very deep input, and hostile byte streams go through
//! `PdfDocument::parse`. A call has to return (no panic, no hang) and keep its memory bounded.

use std::collections::BTreeMap;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use lopdf::content::Operation;
use lopdf::{dictionary, Dictionary, Object, Stream, StringFormat};
use printpdf::cmap::CidCMap;
use printpdf::*;

const LIMIT: Duration = Duration::from_secs(5);

/// Runs `f` on its own thread. `Err` says that it panicked (with the message) or took longer than
/// [`LIMIT`], so a hang cannot hold up the build lock.
fn try_bounded<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(LIMIT) {
        Ok(value) => {
            let _ = handle.join();
            Ok(value)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!("no result after {LIMIT:?} (hang)")),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let why = match handle.join() {
                Err(payload) => payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
                    .unwrap_or_else(|| "unknown panic payload".to_string()),
                Ok(()) => "the thread ended without a result".to_string(),
            };
            Err(format!("panicked: {why}"))
        }
    }
}

/// [`try_bounded`] that fails the test on `Err`.
fn bounded<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    try_bounded(f).unwrap_or_else(|e| panic!("{what}: {e}"))
}

/// Fixed-seed xorshift, so the generated cases are the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

// ---------------------------------------------------------------------------------------------
// Building a document around a content stream
// ---------------------------------------------------------------------------------------------

/// A one-page PDF whose content stream is `content`; the page has the font resource `/F1`.
fn pdf_with_content(content: &[u8]) -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
        "Contents" => content_id,
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("a lopdf document saves to memory");
    bytes
}

struct Parsed {
    ops: Vec<Op>,
    warnings: Vec<PdfWarnMsg>,
}

/// Parses `content` as the only content stream of a page. With `consumers`, the parsed document
/// also goes through text extraction, SVG rendering and saving.
fn parse_now(content: &[u8], consumers: bool) -> Parsed {
    let bytes = pdf_with_content(content);
    let mut warnings = Vec::new();
    let doc = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings)
        .unwrap_or_else(|e| panic!("a PDF with a hostile content stream must parse: {e}"));
    assert_eq!(doc.pages.len(), 1);
    let ops = doc.pages[0].ops.clone();
    if consumers {
        let _ = doc.extract_text();
        let _ = doc.extract_text_boxes();
        let mut svg_warnings = Vec::new();
        let opts = PdfToSvgOptions::default();
        let _ = doc.pages[0].to_svg(&doc.resources, &opts, &mut svg_warnings);
        let mut save_warnings = Vec::new();
        let _ = doc.save(&PdfSaveOptions::default(), &mut save_warnings);
    }
    Parsed { ops, warnings }
}

/// [`parse_now`] on its own thread, failing the test on a panic or a hang.
fn parse_content(what: &str, content: Vec<u8>, consumers: bool) -> Parsed {
    bounded(what, move || parse_now(&content, consumers))
}

fn count_ops(ops: &[Op], pred: impl Fn(&Op) -> bool) -> usize {
    ops.iter().filter(|op| pred(op)).count()
}

fn polygon_points(ops: &[Op]) -> usize {
    ops.iter()
        .map(|op| match op {
            Op::DrawPolygon { polygon } => polygon.rings.iter().map(|r| r.points.len()).sum(),
            Op::DrawLine { line } => line.points.len(),
            _ => 0,
        })
        .sum()
}

fn count_warnings(warnings: &[PdfWarnMsg], needle: &str) -> usize {
    warnings.iter().filter(|w| w.msg.contains(needle)).count()
}

// ---------------------------------------------------------------------------------------------
// Calling parse_op directly
// ---------------------------------------------------------------------------------------------

const SIMPLE_TO_UNICODE: &str = "\
/CIDInit /ProcSet findresource begin 12 dict begin begincmap
/CMapName /Adobe-Identity-UCS def /CMapType 2 def
1 begincodespacerange <00> <FF> endcodespacerange
3 beginbfchar <41> <0058> <42> <00590059> <43> <D83DDE00> endbfchar
1 beginbfrange <61> <7A> <0061> endbfrange
endcmap end end";

const CID_TO_UNICODE: &str = "\
/CIDInit /ProcSet findresource begin 12 dict begin begincmap
1 begincodespacerange <0000> <FFFF> endcodespacerange
2 beginbfchar <0041> <0058> <8001> <00590059> endbfchar
1 beginbfrange <0100> <01FF> <0100> endbfrange
endcmap end end";

/// Codes of one byte (`00` to `7F`) and of two bytes (`8000` to `FFFF`), and a three-byte one.
const ENCODING_CMAP: &str = "\
/CIDInit /ProcSet findresource begin 12 dict begin begincmap
/CMapName /Test-H def /WMode 0 def
3 begincodespacerange <00> <7F> <8000> <FFFF> <A00000> <A0FFFF> endcodespacerange
2 begincidrange <00> <7F> 0 <8000> <80FF> 1000 endcidrange
2 begincidchar <8100> 5 <A00001> 6 endcidchar
endcmap end end";

/// A page state that knows one font of every decode kind, by resource name: `B1` builtin, `S0`
/// and `S1` simple (without and with ToUnicode), `C1` composite Identity-H, `C2` composite with
/// an encoding CMap, a ToUnicode CMap and a CID to glyph id map.
fn page_state() -> PageState {
    let simple = ToUnicodeCMap::parse(SIMPLE_TO_UNICODE).expect("the test CMap parses");
    let cid_unicode = ToUnicodeCMap::parse(CID_TO_UNICODE).expect("the test CMap parses");
    let (encoding, _) = CidCMap::parse(ENCODING_CMAP).expect("the test CMap parses");
    let mut font_decode = BTreeMap::new();
    font_decode.insert(
        "B1".to_string(),
        FontTextDecodeKind::Builtin(BuiltinFont::Helvetica),
    );
    font_decode.insert("S0".to_string(), FontTextDecodeKind::Simple { to_unicode: None });
    font_decode.insert(
        "S1".to_string(),
        FontTextDecodeKind::Simple { to_unicode: Some(Arc::new(simple)) },
    );
    font_decode.insert(
        "C1".to_string(),
        FontTextDecodeKind::Cid { to_unicode: None, encoding_cmap: None, cid_to_gid: None },
    );
    let mut cid_to_gid = BTreeMap::new();
    cid_to_gid.insert(5u16, 65535u16);
    cid_to_gid.insert(1000u16, 0u16);
    font_decode.insert(
        "C2".to_string(),
        FontTextDecodeKind::Cid {
            to_unicode: Some(Arc::new(cid_unicode)),
            encoding_cmap: Some(Arc::new(encoding)),
            cid_to_gid: Some(Arc::new(cid_to_gid)),
        },
    );
    PageState { font_decode, ..PageState::default() }
}

fn operation(name: &str, operands: Vec<Object>) -> Operation {
    Operation::new(name, operands)
}

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

fn string(bytes: &[u8]) -> Object {
    Object::String(bytes.to_vec(), StringFormat::Literal)
}

fn hex_string(bytes: &[u8]) -> Object {
    Object::String(bytes.to_vec(), StringFormat::Hexadecimal)
}

fn numbers(values: &[f32]) -> Vec<Object> {
    values.iter().map(|v| Object::Real(*v)).collect()
}

/// Runs `operations` through `parse_op`, carrying `state` from one to the next, and returns the
/// ops and the warnings. `parse_op` has no error exit, so an `Err` fails the test.
fn interpret(operations: &[Operation], state: &mut PageState) -> (Vec<Op>, Vec<PdfWarnMsg>) {
    let xobjects = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut out = Vec::new();
    for (op_id, operation) in operations.iter().enumerate() {
        match parse_op(0, op_id, operation, state, &xobjects, &mut warnings) {
            Ok(mut ops) => out.append(&mut ops),
            Err(e) => panic!("parse_op returned Err for `{}`: {e}", operation.operator),
        }
    }
    (out, warnings)
}

/// Every operator `parse_op` knows by name, and a few it does not.
const OPERATORS: &[&str] = &[
    "q", "Q", "MP", "CS", "cs", "ri", "rg", "RG", "k", "K", "g", "G", "TJ", "Tj", "T*", "TL",
    "Ts", "Tw", "Tz", "TD", "Tm", "Tc", "Tr", "gs", "BT", "ET", "Tf", "Td", "BDC", "BMC", "EMC",
    "cm", "m", "l", "c", "v", "y", "h", "re", "S", "f", "f*", "B", "B*", "b", "b*", "s", "n", "W",
    "W*", "w", "M", "j", "J", "d", "sc", "scn", "SC", "SCN", "'", "\"", "sh", "Do", "F", "BI",
    "ID", "EI", "d0", "d1", "BX", "EX", "i", "DP", "T", "Tf2", "", "é", "\u{0}",
];

/// The operand count the operator takes, for generating well-formed calls.
fn arity(operator: &str) -> usize {
    match operator {
        "rg" | "RG" | "sc" | "scn" | "SC" | "SCN" => 3,
        "k" | "K" | "v" | "y" | "re" => 4,
        "cm" | "Tm" | "c" => 6,
        "TD" | "Td" | "m" | "l" | "Tf" | "BDC" | "d" => 2,
        "\"" => 3,
        "q" | "Q" | "BT" | "ET" | "EMC" | "h" | "S" | "f" | "f*" | "B" | "B*" | "b" | "b*" | "s"
        | "n" | "W" | "W*" | "T*" => 0,
        _ => 1,
    }
}

/// `n` operands of one kind, indexed by `kind`.
fn operands_of_kind(kind: usize, n: usize) -> Vec<Object> {
    let names = ["C2", "OC", "S1", "B1", "Z", "C1", "S0"];
    (0..n)
        .map(|i| match kind {
            0 => Object::Integer(i as i64 + 1),
            1 => Object::Real(0.5 * (i as f32 + 1.0)),
            2 => name(names[i % names.len()]),
            3 => {
                if i % 2 == 0 {
                    string(b"AB")
                } else {
                    hex_string(&[0x00, 0x41, 0xFF])
                }
            }
            4 => Object::Array(vec![
                Object::Integer(1),
                string(b"A"),
                Object::Integer(-120),
                hex_string(&[0x00, 0x41, 0x42]),
                Object::Real(2.5),
                name("x"),
                Object::Array(vec![]),
            ]),
            5 => {
                let mut dict = Dictionary::new();
                dict.set("A", 1);
                dict.set("B", Object::Array(vec![Object::Null]));
                Object::Dictionary(dict)
            }
            6 => Object::Null,
            7 => Object::Boolean(i % 2 == 0),
            8 => Object::Reference((i as u32 + 1, 0)),
            9 => Object::Stream(Stream::new(dictionary! { "Length" => 0 }, vec![])),
            10 => Object::Real(
                [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX, f32::MIN, 0.0, -0.0]
                    [i % 7],
            ),
            11 => Object::Integer(
                [i64::MAX, i64::MIN, 0, -1, i64::from(i32::MAX) + 1, i64::from(i32::MIN) - 1][i % 6],
            ),
            _ => Object::Integer(0),
        })
        .collect()
}

const KINDS: usize = 13;

/// A random operand: any object kind, with extreme values.
fn random_operand(rng: &mut Rng) -> Object {
    let kind = rng.below(KINDS);
    let index = rng.below(14);
    operands_of_kind(kind, index + 1).pop().unwrap_or(Object::Null)
}

/// Selecting the font first makes the text operators run with each decode kind.
const FONT_CONTEXTS: &[Option<&str>] =
    &[None, Some("B1"), Some("S0"), Some("S1"), Some("C1"), Some("C2"), Some("Missing")];

fn state_in_font(font: Option<&str>) -> PageState {
    let mut state = page_state();
    if let Some(font) = font {
        let select = [
            operation("BT", vec![]),
            operation("Tf", vec![name(font), Object::Integer(12)]),
        ];
        interpret(&select, &mut state);
    }
    state
}

/// Every operator with 0 to 8 operands of every kind, in every font context.
#[test]
fn every_operator_survives_every_operand_shape() {
    bounded("operator x operand matrix", || {
        let mut calls = 0usize;
        for operator in OPERATORS {
            for font in FONT_CONTEXTS {
                let mut state = state_in_font(*font);
                let mut warnings = Vec::new();
                let xobjects = BTreeMap::new();
                for n in 0..=8 {
                    for kind in 0..KINDS {
                        let op = operation(operator, operands_of_kind(kind, n));
                        let res = parse_op(0, calls, &op, &mut state, &xobjects, &mut warnings);
                        assert!(res.is_ok(), "`{operator}` with {n} operands of kind {kind}");
                        calls += 1;
                    }
                }
            }
        }
        assert!(calls > 40_000);
    });
}

/// Random operator sequences: well-formed calls most of the time, so the path, text and state
/// code runs deep, and random garbage the rest of the time.
#[test]
fn random_operator_sequences_with_carried_state() {
    bounded("random sequences", || {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut total_ops = 0usize;
        let mut total_warnings = 0usize;
        for _ in 0..60 {
            let mut state = state_in_font(FONT_CONTEXTS[rng.below(FONT_CONTEXTS.len())]);
            let mut operations = Vec::new();
            for _ in 0..400 {
                let operator = OPERATORS[rng.below(OPERATORS.len())];
                let operands = if rng.below(10) < 6 {
                    // well-formed arity, numbers, with a name or string where the operator
                    // takes one
                    match operator {
                        "Tj" | "'" => vec![string(b"Hello \xff\x00 world")],
                        "TJ" => vec![operands_of_kind(4, 1).remove(0)],
                        "\"" => vec![Object::Real(1.0), Object::Real(2.0), hex_string(&[1, 2, 3])],
                        "Tf" => vec![name(["B1", "S1", "C1", "C2", "Zz"][rng.below(5)]), Object::Integer(10)],
                        "gs" | "Do" | "sh" | "cs" | "CS" | "MP" | "BMC" | "ri" => vec![name("N1")],
                        "BDC" => vec![name("Span"), Object::Dictionary(Dictionary::new())],
                        "d" => vec![Object::Array(numbers(&[3.0, 2.0])), Object::Integer(0)],
                        _ => {
                            let values: Vec<f32> = (0..arity(operator))
                                .map(|_| (rng.below(2000) as f32 - 1000.0) / 8.0)
                                .collect();
                            numbers(&values)
                        }
                    }
                } else {
                    let n = rng.below(9);
                    (0..n).map(|_| random_operand(&mut rng)).collect()
                };
                operations.push(operation(operator, operands));
            }
            let (ops, warnings) = interpret(&operations, &mut state);
            total_ops += ops.len();
            total_warnings += warnings.len();
        }
        assert!(total_ops > 1000, "the sequences must produce ops, got {total_ops}");
        assert!(total_warnings < 20 * 60 * 400, "warnings must stay a few per operator");
    });
}

// ---------------------------------------------------------------------------------------------
// Operators with the wrong operands, in a real content stream
// ---------------------------------------------------------------------------------------------

/// `cm` takes six operands. Fewer or more is skipped with a warning, and the stream goes on.
#[test]
fn cm_with_the_wrong_operand_count() {
    let content = b"\
1 0 0 cm
1 0 0 1 0 cm
1 0 0 1 0 0 0 cm
cm
/F1 cm
(a) (b) (c) (d) (e) (f) cm
1 0 0 1 5 5 cm
q 2 0 0 2 0 0 cm Q
1 0 0 cm 1 0 0 1 1 1 cm
0 0 1 rg"
        .to_vec();
    let parsed = parse_content("cm operands", content, false);
    let matrices = count_ops(&parsed.ops, |op| matches!(op, Op::SetTransformationMatrix { .. }));
    // the string operand version, `1 0 0 1 5 5`, `2 0 0 2 0 0` and the last `1 0 0 1 1 1`
    assert_eq!(matrices, 4);
    assert!(count_warnings(&parsed.warnings, "'cm' expects 6") >= 5, "{:#?}", parsed.warnings);
    assert!(matches!(parsed.ops.last(), Some(Op::SetFillColor { .. })));
}

/// The operand-count table: each line has too few or too many operands, or the wrong types. Each
/// line is skipped or read with defaults, and the last line still takes effect.
#[test]
fn operand_count_table_in_a_stream() {
    let lines = [
        "9 1 0 0 rg",
        "1 0 rg",
        "rg",
        "1 2 3 4 5 6 7 cm",
        "BT 5 Td ET",
        "BT 5 6 7 Td ET",
        "BT 5 TD ET",
        "BT 1 2 3 Tm ET",
        "BT /F1 Tf ET",
        "BT 12 /F1 Tf ET",
        "BT /F1 12 99 Tf ET",
        "BT Tj ET",
        "BT (a) (b) Tj ET",
        "BT 5 Tj ET",
        "BT [(a)] Tj ET",
        "BT (a) TJ ET",
        "BT TJ ET",
        "BT [(a) 5 (b)] [(c)] TJ ET",
        "BT (a) ' (b) (c) ' ' ET",
        "BT 1 (a) \" 1 2 \" 1 2 3 \" 1 2 (a) \" ET",
        "/P1 scn",
        "0.5 /Cs1 scn",
        "1 2 scn",
        "1 2 3 4 5 scn",
        "scn sc SC SCN",
        "[] 0 d",
        "[1 2 3] d",
        "[1 2 3] 0 1 d",
        "[-1 -2] -3 d",
        "[0 0] 0 d",
        "-1 w",
        "w M j J",
        "99999999999999999999.5 0 w",
        "7 j 7 J -1 j -1 J 1000000 j",
        "/Missing gs gs",
        "/Missing Do Do",
        "/Missing sh sh",
        "1 2 3 4 5 6 7 8 c 1 2 v 1 2 3 y",
        "1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 re",
        "0 0 m 1 2 3 l 7 re f",
        "/Span BDC EMC",
        "BDC EMC BMC EMC",
        "/OC BDC EMC",
        "/OC 1 BDC EMC",
        "1 2 BDC EMC",
        "/OC /L1 BDC EMC",
        "100000000000000000000000000000000000000000.0 0 m 1 1 l S",
        "1e38 1e38 1e38 1e38 re f",
        "0 0 1 rg",
    ];
    let content = lines.join("\n").into_bytes();
    let parsed = parse_content("operand count table", content, false);
    assert!(matches!(parsed.ops.last(), Some(Op::SetFillColor { .. })));
}

/// Unbalanced `Q`, `ET`, `EMC`, and `BT` twice.
#[test]
fn unbalanced_state_operators_in_a_stream() {
    let content = b"Q Q Q q Q Q ET ET BT BT ET EMC EMC /A BMC /B BMC EMC EMC EMC q q 0 0 1 rg"
        .to_vec();
    let parsed = parse_content("unbalanced", content, false);
    assert!(count_warnings(&parsed.warnings, "Q") >= 1, "{:#?}", parsed.warnings);
    assert!(count_warnings(&parsed.warnings, "EMC") >= 1);
    assert!(matches!(parsed.ops.last(), Some(Op::SetFillColor { .. })));
}

/// `h`, `l`, `c`, `v`, `y` before any `m`, paint operators on empty paths, `W` on nothing.
#[test]
fn path_operators_without_a_current_point() {
    let content = b"\
h h h S f B b s n W W* f* B* b*
5 5 l S
1 2 3 4 5 6 c f
1 2 3 4 v S
1 2 3 4 y B
h S
0 0 m h h h f
10 10 m 20 20 l 30 30 l h b
0 0 0 0 re W n
0 0 10 10 re 5 5 5 5 re f*
m l
0 0 1 rg"
        .to_vec();
    let parsed = parse_content("path operators", content, false);
    assert!(matches!(parsed.ops.last(), Some(Op::SetFillColor { .. })));
    assert!(polygon_points(&parsed.ops) > 0);
}

/// Dash arrays: empty, negative, not numbers, nested, and very long.
#[test]
fn dash_patterns_of_every_kind() {
    let mut content = Vec::new();
    content.extend_from_slice(b"[] 0 d [-1 -2 -3] -4 d [0] 0 d [(a) /b [1]] 1 d [1000000000000000000000000000000000000000.0 -1.5] 1 d ");
    content.extend_from_slice(b"0 0 m 100 100 l S ");
    content.extend_from_slice(b"[");
    for _ in 0..20_000 {
        content.extend_from_slice(b"1 ");
    }
    content.extend_from_slice(b"] 0 d 0 0 m 5 5 l S 0 0 1 rg");
    let parsed = parse_content("dash patterns", content, false);
    assert!(matches!(parsed.ops.last(), Some(Op::SetFillColor { .. })));
}

// ---------------------------------------------------------------------------------------------
// Stacks and the clip path
// ---------------------------------------------------------------------------------------------

/// A hundred thousand `q` do not grow the graphics state stack without bound, and the `Q` that
/// follow them still balance.
#[test]
fn graphics_state_stack_is_capped() {
    bounded("deep q", || {
        let depth = 100_000;
        let mut state = PageState::default();
        let saves: Vec<Operation> = (0..depth).map(|_| operation("q", vec![])).collect();
        let (ops, warnings) = interpret(&saves, &mut state);
        assert_eq!(count_ops(&ops, |op| matches!(op, Op::SaveGraphicsState)), depth);
        assert!(
            state.transform_stack.len() <= 1000,
            "the stack holds {} entries",
            state.transform_stack.len()
        );
        assert!(!warnings.is_empty(), "going past the limit must warn");
        assert!(warnings.len() < 100, "one warning per crossing, not one per `q`");

        let restores: Vec<Operation> = (0..depth).map(|_| operation("Q", vec![])).collect();
        let (ops, warnings) = interpret(&restores, &mut state);
        assert_eq!(count_ops(&ops, |op| matches!(op, Op::RestoreGraphicsState)), depth);
        assert_eq!(
            count_warnings(&warnings, "empty transform stack"),
            0,
            "balanced nesting must not warn about an unbalanced `Q`"
        );
        assert!(state.transform_stack.is_empty());

        // one `Q` too many still warns
        let (_, warnings) = interpret(&[operation("Q", vec![])], &mut state);
        assert_eq!(count_warnings(&warnings, "empty transform stack"), 1);
    });
}

/// `cm` inside the deepest `q` levels does not fail either.
#[test]
fn cm_in_deeply_nested_q() {
    bounded("deep q and cm", || {
        let mut state = PageState::default();
        let mut operations = Vec::new();
        for _ in 0..5_000 {
            operations.push(operation("q", vec![]));
            operations.push(operation("cm", numbers(&[2.0, 0.0, 0.0, 2.0, 1.0, 1.0])));
        }
        let (ops, _) = interpret(&operations, &mut state);
        assert_eq!(count_ops(&ops, |op| matches!(op, Op::SetTransformationMatrix { .. })), 5_000);
    });
}

/// The marked content stack is capped like the graphics state stack, and `EMC` still balances.
#[test]
fn marked_content_stack_is_capped() {
    bounded("deep BMC", || {
        let depth = 100_000;
        let mut state = PageState::default();
        let begins: Vec<Operation> = (0..depth)
            .map(|i| {
                if i % 2 == 0 {
                    operation("BMC", vec![name("Span")])
                } else {
                    operation("BDC", vec![name("Span"), Object::Dictionary(Dictionary::new())])
                }
            })
            .collect();
        let (ops, warnings) = interpret(&begins, &mut state);
        assert_eq!(
            count_ops(&ops, |op| matches!(
                op,
                Op::BeginMarkedContent { .. } | Op::BeginMarkedContentWithProperties { .. }
            )),
            depth
        );
        assert!(
            state.current_marked_content.len() <= 1000,
            "the stack holds {} entries",
            state.current_marked_content.len()
        );
        assert!(!warnings.is_empty(), "going past the limit must warn");
        assert!(warnings.len() < 100, "one warning per crossing, not one per operator");

        let ends: Vec<Operation> = (0..depth).map(|_| operation("EMC", vec![])).collect();
        let (ops, warnings) = interpret(&ends, &mut state);
        assert_eq!(count_ops(&ops, |op| matches!(op, Op::EndMarkedContent { .. })), depth);
        assert_eq!(count_warnings(&warnings, "no current_marked_content"), 0);
        assert!(state.current_marked_content.is_empty());

        let (ops, warnings) = interpret(&[operation("EMC", vec![])], &mut state);
        assert!(ops.is_empty());
        assert_eq!(count_warnings(&warnings, "no current_marked_content"), 1);
    });
}

/// `W` does not consume the path, so every further `W` on a growing path used to emit the whole
/// path again: n repetitions gave n squared points. The output has to stay linear in the input.
#[test]
fn repeated_clip_on_one_path_stays_linear() {
    let n = 2_000;
    let content = "0 0 m 1 1 l W\n".repeat(n).into_bytes();
    let input_len = content.len();
    let parsed = parse_content("repeated W", content, false);
    let points = polygon_points(&parsed.ops);
    assert!(
        parsed.ops.len() + points <= 16 * input_len + 1024,
        "{} ops and {points} points from {input_len} bytes",
        parsed.ops.len()
    );
    // the first clip is emitted, as for `W n`
    assert!(points >= 2);
}

/// The same with `W*`, with both mixed, and with a paint operator at the end that takes the
/// accumulated path once.
#[test]
fn repeated_clip_variants_stay_linear() {
    bounded("repeated W variants", || {
        let n = 3_000;
        let mut operations = Vec::new();
        for i in 0..n {
            operations.push(operation("m", numbers(&[0.0, 0.0])));
            operations.push(operation("l", numbers(&[1.0, 1.0])));
            operations.push(operation(["W", "W*"][i % 2], vec![]));
        }
        operations.push(operation("f", vec![]));
        let (ops, warnings) = interpret(&operations, &mut PageState::default());
        let points = polygon_points(&ops);
        assert!(points <= 4 * n + 16, "{points} points from {n} repetitions");
        assert!(!warnings.is_empty(), "the repeated clips must warn");
    });
}

/// `re W n` and `re W f` keep their meaning: the clip is emitted at `W`, and the fill follows.
#[test]
fn clip_then_paint_is_unchanged() {
    bounded("clip then paint", || {
        let rect = || operation("re", numbers(&[0.0, 0.0, 10.0, 10.0]));
        let (ops, warnings) = interpret(
            &[rect(), operation("W", vec![]), operation("n", vec![])],
            &mut PageState::default(),
        );
        assert!(warnings.is_empty(), "{warnings:#?}");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Op::DrawPolygon { polygon } => {
                assert_eq!(polygon.mode, PaintMode::Clip);
                assert_eq!(polygon.rings.len(), 1);
                assert_eq!(polygon.rings[0].points.len(), 4);
            }
            other => panic!("expected the clip polygon, got {other:?}"),
        }

        let (ops, warnings) = interpret(
            &[rect(), operation("W*", vec![]), operation("f", vec![])],
            &mut PageState::default(),
        );
        assert!(warnings.is_empty(), "{warnings:#?}");
        assert_eq!(ops.len(), 2);
        assert!(matches!(&ops[0], Op::DrawPolygon { polygon } if polygon.mode == PaintMode::Clip));
        assert!(matches!(&ops[1], Op::DrawPolygon { polygon } if polygon.mode == PaintMode::Fill));

        // `n` ended the first path: the next `W` on a new path clips again
        let (ops, warnings) = interpret(
            &[
                rect(),
                operation("W", vec![]),
                operation("n", vec![]),
                rect(),
                operation("W", vec![]),
                operation("n", vec![]),
            ],
            &mut PageState::default(),
        );
        assert!(warnings.is_empty(), "{warnings:#?}");
        assert_eq!(ops.len(), 2);
    });
}

/// The public path builder on its own, in every order.
#[test]
fn path_builder_in_every_order() {
    let p = |x: f32, y: f32| Point { x: Pt(x), y: Pt(y) };
    let modes = [PaintMode::Fill, PaintMode::Stroke, PaintMode::FillStroke, PaintMode::Clip];
    let windings = [WindingOrder::NonZero, WindingOrder::EvenOdd];
    let mut builders = Vec::new();
    for variant in 0..8 {
        let mut b = PathBuilder::default();
        match variant {
            0 => {}
            1 => b.close_path(),
            2 => b.line_to(p(1.0, 1.0)),
            3 => b.curve_to(p(1.0, 1.0), p(2.0, 2.0), p(3.0, 3.0)),
            4 => b.v_curve_to(p(1.0, 1.0), p(3.0, 3.0)),
            5 => b.y_curve_to(p(1.0, 1.0), p(3.0, 3.0)),
            6 => {
                b.move_to(p(0.0, 0.0));
                b.close_path();
                b.close_path();
                b.move_to(p(0.0, 0.0));
            }
            _ => {
                b.move_to(p(f32::NAN, f32::INFINITY));
                b.line_to(p(f32::NEG_INFINITY, f32::MAX));
                b.close_path();
            }
        }
        builders.push(b);
    }
    for b in &builders {
        for mode in modes {
            for winding in windings {
                let path = b.build(mode, winding);
                let _ = path_to_op(&path);
            }
        }
    }
    let mut b = PathBuilder::default();
    b.clear();
    b.clear();
    let _ = path_to_op(&b.build(PaintMode::Stroke, WindingOrder::NonZero));
    let _ = RectangleBuilder::default().get_ops();
    let mut r = RectangleBuilder::default();
    r.set_winding_order(WindingOrder::EvenOdd);
    r.new(Pt(0.0), Pt(0.0), Pt(f32::NAN), Pt(-1.0));
    r.set_winding_order(WindingOrder::EvenOdd);
    assert_eq!(r.get_ops().len(), 1);
    r.clear();
    assert!(r.get_ops().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Text-showing strings
// ---------------------------------------------------------------------------------------------

/// Strings for the show-text operators: empty, odd and even lengths, every byte value, and bytes
/// that are not valid in the font's encoding or codespace.
fn hostile_strings() -> Vec<Vec<u8>> {
    let mut strings: Vec<Vec<u8>> = vec![
        vec![],
        vec![0],
        vec![0xFF],
        vec![0x41],
        vec![0x00, 0x41],
        vec![0x00, 0x41, 0x00],
        vec![0x80],
        vec![0x80, 0x00],
        vec![0x80, 0x00, 0x7F],
        vec![0xA0, 0x00],
        vec![0xA0, 0x00, 0x01],
        vec![0xA0, 0x00, 0x01, 0x41],
        vec![0x81, 0x00, 0x81],
        vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        b"Hello, world".to_vec(),
        b"ABCabc".to_vec(),
        "naïve é€ 日本語".as_bytes().to_vec(),
        (0..=255u8).collect(),
        vec![0x41; 255],
        vec![0x00; 20_001],
    ];
    // each single byte and each pair of boundary bytes
    for b in [0x00, 0x01, 0x7F, 0x80, 0x81, 0xA0, 0xFE, 0xFF] {
        for c in [0x00, 0x01, 0x7F, 0x80, 0xA0, 0xFF] {
            strings.push(vec![b, c]);
            strings.push(vec![b, c, b]);
        }
    }
    strings
}

#[test]
fn show_text_strings_in_every_font_kind() {
    bounded("show text", || {
        let strings = hostile_strings();
        for font in FONT_CONTEXTS {
            let mut state = state_in_font(*font);
            let mut operations = Vec::new();
            for s in &strings {
                operations.push(operation("Tj", vec![string(s)]));
                operations.push(operation("Tj", vec![hex_string(s)]));
                operations.push(operation(
                    "TJ",
                    vec![Object::Array(vec![string(s), Object::Integer(-250), hex_string(s), Object::Real(f32::NAN)])],
                ));
                operations.push(operation("'", vec![string(s)]));
                operations.push(operation(
                    "\"",
                    vec![Object::Real(1.0), Object::Real(-1.0), string(s)],
                ));
            }
            let (ops, _) = interpret(&operations, &mut state);
            assert_eq!(ops.len(), operations.len(), "one op per show-text operator");
        }
    });
}

/// An odd-length string in an Identity-H font drops the trailing byte with a warning.
#[test]
fn odd_length_string_in_an_identity_font_warns() {
    bounded("odd identity string", || {
        let mut state = state_in_font(Some("C1"));
        let (ops, warnings) = interpret(&[operation("Tj", vec![string(&[0x00, 0x41, 0x00])])], &mut state);
        assert_eq!(ops.len(), 1);
        assert_eq!(count_warnings(&warnings, "odd byte length"), 1, "{warnings:#?}");
    });
}

/// Text before any `Tf`, and `Tf` naming a font that does not exist.
#[test]
fn text_without_a_usable_font() {
    bounded("no font", || {
        let mut state = PageState::default();
        let operations = [
            operation("Tj", vec![string(b"abc")]),
            operation("TJ", vec![Object::Array(vec![string(&[0, 1, 2]), Object::Integer(1)])]),
            operation("Tf", vec![name("Nope"), Object::Integer(12)]),
            operation("Tj", vec![string(&[0xFF, 0xFE])]),
            operation("Tf", vec![Object::Null, Object::Null]),
            operation("Tj", vec![string(b"")]),
        ];
        let (ops, _) = interpret(&operations, &mut state);
        assert!(!ops.is_empty());
    });
}

// ---------------------------------------------------------------------------------------------
// Bytes the tokenizer cannot read, and mutated streams
// ---------------------------------------------------------------------------------------------

/// Hostile byte streams. What the tokenizer drops is not the point here; the call has to return.
#[test]
fn hostile_byte_streams_parse() {
    let mut cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", vec![]),
        ("whitespace", b" \r\n\t\x0c ".to_vec()),
        ("nul bytes", vec![0; 64]),
        ("garbage prefix", b"\x00\xff\xfe\x80garbage 1 0 0 rg 0 0 10 10 re f".to_vec()),
        ("stray )", b"0 0 10 10 re f ) 1 0 0 rg 0 0 10 10 re f".to_vec()),
        ("stray }", b"0 0 10 10 re f } 1 0 0 rg 0 0 10 10 re f".to_vec()),
        ("stray {", b"0 0 10 10 re f { 1 0 0 rg 0 0 10 10 re f".to_vec()),
        ("stray >>", b"0 0 10 10 re f >> 1 0 0 rg 0 0 10 10 re f".to_vec()),
        ("stray ]", b"0 0 10 10 re f ] 1 0 0 rg 0 0 10 10 re f".to_vec()),
        ("unterminated string", b"BT (abc Tj ET 1 0 0 rg".to_vec()),
        ("unterminated hex", b"BT <ABC Tj ET 1 0 0 rg".to_vec()),
        ("bad hex", b"BT <AZ> Tj ET 1 0 0 rg".to_vec()),
        ("string escapes", b"(\\400) Tj (\\7) Tj (\\) Tj (a\\".to_vec()),
        ("string at eof", b"BT (a\\".to_vec()),
        ("number soup", b"--5 0 m +. 0 m 0.0.5 0 m 1-2 0 m .5 -.5 5. +7 m - . + 5 5 l".to_vec()),
        ("big integer", b"99999999999999999999 0 m 1 1 l S".to_vec()),
        ("exponent", b"1e38 1e38 1e38 1e38 re f 1E-5 w".to_vec()),
        ("name garbage", b"/A#zz gs /A# gs /A#41 gs /#00 gs / gs".to_vec()),
        ("operator with dash", b"T-x 1 0 0 rg d0 d1 BX EX".to_vec()),
        ("bi without id", b"0 0 10 10 re f BI /W 1 /H 1 0 1 0 rg".to_vec()),
        ("inline image", b"BI /W 1 /H 1 /BPC 8 /CS /G ID \x00 EI 1 0 0 rg".to_vec()),
        ("id without ei", b"BI /W 1 /H 1 /BPC 8 /CS /G ID \x00\x01\x02".to_vec()),
        ("bi last token", b"q Q BI".to_vec()),
        ("comments", b"% only a comment".to_vec()),
        ("comment then cr", b"% c\r1 0 0 rg\r0 0 5 5 re\rf\r".to_vec()),
        ("cr only", b"1 0 0 rg\r0 0 10 10 re\rf\r".to_vec()),
    ];
    let mut huge_digits = vec![b'9'; 400];
    huge_digits.extend_from_slice(b".0 0 m 1 1 l S");
    cases.push(("400 digit number", huge_digits));
    cases.push(("150 nested strings", {
        let mut v = b"BT ".to_vec();
        v.extend(std::iter::repeat(b'(').take(150));
        v.extend(std::iter::repeat(b')').take(150));
        v.extend_from_slice(b" Tj ET 1 0 0 rg");
        v
    }));
    cases.push(("150 nested arrays", {
        let mut v = Vec::new();
        v.extend(std::iter::repeat(b'[').take(150));
        v.extend(std::iter::repeat(b']').take(150));
        v.extend_from_slice(b" TJ 1 0 0 rg");
        v
    }));
    cases.push(("100000 open brackets", vec![b'['; 100_000]));
    cases.push(("100000 open dicts", b"<<".repeat(100_000)));
    cases.push(("100000 open parens", vec![b'('; 100_000]));
    cases.push(("1 MB name", {
        let mut v = b"/".to_vec();
        v.extend(std::iter::repeat(b'A').take(1_000_000));
        v.extend_from_slice(b" gs 1 0 0 rg");
        v
    }));
    cases.push(("100000 operands", {
        let mut v = "1 ".repeat(100_000).into_bytes();
        v.extend_from_slice(b"rg 1 0 0 1 0 0 cm");
        v
    }));
    cases.push(("100000 sc operands", {
        let mut v = "0.5 ".repeat(100_000).into_bytes();
        v.extend_from_slice(b"scn SCN");
        v
    }));
    cases.push(("100000 q", "q ".repeat(100_000).into_bytes()));
    cases.push(("100000 Q", "Q ".repeat(100_000).into_bytes()));
    cases.push(("100000 BMC", "/A BMC ".repeat(100_000).into_bytes()));
    cases.push(("100000 EMC", "EMC ".repeat(100_000).into_bytes()));
    cases.push(("100000 m", "0 0 m ".repeat(100_000).into_bytes()));
    cases.push(("100000 h", "0 0 m 1 1 l h ".repeat(100_000).into_bytes()));
    cases.push(("20000 W n", "0 0 10 10 re W n\n".repeat(20_000).into_bytes()));
    cases.push(("100000 Tf", "/F1 12 Tf ".repeat(100_000).into_bytes()));

    for (what, content) in cases {
        let parsed = parse_content(what, content, false);
        let _ = parsed.ops.len();
    }
}

/// A valid stream that uses most operators, for the mutations below.
const BASE_STREAM: &str = "\
q 0.5 0 0 0.5 10 10 cm /GS1 gs 1 0 0 rg 0 0 100 100 re f Q
BT /F1 12 Tf 50 700 Td (Hello) Tj 0 -14 Td [(W) -120 (orld)] TJ T* ET
0 0 m 10 10 l 20 0 20 10 30 10 c h S
[3 2] 0 d 2 w 1 J 1 j 4 M 0 0 1 RG 0 1 0 0 k 0.5 g 0.2 G
/Span << /MCID 0 >> BDC (x) Tj EMC
/OC /MC0 BDC EMC
q 10 10 50 50 re W n Q
BT 1 0 0 1 72 720 Tm /F1 9 Tf 14 TL (line) ' 1 2 (q) \" 0 Tr 2 Tc 3 Tw 90 Tz 4 Ts ET
/Im1 Do /Sh1 sh 1 2 3 4 v 5 6 7 8 y 0.1 0.2 0.3 sc 0.1 0.2 0.3 0.4 SCN
";

const TOKENS: &[&str] = &[
    "q", "Q", "BT", "ET", "BMC", "BDC", "EMC", "cm", "Tm", "Td", "TD", "Tf", "Tj", "TJ", "'", "\"",
    "m", "l", "c", "v", "y", "h", "re", "S", "f", "B", "b", "n", "W", "W*", "d", "sc", "scn",
    "rg", "k", "g", "gs", "Do", "sh", "BI", "ID", "EI", "0", "1", "-1", "0.5", "1e38",
    "99999999999999999999", "-0", ".", "-", "+", "--5", "(", ")", "<", ">", "<<", ">>", "[", "]",
    "{", "}", "/", "/F1", "/A#zz", "%", "\0", "\r", "\n", "\u{e9}", "\u{ff}",
];

#[test]
fn mutated_streams_parse() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for iteration in 0..300 {
        let mut bytes = BASE_STREAM.as_bytes().to_vec();
        for _ in 0..1 + rng.below(6) {
            if bytes.is_empty() {
                break;
            }
            let at = rng.below(bytes.len());
            match rng.below(6) {
                // delete a range
                0 => {
                    let end = (at + 1 + rng.below(40)).min(bytes.len());
                    bytes.drain(at..end);
                }
                // insert a token
                1 | 2 => {
                    let token = TOKENS[rng.below(TOKENS.len())];
                    let mut insert = format!(" {token} ").into_bytes();
                    if rng.below(4) == 0 {
                        insert = token.as_bytes().to_vec();
                    }
                    bytes.splice(at..at, insert);
                }
                // flip a byte
                3 => bytes[at] = (rng.next() & 0xFF) as u8,
                // repeat a range
                4 => {
                    let end = (at + 1 + rng.below(60)).min(bytes.len());
                    let piece = bytes[at..end].to_vec();
                    for _ in 0..1 + rng.below(8) {
                        bytes.splice(at..at, piece.iter().copied());
                    }
                }
                // cut the stream
                _ => bytes.truncate(at),
            }
        }
        let parsed = parse_content(&format!("mutation {iteration}"), bytes, false);
        let _ = parsed.ops.len();
    }
}

/// The valid base stream parses to the ops it should, so the mutation test starts from something
/// that works.
#[test]
fn base_stream_is_valid() {
    let parsed = parse_content("base stream", BASE_STREAM.as_bytes().to_vec(), false);
    assert!(parsed.ops.len() > 40, "{} ops", parsed.ops.len());
    assert_eq!(count_ops(&parsed.ops, |op| matches!(op, Op::SaveGraphicsState)), 2);
    assert_eq!(count_ops(&parsed.ops, |op| matches!(op, Op::RestoreGraphicsState)), 2);
}

// ---------------------------------------------------------------------------------------------
// What the parser hands on: the same streams through the consumers of parsed pages
// ---------------------------------------------------------------------------------------------

/// Parsed hostile pages go through text extraction, text boxes, SVG rendering and saving. A
/// failure here can sit in the consumer rather than in the parser; every failing stream is
/// listed.
#[test]
fn parsed_hostile_pages_go_through_the_consumers() {
    let mut streams: Vec<(String, Vec<u8>)> = vec![
        ("base stream".to_string(), BASE_STREAM.as_bytes().to_vec()),
        ("cm counts".to_string(), b"1 0 0 cm 1 0 0 1 0 cm cm 1 0 0 1 0 0 0 cm 0 0 1 rg".to_vec()),
        ("zero matrix".to_string(), b"0 0 0 0 0 0 cm BT /F1 12 Tf 0 0 0 0 0 0 Tm (a) Tj ET".to_vec()),
        ("unbalanced".to_string(), b"Q Q q Q ET ET BT BT ET EMC EMC /A BMC EMC EMC q q BT (a) Tj".to_vec()),
        ("huge numbers".to_string(), b"1000000000000000000000000000000000000000.0 0 m 1 1 l S \
            BT /F1 1000000000000000000000000000000000000000.0 Tf 1000000000000000000000000000000000000000.0 \
            1000000000000000000000000000000000000000.0 Td (a) Tj ET 1000000000000000000000000000000000000000.0 w"
            .to_vec()),
        ("negative sizes".to_string(), b"-1 w 0 0 -10 -10 re f BT /F1 -12 Tf (a) Tj ET 0 Tz BT /F1 12 Tf 0 Tz (a) Tj ET".to_vec()),
        ("dash".to_string(), b"[] 0 d [-1 -2] -3 d [0 0] 0 d 0 0 m 9 9 l S [1000000000000000000000000000000000000000.0] 0 d 0 0 m 9 9 l S".to_vec()),
        ("clip".to_string(), "0 0 m 1 1 l W\n".repeat(300).into_bytes()),
        ("deep q".to_string(), "q 2 0 0 2 0 0 cm\n".repeat(400).into_bytes()),
        ("deep marked content".to_string(), "/Span BMC (x) Tj\n".repeat(400).into_bytes()),
        ("text states".to_string(), b"BT /F1 12 Tf 1 0 0 1 50 50 Tm 5 TL T* T* (a) ' 1 2 (b) \" 3 Tr 2 Tc 3 Tw 50 Tz 4 Ts [(a) -1000000 (b) 1000000] TJ ET".to_vec()),
    ];
    for (i, line) in [
        "9 1 0 0 rg", "1 0 rg", "1 2 3 4 5 6 7 cm", "BT 5 Td ET", "BT 5 6 7 Td ET", "BT 1 2 3 Tm ET",
        "BT /F1 Tf ET", "BT 12 /F1 Tf (a) Tj ET", "BT (a) TJ ET", "BT [(a)] Tj ET", "/P1 scn", "1 2 scn",
        "1 2 3 4 5 scn", "[1 2 3] d", "-1 w", "/Span BDC EMC", "/OC 1 BDC EMC", "/OC /L1 BDC (a) Tj EMC /OC /L1 BDC EMC",
        "/Missing gs /Missing Do /Missing sh", "0 0 m h h f", "1 2 3 4 5 6 c f",
    ]
    .iter()
    .enumerate()
    {
        streams.push((format!("line {i}: {line}"), line.as_bytes().to_vec()));
    }

    let mut failures = Vec::new();
    for (what, content) in streams {
        if let Err(e) = try_bounded(move || parse_now(&content, true).ops.len()) {
            failures.push(format!("{what}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{} streams failed:\n{}", failures.len(), failures.join("\n"));
}
