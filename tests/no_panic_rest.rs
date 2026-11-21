//! Nothing in date.rs, serialize.rs, forms.rs, text_merge.rs, html/ and the JSON API may panic,
//! hang or run out of memory, whatever the input.
//!
//! Tests that can abort the process (a stack overflow), spin forever or allocate without bound run
//! in a child process of this test binary that the parent kills after a time limit or when its
//! resident memory passes a limit, so a failure is a failed test and not a stuck machine.

use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use printpdf::*;

const CHILD_ENV: &str = "PRINTPDF_NO_PANIC_REST_CHILD";

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn resident_mb(pid: u32) -> Option<u64> {
    let out = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let kb: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(kb / 1024)
}

/// Runs the test `name` of this binary again in a child process (with `CHILD_ENV` set) and checks
/// that it ended well. The child is killed after `secs` seconds or when it uses more than
/// `max_mb` MB. Returns `true` in the parent, `false` in the child, which then runs the test body.
fn in_child_process(name: &str, secs: u64, max_mb: u64) -> bool {
    if std::env::var_os(CHILD_ENV).is_some() {
        return false;
    }
    let exe = std::env::current_exe().expect("the test executable");
    let mut child = Command::new(exe)
        .args(["--exact", name, "--test-threads=1", "--nocapture", "--include-ignored"])
        .env(CHILD_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the child process");
    let mut out = child.stdout.take().expect("piped stdout");
    let mut err = child.stderr.take().expect("piped stderr");
    let out_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = out.read_to_end(&mut bytes);
        bytes
    });
    let err_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = err.read_to_end(&mut bytes);
        bytes
    });

    let start = Instant::now();
    let ended = loop {
        if let Some(status) = child.try_wait().expect("wait for the child") {
            break Ok(status);
        }
        if start.elapsed() > Duration::from_secs(secs) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(format!("no result after {secs} s (a hang)"));
        }
        if let Some(mb) = resident_mb(child.id()) {
            if mb > max_mb {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!("{mb} MB of memory in use (limit {max_mb} MB)"));
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let stdout = String::from_utf8_lossy(&out_reader.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err_reader.join().unwrap_or_default()).into_owned();
    let tail: String = stderr.chars().rev().take(1500).collect::<Vec<_>>().into_iter().rev().collect();
    match ended {
        Ok(status) if status.success() => {
            assert!(stdout.contains("running 1 test"), "{name}: the child did not run the test:\n{stdout}");
        }
        Ok(status) => panic!("{name}: the child process ended with {status}\n{tail}"),
        Err(why) => panic!("{name}: {why}\n{tail}"),
    }
    true
}

// ---------------------------------------------------------------------------------------------
// date.rs
// ---------------------------------------------------------------------------------------------

mod date_cases {
    use std::str::FromStr;

    use printpdf::date::*;

    use super::xorshift;

    #[test]
    fn datetime_from_str_with_a_multibyte_first_offset_character_is_an_error() {
        for s in [
            "2017-05-05 15:02:24.000 \u{e9}2:00:00",
            "2017-05-05 15:02:24 \u{20ac}",
            "2017-05-05 15:02 \u{1F600}\u{1F600}",
        ] {
            assert!(DateTime::from_str(s).is_err(), "{s:?}");
        }
    }

    #[test]
    fn offset_json_with_a_multibyte_first_character_is_an_error() {
        for s in ["\"\u{e9}2:00:00.000\"", "\"\u{20ac}\"", "\"\u{1F600}x\""] {
            assert!(serde_json::from_str::<Offset>(s).is_err(), "Offset {s}");
            assert!(serde_json::from_str::<UtcOffset>(s).is_err(), "UtcOffset {s}");
        }
    }

    #[test]
    fn offset_parts_that_do_not_fit_once_the_sign_is_applied_are_errors() {
        // `-` and then `-128`: the sign turns -128 into 128, which is not an i8.
        assert!(DateTime::from_str("2017-05-05 15:02:24 --128").is_err());
        assert!(DateTime::from_str("2017-05-05 15:02:24 --00:-128").is_err());
        assert!(DateTime::from_str("2017-05-05 15:02:24 --00:00:-128").is_err());
        assert!(serde_json::from_str::<UtcOffset>("\"--128:00:00\"").is_err());
        assert!(serde_json::from_str::<UtcOffset>("\"-00:-128:00\"").is_err());
        assert!(serde_json::from_str::<Offset>("\"--128:00:00.000\"").is_err());
        assert!(serde_json::from_str::<Offset>("\"-00:00:-128.000\"").is_err());
        assert!(serde_json::from_str::<Offset>("\"-00:00:00.-32768\"").is_err());
    }

    #[test]
    fn the_extreme_offsets_serialize_without_overflow() {
        let offset = Offset {
            hours: i8::MIN,
            minutes: i8::MIN,
            seconds: i8::MIN,
            milliseconds: i16::MIN,
        };
        let json = serde_json::to_string(&offset).expect("serialize");
        assert_eq!(json, "\"-128:128:128.32768\"");
        let utc = UtcOffset {
            hours: i8::MIN,
            minutes: i8::MIN,
            seconds: i8::MIN,
        };
        serde_json::to_string(&utc).expect("serialize");
        let dt = DateTime {
            date: Date { year: 2017, month: 5, day: 5 },
            time: Time { hour: 15, minute: 2, second: 24, millisecond: 0 },
            offset,
        };
        let _ = dt.to_string();
        let _ = dt.unix_timestamp();
        let _ = serde_json::to_string(&dt).expect("serialize");
    }

    #[test]
    fn pdf_dates_that_are_empty_truncated_or_odd_do_not_panic() {
        let valid = "D:20170505150224+02'00'00'";
        for end in 0..=valid.len() {
            let _ = parse_pdf_date(&valid[..end]);
            let _ = parse_pdf_date(&valid[end.min(2)..end]);
        }
        // A multibyte character at every position.
        for at in 0..valid.len() {
            let mut s = valid.to_string();
            s.replace_range(at..at + 1, "\u{e9}");
            let _ = parse_pdf_date(&s);
            let mut s = valid.to_string();
            s.replace_range(at..at + 1, "\u{1F600}");
            let _ = parse_pdf_date(&s);
        }
        for tz in [
            "+99'99'99'", "-99'99'99'", "+9", "-9'", "Z", "z00'00'", "+0200", "+24'00'", "+00'60'",
            "+-9'-9'", "-+9'+9'", "+", "-", "'", "+'", "+00", "+00'", "+00'0", "+00'00'0",
        ] {
            let s = format!("D:20170505150224{tz}");
            if let Ok(dt) = parse_pdf_date(&s) {
                let _ = dt.unix_timestamp();
                let _ = dt.to_string();
                let _ = serde_json::to_string(&dt);
                let _ = dt.offset();
            }
        }
        assert!(parse_pdf_date("").is_err());
        assert!(parse_pdf_date("D:").is_err());
        assert!(parse_pdf_date("D:2017").is_err());
        assert!(parse_pdf_date("D:2017050515022").is_err());
        assert!(parse_pdf_date("D:20170505150224").is_ok());
    }

    #[test]
    fn dates_outside_the_supported_range_do_not_panic() {
        for s in [
            "12345-01-01 00:00:00 +00:00:00",
            "2147483647-02-29 00:00:00 +00:00:00",
            "99999999999-01-01 00:00:00 +00:00:00",
            "0000-00-00 00:00:00 +00:00:00",
            "2017-13-01 00:00:00 +00:00:00",
            "2017-12-32 00:00:00 +00:00:00",
            "2017-12-31 24:00:00 +00:00:00",
            "2017-12-31 23:60:00 +00:00:00",
            "2017-12-31 23:59:60 +00:00:00",
            "2017-12-31 23:59:59.1000 +00:00:00",
            "2017-12-31 23:59:59 +127:127:127",
            "2017-12-31 23:59:59 -127:-127:-127",
            "2017-12-31 23:59 +99",
        ] {
            if let Ok(dt) = s.parse::<DateTime>() {
                let _ = dt.unix_timestamp();
                let _ = dt.to_string();
                let _ = serde_json::to_string(&dt);
            }
        }
        let big = DateTime {
            date: Date { year: i32::MAX, month: 12, day: 31 },
            time: Time { hour: 255, minute: 255, second: 255, millisecond: u16::MAX },
            offset: Offset { hours: i8::MAX, minutes: i8::MAX, seconds: i8::MAX, milliseconds: i16::MAX },
        };
        assert_eq!(big.unix_timestamp(), 0, "an invalid date counts as the epoch");
        let _ = big.to_string();
        let _ = serde_json::to_string(&big);
        let small = DateTime {
            date: Date { year: i32::MIN, month: 0, day: 0 },
            time: Time { hour: 0, minute: 0, second: 0, millisecond: 0 },
            offset: Offset { hours: i8::MIN, minutes: i8::MIN, seconds: i8::MIN, milliseconds: i16::MIN },
        };
        assert_eq!(small.unix_timestamp(), 0);
        let _ = small.to_string();
        let _ = small.month();
        let _ = serde_json::to_string(&small);
        for ts in [0, 1, -1, i64::MAX, i64::MIN, 253_402_300_800, -377_705_116_801, 1 << 40] {
            let _ = DateTime::from_unix_timestamp(ts);
        }
        assert!(DateTime::from_unix_timestamp(i64::MAX).is_err());
        assert!(DateTime::from_unix_timestamp(i64::MIN).is_err());
        let _ = DateTime::now();
        let _ = DateTime::now_utc();
    }

    /// Mutations of valid strings (inserted, deleted, truncated, multibyte, huge numbers) go
    /// through every parser of the module.
    #[test]
    fn mutated_date_strings_do_not_panic() {
        const PIECES: &[&str] = &[
            "0", "1", "2", "9", "-", "+", ":", ".", " ", "'", "D", "Z", "\u{e9}", "\u{20ac}",
            "\u{1F600}", "\u{0}", "128", "-128", "32768", "-32768", "999999999999",
        ];
        const SEEDS: &[&str] = &[
            "2017-05-05 15:02:24.000 +02:00:00",
            "2017-05-05",
            "15:02:24.000",
            "+02:00:00.000",
            "-05:30:00",
            "D:20170505150224+02'00'",
            "2017-05-05 15:02 -05:30",
        ];
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for i in 0..300usize {
            let seed = SEEDS[i % SEEDS.len()];
            let mut chars: Vec<char> = seed.chars().collect();
            for _ in 0..(1 + xorshift(&mut state) % 4) {
                let piece = PIECES[(xorshift(&mut state) % PIECES.len() as u64) as usize];
                let at = (xorshift(&mut state) % (chars.len() as u64 + 1)) as usize;
                match xorshift(&mut state) % 3 {
                    0 => {
                        for (k, c) in piece.chars().enumerate() {
                            chars.insert((at + k).min(chars.len()), c);
                        }
                    }
                    1 => {
                        if at < chars.len() {
                            chars.remove(at);
                        }
                    }
                    _ => chars.truncate(at),
                }
            }
            let s: String = chars.into_iter().collect();
            let _ = parse_pdf_date(&s);
            if let Ok(dt) = DateTime::from_str(&s) {
                let _ = dt.unix_timestamp();
                let _ = dt.to_string();
            }
            let json = serde_json::to_string(&s).expect("a string is JSON");
            let _ = serde_json::from_str::<DateTime>(&json);
            let _ = serde_json::from_str::<Date>(&json);
            let _ = serde_json::from_str::<Time>(&json);
            if let Ok(offset) = serde_json::from_str::<Offset>(&json) {
                let _ = serde_json::to_string(&offset);
            }
            if let Ok(offset) = serde_json::from_str::<UtcOffset>(&json) {
                let _ = serde_json::to_string(&offset);
            }
        }
    }

    #[test]
    fn valid_dates_parse_and_format_as_before() {
        let dt = parse_pdf_date("D:20170505150224+02'30'").expect("valid");
        assert_eq!(dt.to_string(), "2017-05-05 15:02:24.000 +02:30:00");
        assert_eq!(dt.to_string().parse::<DateTime>(), Ok(dt));
        let dt = parse_pdf_date("D:20170505150224-05'30'").expect("valid");
        assert_eq!(dt.offset().whole_hours(), -5);
        assert_eq!(dt.offset().minutes_past_hour(), -30);
        let dt: DateTime = "2017-05-05 15:02 -05:30".parse().expect("valid");
        assert_eq!((dt.time.second, dt.time.millisecond), (0, 0));
        assert_eq!((dt.offset.hours, dt.offset.minutes), (-5, -30));
        let off: Offset = serde_json::from_str("\"-05:30:15.250\"").expect("valid");
        assert_eq!(off, Offset { hours: -5, minutes: -30, seconds: -15, milliseconds: -250 });
        assert_eq!(serde_json::to_string(&off).expect("serialize"), "\"-05:30:15.250\"");
        let utc: UtcOffset = serde_json::from_str("\"+02:00:30\"").expect("valid");
        assert_eq!((utc.hours, utc.minutes, utc.seconds), (2, 0, 30));
        assert_eq!(parse_pdf_date("D:20240229120000").map(|d| d.day()), Ok(29));
        assert!(parse_pdf_date("D:20230229120000").is_err());
        assert!(parse_pdf_date("D:20231331120000").is_err());
    }
}

// ---------------------------------------------------------------------------------------------
// serialize.rs and text_merge.rs
// ---------------------------------------------------------------------------------------------

const MOCK_TTF: &[u8] = include_bytes!("./assets/fonts/mock/mock_ttf.ttf");

fn lp(x: f32, bezier: bool) -> LinePoint {
    LinePoint { p: Point::new(Mm(x), Mm(x)), bezier }
}

fn save_all_ways(doc: &PdfDocument, what: &str) {
    for optimize in [true, false] {
        for secure in [true, false] {
            for subset_fonts in [true, false] {
                let opts = PdfSaveOptions { optimize, secure, subset_fonts, ..Default::default() };
                let mut warnings = Vec::new();
                let bytes = doc.save(&opts, &mut warnings);
                assert!(bytes.starts_with(b"%PDF-"), "{what}: not a PDF");
                lopdf::Document::load_mem(&bytes)
                    .unwrap_or_else(|e| panic!("{what}: the saved file does not load: {e}"));
            }
        }
    }
}

#[test]
fn serialize_ops_naming_unregistered_resources_are_saved() {
    if in_child_process("serialize_ops_naming_unregistered_resources_are_saved", 60, 1500) {
        return;
    }
    let missing_font = PdfFontHandle::External(FontId("missing-font".into()));
    let ops = vec![
        Op::SetColorSpaceFill { id: "missing-cs".into() },
        Op::SetColorSpaceStroke { id: "missing-cs".into() },
        Op::LoadGraphicsState { gs: ExtendedGraphicsStateId("missing-gs".into()) },
        Op::PaintShading { id: ShadingId("missing-shading".into()) },
        Op::UseXobject { id: XObjectId("missing-xobject".into()), transform: XObjectTransform::default() },
        Op::UseXobject {
            id: XObjectId("missing-xobject".into()),
            transform: XObjectTransform { no_auto_scale: true, scale_x: Some(0.0), dpi: Some(0.0), ..Default::default() },
        },
        Op::BeginLayer { layer_id: LayerInternalId("missing-layer".into()) },
        Op::EndLayer,
        Op::BeginOptionalContent { layer_id: LayerInternalId("missing-layer".into()) },
        Op::EndOptionalContent,
        Op::SetFont { font: missing_font, size: Pt(12.0) },
        Op::StartTextSection,
        Op::ShowText { items: vec![TextItem::Text("hello".into())] },
        Op::ShowText {
            items: vec![TextItem::GlyphIds(vec![
                Codepoint::new(7, 0.0),
                Codepoint::with_cid(u16::MAX, 1.0, "x".into()),
            ])],
        },
        Op::MoveToNextLineShowText { text: "x".into() },
        Op::SetSpacingMoveAndShowText { word_spacing: 1.0, char_spacing: 1.0, text: "y".into() },
        Op::SetTextMatrix { matrix: TextMatrix::Raw([1.0, 0.0, 0.0, 1.0, 5.0, 5.0]) },
        Op::ShowText { items: vec![TextItem::GlyphIds(vec![Codepoint::new(3, 0.0)])] },
        Op::EndTextSection,
        Op::EndMarkedContent,
        Op::RestoreGraphicsState,
        Op::RestoreGraphicsState,
    ];
    let mut doc = PdfDocument::new("dangling ids");
    doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
    save_all_ways(&doc, "unregistered resources");

    // An embedded font that is registered but never shown, and a shown glyph that is not in it.
    let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).expect("mock font parses");
    let unused = doc.add_font(&font);
    let used = doc.add_font(&font);
    let mut ops = vec![
        Op::SetFont { font: PdfFontHandle::External(unused), size: Pt(10.0) },
        Op::SetFont { font: PdfFontHandle::External(used), size: Pt(10.0) },
        Op::StartTextSection,
    ];
    for gid in [0u16, 1, 2, 3, 200, u16::MAX] {
        ops.push(Op::ShowText { items: vec![TextItem::GlyphIds(vec![Codepoint::new(gid, 0.0)])] });
    }
    ops.push(Op::EndTextSection);
    doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
    save_all_ways(&doc, "an unshown font and glyphs outside it");
}

#[test]
fn serialize_paths_with_odd_point_lists_are_saved() {
    if in_child_process("serialize_paths_with_odd_point_lists_are_saved", 60, 1500) {
        return;
    }
    let lines = vec![
        vec![],
        vec![lp(1.0, true)],
        vec![lp(1.0, false), lp(2.0, true)],
        vec![lp(1.0, true), lp(2.0, true), lp(3.0, true)],
        vec![lp(0.0, false), lp(1.0, true), lp(2.0, true)],
        vec![lp(0.0, false), lp(1.0, true), lp(2.0, false), lp(3.0, true)],
        vec![lp(0.0, false), lp(1.0, true), lp(2.0, true), lp(3.0, true), lp(4.0, true), lp(5.0, true)],
    ];
    let mut ops = Vec::new();
    for points in lines {
        ops.push(Op::DrawLine { line: Line { points: points.clone(), is_closed: true } });
        ops.push(Op::DrawLine { line: Line { points: points.clone(), is_closed: false } });
        for mode in [PaintMode::Clip, PaintMode::Fill, PaintMode::Stroke, PaintMode::FillStroke] {
            ops.push(Op::DrawPolygon {
                polygon: Polygon {
                    rings: vec![PolygonRing { points: points.clone() }, PolygonRing { points: points.clone() }],
                    mode,
                    winding_order: WindingOrder::EvenOdd,
                },
            });
        }
    }
    ops.push(Op::DrawPolygon {
        polygon: Polygon { rings: vec![], mode: PaintMode::Fill, winding_order: WindingOrder::NonZero },
    });
    let mut doc = PdfDocument::new("paths");
    doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
    save_all_ways(&doc, "paths");
}

fn actual_text_span(units: &[u8]) -> Op {
    let mut data = vec![0xFE, 0xFF];
    data.extend_from_slice(units);
    let mut map = std::collections::BTreeMap::new();
    map.insert("ActualText".to_string(), DictItem::String { data, literal: false });
    Op::BeginMarkedContentWithProperties { tag: "Span".into(), properties: DictItem::Dict { map } }
}

fn random_ops(font: &FontId, state: &mut u64) -> Vec<Op> {
    let count = (xorshift(state) % 40) as usize;
    let mut ops = Vec::new();
    for _ in 0..count {
        let pick = xorshift(state) % 26;
        let y = (xorshift(state) % 3) as f32 * 14.0;
        let gid = [0u16, 1, 2, 3, 4, 5, 200, u16::MAX][(xorshift(state) % 8) as usize];
        ops.push(match pick {
            0 => Op::StartTextSection,
            1 | 2 => Op::EndTextSection,
            3 => Op::SetFont { font: PdfFontHandle::External(font.clone()), size: Pt(10.0) },
            4 => Op::SetFont { font: PdfFontHandle::External(FontId("missing".into())), size: Pt(10.0) },
            5 => Op::SetFont { font: PdfFontHandle::Builtin(BuiltinFont::Courier), size: Pt(0.0) },
            6 | 7 => Op::SetTextMatrix {
                matrix: TextMatrix::Raw([1.0, 0.0, 0.0, 1.0, 10.0 * pick as f32, 700.0 - y]),
            },
            8 => Op::SetTextMatrix { matrix: TextMatrix::Raw([2.0, 0.0, 0.0, 2.0, 1.0, y]) },
            9 | 10 => Op::ShowText {
                items: vec![TextItem::GlyphIds(vec![Codepoint::with_cid(gid, 0.0, "a".into())])],
            },
            11 => Op::ShowText { items: vec![TextItem::Text("ab".into())] },
            12 => Op::ShowText { items: vec![] },
            13 => Op::ShowText {
                items: vec![TextItem::Offset(-120.0), TextItem::GlyphIds(vec![Codepoint::new(gid, 3.0)])],
            },
            14 => Op::SetCharacterSpacing { multiplier: 1.0 },
            15 => Op::SetHorizontalScaling { percent: 50.0 },
            16 | 17 => actual_text_span(&[0, 65, 0, 66]),
            18 => actual_text_span(&[0]),
            19 => Op::BeginMarkedContent { tag: "P".into() },
            20 | 21 => Op::EndMarkedContent,
            22 => Op::EndMarkedContentWithProperties,
            23 => Op::SaveGraphicsState,
            24 => Op::RestoreGraphicsState,
            _ => Op::LoadGraphicsState { gs: ExtendedGraphicsStateId("gs".into()) },
        });
    }
    ops
}

/// Random op sequences (unbalanced text sections and marked content, runs the glyph-run and
/// text-object mergers combine, glyphs outside the font) are saved.
#[test]
fn serialize_random_op_sequences_are_saved() {
    if in_child_process("serialize_random_op_sequences_are_saved", 120, 1500) {
        return;
    }
    let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).expect("mock font parses");
    let mut state = 0xD1B5_4A32_D192_ED03u64;
    for round in 0..300 {
        let mut doc = PdfDocument::new("random");
        let font_id = doc.add_font(&font);
        let ops = random_ops(&font_id, &mut state);
        doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops.clone()));
        let optimize = round % 2 == 0;
        let opts = PdfSaveOptions { optimize, subset_fonts: round % 3 != 0, ..Default::default() };
        let bytes = doc.save(&opts, &mut Vec::new());
        assert!(bytes.starts_with(b"%PDF-"), "round {round}: {ops:?}");
    }
}

#[test]
fn serialize_documents_with_extreme_dates_and_every_conformance_level_are_saved() {
    if in_child_process("serialize_documents_with_extreme_dates_and_every_conformance_level_are_saved", 120, 1500) {
        return;
    }
    let dates = [
        DateTime::epoch(),
        "9999-12-31 23:59:59.999 +127:127:127".parse::<DateTime>().expect("valid"),
        "0000-01-01 00:00:00.000 -127:-127:-127".parse::<DateTime>().expect("valid"),
        "2147483647-12-31 23:59:59.999 +00:00:00".parse::<DateTime>().expect("valid"),
        DateTime {
            date: Date { year: i32::MIN, month: 0, day: 0 },
            time: Time { hour: u8::MAX, minute: u8::MAX, second: u8::MAX, millisecond: u16::MAX },
            offset: Offset { hours: i8::MIN, minutes: i8::MIN, seconds: i8::MIN, milliseconds: i16::MIN },
        },
    ];
    let levels = [
        PdfConformance::default(),
        PdfConformance::A1B_2005_PDF_1_4,
        PdfConformance::A2B_2011_PDF_1_7,
        PdfConformance::A3_2012_PDF_1_7,
        PdfConformance::UA_2014_PDF_1_6,
        PdfConformance::X1A_2001_PDF_1_3,
        PdfConformance::X4_2010_PDF_1_4,
        PdfConformance::X5G_2010_PDF_1_6,
        PdfConformance::E1_2008_PDF_1_6,
        PdfConformance::VT_2010_PDF_1_4,
    ];
    for level in levels {
        for date in dates {
            let mut doc = PdfDocument::new("dates");
            doc.metadata.info.creation_date = date;
            doc.metadata.info.modification_date = date;
            doc.metadata.info.metadata_date = date;
            doc.metadata.info.conformance = level.clone();
            doc.pages.push(PdfPage::new(Mm(50.0), Mm(50.0), vec![]));
            let opts = PdfSaveOptions { optimize: false, ..Default::default() };
            let bytes = doc.save(&opts, &mut Vec::new());
            assert!(bytes.starts_with(b"%PDF-"));
            // The dates also go through JSON, which parses them again.
            let json = serde_json::to_string(&doc).expect("serialize");
            let _ = serde_json::from_str::<PdfDocument>(&json);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// forms.rs
// ---------------------------------------------------------------------------------------------

use lopdf::{dictionary, Document, Object, StringFormat};
use printpdf::forms::{fields_svg, fill_form, parse_form_fields, FieldValue, FormStamp, StampPath, StampSegment};

/// A one-page PDF with the form field `f` (a widget); `extra` adds objects to the document and
/// entries to the field.
fn form_pdf_with(
    ft: &str,
    extra: impl FnOnce(&mut Document) -> Vec<(&'static str, Object)>,
) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();
    let extra = extra(&mut doc);
    let mut field = dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "P" => page_id,
        "T" => Object::String(b"f".to_vec(), StringFormat::Literal),
        "FT" => ft,
        "Rect" => vec![0.into(), 0.into(), 100.into(), 20.into()],
    };
    for (key, value) in extra {
        field.set(key, value);
    }
    let field_id = doc.add_object(field);
    doc.objects.insert(page_id, Object::Dictionary(dictionary! {
        "Type" => "Page", "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
        "Annots" => vec![field_id.into()],
    }));
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
        "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1,
    }));
    let form = doc.add_object(dictionary! { "Fields" => vec![field_id.into()] });
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id, "AcroForm" => form });
    doc.trailer.set("Root", catalog);
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("save");
    out
}

/// Reads and fills the form of `bytes`; neither may panic.
fn read_and_fill(bytes: &[u8]) {
    let _ = parse_form_fields(bytes);
    let stamp = FormStamp {
        page: 0,
        rect: [0.0, 0.0, 10.0, 10.0],
        view_box: [0.0, 0.0, 1.0, 1.0],
        paths: vec![],
    };
    for flatten in [false, true] {
        let _ = fill_form(bytes, &[FieldValue { name: "f".into(), value: "x".into() }], &[stamp.clone()], flatten);
    }
}

#[test]
fn forms_a_value_that_contains_itself_does_not_overflow_the_stack() {
    if in_child_process("forms_a_value_that_contains_itself_does_not_overflow_the_stack", 60, 1500) {
        return;
    }
    // `/V`, `/DV` and `/Opt` each name an array that holds a reference to itself.
    for (ft, key) in [("Tx", "V"), ("Tx", "DV"), ("Ch", "Opt"), ("Ch", "V")] {
        let bytes = form_pdf_with(ft, |doc| {
            let looping = doc.new_object_id();
            doc.objects.insert(looping, Object::Array(vec![Object::Reference(looping)]));
            vec![(key, Object::Reference(looping))]
        });
        read_and_fill(&bytes);
    }
    // Two arrays that hold each other, and an option that is an array of such arrays.
    let bytes = form_pdf_with("Ch", |doc| {
        let a = doc.new_object_id();
        let b = doc.new_object_id();
        doc.objects.insert(a, Object::Array(vec![Object::Reference(b), Object::Reference(b)]));
        doc.objects.insert(b, Object::Array(vec![Object::Reference(a), Object::Reference(a)]));
        vec![
            ("V", Object::Reference(a)),
            ("Opt", Object::Array(vec![Object::Reference(a), Object::Reference(b)])),
        ]
    });
    read_and_fill(&bytes);
    // A real value is still read.
    let bytes = form_pdf_with("Tx", |_| vec![("V", Object::String(b"hello".to_vec(), StringFormat::Literal))]);
    let fields = parse_form_fields(&bytes).expect("a PDF");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].value, "hello");
}

#[test]
fn forms_a_value_with_huge_fan_out_is_read_in_bounded_time() {
    if in_child_process("forms_a_value_with_huge_fan_out_is_read_in_bounded_time", 60, 1500) {
        return;
    }
    // 14 levels of arrays that list the next level 200 times: 200^14 paths to one string.
    let bytes = form_pdf_with("Tx", |doc| {
        let ids: Vec<_> = (0..14).map(|_| doc.new_object_id()).collect();
        for (i, id) in ids.iter().enumerate() {
            let next = match ids.get(i + 1) {
                Some(next) => Object::Reference(*next),
                None => Object::String(b"leaf".to_vec(), StringFormat::Literal),
            };
            doc.objects.insert(*id, Object::Array(vec![next; 200]));
        }
        vec![("V", Object::Reference(ids[0])), ("Opt", Object::Array(vec![Object::Reference(ids[0]); 50]))]
    });
    read_and_fill(&bytes);
}

#[test]
fn forms_cyclic_kids_and_parents_are_read() {
    if in_child_process("forms_cyclic_kids_and_parents_are_read", 60, 1500) {
        return;
    }
    // A field that is its own kid, two fields that are each other's kids, a deep chain, and a
    // page that is its own parent (the resources of a page are looked up through `/Parent`).
    let bytes = form_pdf_with("Tx", |doc| {
        let a = doc.new_object_id();
        let b = doc.new_object_id();
        doc.objects.insert(a, Object::Dictionary(dictionary! {
            "T" => Object::String(b"a".to_vec(), StringFormat::Literal),
            "Kids" => vec![a.into(), b.into()],
        }));
        doc.objects.insert(b, Object::Dictionary(dictionary! {
            "T" => Object::String(b"b".to_vec(), StringFormat::Literal),
            "Parent" => a, "Kids" => vec![a.into(), b.into()],
        }));
        vec![("Parent", Object::Reference(a)), ("Kids", Object::Array(vec![a.into(), b.into()]))]
    });
    read_and_fill(&bytes);

    let bytes = form_pdf_with("Tx", |doc| {
        let mut previous = None;
        let mut ids = Vec::new();
        for i in 0..200 {
            let id = doc.new_object_id();
            let mut d = dictionary! { "T" => Object::String(format!("n{i}").into_bytes(), StringFormat::Literal) };
            if let Some(p) = previous {
                d.set("Kids", Object::Array(vec![Object::Reference(p)]));
            }
            doc.objects.insert(id, Object::Dictionary(d));
            ids.push(id);
            previous = Some(id);
        }
        vec![("Kids", Object::Array(vec![Object::Reference(ids[199])]))]
    });
    read_and_fill(&bytes);

    // The page is its own parent.
    let mut doc = Document::load_mem(&form_pdf_with("Tx", |_| vec![])).expect("loads");
    let page_id = *doc.get_pages().values().next().expect("a page");
    if let Ok(page) = doc.get_object_mut(page_id).and_then(Object::as_dict_mut) {
        page.set("Parent", Object::Reference(page_id));
    }
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save");
    read_and_fill(&bytes);
}

#[test]
fn forms_mutated_documents_do_not_panic() {
    if in_child_process("forms_mutated_documents_do_not_panic", 120, 1500) {
        return;
    }
    let base = form_pdf_with("Ch", |_| {
        vec![
            ("Opt", Object::Array(vec![Object::String(b"red".to_vec(), StringFormat::Literal)])),
            ("V", Object::String(b"red".to_vec(), StringFormat::Literal)),
            ("DA", Object::String(b"/Helv 10 Tf 0 g".to_vec(), StringFormat::Literal)),
        ]
    });
    let mut state = 0xA076_1D64_78BD_642Fu64;
    let mut failures = Vec::new();
    for round in 0..300 {
        let mut bytes = base.clone();
        match xorshift(&mut state) % 3 {
            0 => {
                let keep = (xorshift(&mut state) % (bytes.len() as u64 + 1)) as usize;
                bytes.truncate(keep);
            }
            _ => {
                for _ in 0..(1 + xorshift(&mut state) % 4) {
                    let at = (xorshift(&mut state) % bytes.len() as u64) as usize;
                    if let Some(byte) = bytes.get_mut(at) {
                        *byte = xorshift(&mut state) as u8;
                    }
                }
            }
        }
        let result = std::panic::catch_unwind(|| read_and_fill(&bytes));
        if result.is_err() {
            failures.push(round);
        }
    }
    assert!(failures.is_empty(), "panics in rounds {failures:?}");
}

#[test]
fn forms_odd_text_strings_widgets_and_stamps_do_not_panic() {
    if in_child_process("forms_odd_text_strings_widgets_and_stamps_do_not_panic", 60, 1500) {
        return;
    }
    // `/V` strings with an odd number of UTF-16 bytes, a bare byte order mark, invalid UTF-8.
    for value in [
        &b"\xFE\xFF\x00"[..], b"\xFE\xFF", b"\xFE", b"\xEF\xBB\xBF\xFF\xFE", b"\xFE\xFF\xD8\x00", b"\xFF\xFF\xFF",
    ] {
        let bytes = form_pdf_with("Tx", |_| vec![("V", Object::String(value.to_vec(), StringFormat::Hexadecimal))]);
        let _ = parse_form_fields(&bytes);
        read_and_fill(&bytes);
        let _ = printpdf::forms::decode_text_string(value);
    }
    // Rectangles with too few, too many and non-numeric entries, and a `/DA` without operands.
    for rect in [
        vec![],
        vec![Object::Integer(1)],
        vec![Object::Integer(1); 3],
        vec![Object::Integer(1); 5],
        vec![Object::Null, Object::Null, Object::Null, Object::Null],
        vec![Object::Real(f32::NAN), Object::Real(f32::INFINITY), Object::Real(-1.0), Object::Real(f32::MIN)],
    ] {
        let rect = Object::Array(rect);
        for da in [&b""[..], b"Tf", b"g", b"rg", b"0 0 rg", b"/F 1e999 Tf", b"1 2 k", b"Tf Tf Tf"] {
            let bytes = form_pdf_with("Tx", |_| {
                vec![
                    ("Rect", rect.clone()),
                    ("DA", Object::String(da.to_vec(), StringFormat::Literal)),
                    ("V", Object::String(b"abc".to_vec(), StringFormat::Literal)),
                    ("Q", Object::Integer(i64::MAX)),
                    ("MaxLen", Object::Integer(i64::MIN)),
                    ("Ff", Object::Integer(i64::MAX)),
                ]
            });
            read_and_fill(&bytes);
        }
    }
    // Stamps with degenerate or non-finite geometry, and the SVG of fields on odd pages.
    let bytes = form_pdf_with("Tx", |_| vec![("V", Object::String(b"abc".to_vec(), StringFormat::Literal))]);
    for rect in [[0.0, 0.0, 0.0, 0.0], [f32::NAN; 4], [f32::INFINITY, 0.0, f32::NEG_INFINITY, 1.0]] {
        for view_box in [[0.0; 4], [0.0, 0.0, -1.0, 1.0], [f32::NAN; 4], [0.0, 0.0, 1e-30, 1e30]] {
            let stamp = FormStamp {
                page: usize::MAX,
                rect,
                view_box,
                paths: vec![StampPath {
                    segments: vec![StampSegment::MoveTo(f32::NAN, 1.0), StampSegment::CubicTo(0.0, 0.0, 1.0, 1.0, 2.0, 2.0), StampSegment::Close],
                    stroke: Some((f32::NAN, [2.0, -1.0, f32::NAN])),
                    fill: Some([0.0, 0.0, 0.0]),
                }],
            };
            let _ = fill_form(&bytes, &[], &[stamp.clone(), FormStamp { page: 0, ..stamp }], true);
        }
    }
    let fields = parse_form_fields(&bytes).expect("a PDF");
    for page_height in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
        let _ = fields_svg(&fields, 0, page_height);
        let _ = fields_svg(&fields, usize::MAX, page_height);
    }
}

// ---------------------------------------------------------------------------------------------
// html/
// ---------------------------------------------------------------------------------------------

#[cfg(feature = "html")]
#[allow(deprecated)]
#[test]
fn html_config_tags_with_odd_text_are_read() {
    use printpdf::html::process_html_for_rendering;

    let (_, config) = process_html_for_rendering(
        "<html><head><title> My Title </title><meta name=\"pdf.options.pageWidth\" content=\"123.5\">\
         <meta name=\"pdf.options.pageHeight\" content=\"200\">\
         <meta name=\"pdf.metadata.author\" content=\"Ada \u{e9}\"></head><body/></html>",
    );
    assert_eq!(config.title.as_deref(), Some("My Title"));
    assert_eq!(config.page_width, Some(123.5));
    assert_eq!(config.page_height, Some(200.0));
    assert_eq!(config.metadata.get("author").map(String::as_str), Some("Ada \u{e9}"));

    for html in [
        "", "<", "<title", "<title>", "</title>", "</title><title>", "<title></title>", "<title>\u{e9}",
        "\u{e9}<title>\u{e9}</title>\u{e9}", "<meta", "<meta ", "<meta >", "<meta name=", "<meta name=\"",
        "<meta name=\"pdf.options.pageWidth\" content=\"", "<meta name=\"\u{e9}\" content=\"\u{e9}\"",
        "\u{1F600}<meta \u{1F600} name=\"a\" content=\"b\"\u{1F600}>\u{1F600}", "<meta <meta <meta ",
        "<meta name=\"pdf.options.pageWidth\" content=\"NaN\"><meta name=\"pdf.options.pageHeight\" content=\"inf\">",
    ] {
        let (preprocessed, _) = process_html_for_rendering(html);
        let _ = preprocessed.len();
    }
    let (_, config) = process_html_for_rendering("<title>x");
    assert_eq!(config.title, None);
}

#[cfg(feature = "html")]
#[test]
fn html_elements_that_cannot_be_drawn_are_removed() {
    use printpdf::html::RawHtml;

    let clean = |html: &str| RawHtml::new(html).preprocess().as_str().to_string();
    assert_eq!(
        clean("a<script>b</script>c<script src=\"x\">d</script>e<script src=\"y\"/>f<iframe/>g"),
        "acefg"
    );
    for html in [
        "<script>", "<script>x", "<script >", "<script ", "<script src=x", "<script src=x>", "</script>",
        "<script></script", "\u{e9}<script>\u{e9}</script>\u{e9}", "<script/>", "<script />", "<script\u{e9}>",
        "<script><script></script></script>", "<video src=\"\u{1F600}\">\u{1F600}</video>\u{1F600}", "<<<<script>",
    ] {
        let _ = clean(html);
    }
    let many = "<script src=\"x\">".repeat(300);
    let _ = clean(&many);
}

#[cfg(feature = "html")]
fn render_html(html: &str, w: f32, h: f32, margins: [f32; 4]) -> Result<Vec<PdfPage>, String> {
    use printpdf::html::{build_font_pool, xml_to_pdf_pages, PageMargins, XmlRenderOptions};
    let mut options = XmlRenderOptions::default();
    options.page_width = Mm(w);
    options.page_height = Mm(h);
    options.margins = PageMargins { top: Mm(margins[0]), right: Mm(margins[1]), bottom: Mm(margins[2]), left: Mm(margins[3]) };
    let families: &[&str] = &[];
    options.font_pool = Some(build_font_pool(&std::collections::BTreeMap::new(), Some(families)));
    xml_to_pdf_pages(html, &options)
        .map(|(pages, ..)| pages)
        .map_err(|warnings| format!("{warnings:?}"))
}

#[cfg(feature = "html")]
fn render_with_size(w: f32, h: f32, margins: [f32; 4]) -> Result<usize, String> {
    render_html("<html><body><p>Hello</p></body></html>", w, h, margins).map(|pages| pages.len())
}

/// The points of all the paths in `ops`.
#[cfg(feature = "html")]
fn path_points(ops: &[Op]) -> usize {
    ops.iter()
        .map(|op| match op {
            Op::DrawLine { line } => line.points.len(),
            Op::DrawPolygon { polygon } => polygon.rings.iter().map(|r| r.points.len()).sum(),
            _ => 0,
        })
        .sum()
}

#[cfg(feature = "html")]
#[test]
fn html_page_sizes_that_leave_no_content_area_are_errors() {
    if in_child_process("html_page_sizes_that_leave_no_content_area_are_errors", 120, 1500) {
        return;
    }
    let nan = f32::NAN;
    let inf = f32::INFINITY;
    let none = [0.0; 4];
    let cases: Vec<(f32, f32, [f32; 4])> = vec![
        (0.0, 297.0, none),
        (210.0, 0.0, none),
        (0.0, 0.0, none),
        (-210.0, 297.0, none),
        (210.0, -297.0, none),
        (nan, 297.0, none),
        (210.0, nan, none),
        (inf, 297.0, none),
        (210.0, inf, none),
        (210.0, 297.0, [297.0, 0.0, 0.0, 0.0]),
        (210.0, 297.0, [100.0, 0.0, 200.0, 0.0]),
        (210.0, 297.0, [0.0, 105.0, 0.0, 105.0]),
        (210.0, 297.0, [nan, 0.0, 0.0, 0.0]),
        (210.0, 297.0, [0.0, inf, 0.0, 0.0]),
        (210.0, 297.0, [-inf, 0.0, 0.0, 0.0]),
        (210.0, 297.0, [0.0, 0.0, 0.0, nan]),
    ];
    for (w, h, margins) in cases {
        let result = render_with_size(w, h, margins);
        assert!(result.is_err(), "page {w} x {h} mm with margins {margins:?}: {result:?}");
    }
    for (w, h, margins) in [(210.0, 297.0, none), (45.0, 200.0, none), (210.0, 297.0, [10.0; 4]), (20.0, 20.0, [5.0; 4])] {
        let result = render_with_size(w, h, margins);
        assert!(matches!(result, Ok(pages) if pages >= 1), "page {w} x {h} mm: {result:?}");
    }
}

/// Sizes that ask layout for a page per sliver of content, or overflow its arithmetic.
#[cfg(feature = "html")]
#[test]
fn html_tiny_and_huge_page_sizes_are_errors() {
    if in_child_process("html_tiny_and_huge_page_sizes_are_errors", 120, 1500) {
        return;
    }
    let none = [0.0; 4];
    for (w, h, margins) in [
        (1e-30, 1e-30, none),
        (210.0, 0.001, none),
        (0.001, 297.0, none),
        (210.0, 297.0, [148.4999, 0.0, 148.4999, 0.0]),
        (1e30, 297.0, none),
        (210.0, 1e30, none),
        (3.0e37, 3.0e37, none),
        (f32::MAX, f32::MAX, none),
        (210.0, 297.0, [-1e30, 0.0, 0.0, 0.0]),
    ] {
        let result = render_with_size(w, h, margins);
        assert!(result.is_err(), "page {w} x {h} mm with margins {margins:?}: {result:?}");
    }
}

#[cfg(feature = "html")]
fn border_points(radius: f32) -> usize {
    use azul_core::geom::{LogicalPosition, LogicalRect, LogicalSize};
    use printpdf::html::border::{
        render_border, BorderColors, BorderConfig, BorderRadii, BorderStyleType, BorderStyles, BorderWidths,
    };
    let config = BorderConfig {
        bounds: LogicalRect { origin: LogicalPosition::new(10.0, 10.0), size: LogicalSize::new(100.0, 60.0) },
        widths: BorderWidths { top: 1.0, right: 2.0, bottom: 3.0, left: 4.0 },
        colors: BorderColors::default(),
        styles: BorderStyles {
            top: BorderStyleType::Solid,
            right: BorderStyleType::Dashed,
            bottom: BorderStyleType::Dotted,
            left: BorderStyleType::Double,
        },
        radii: BorderRadii {
            top_left: (radius, radius),
            top_right: (radius, radius),
            bottom_right: (radius, radius),
            bottom_left: (radius, radius),
        },
        page_height: 800.0,
        margin_left: 0.0,
        margin_top: 0.0,
    };
    let mut ops = Vec::new();
    render_border(&mut ops, &config);
    path_points(&ops)
}

#[cfg(feature = "html")]
#[test]
fn html_border_corner_arcs_have_a_bounded_number_of_segments() {
    if in_child_process("html_border_corner_arcs_have_a_bounded_number_of_segments", 60, 1500) {
        return;
    }
    // The arc of a corner has one segment per point of radius, up to a limit: a radius of
    // 100 000 pt does not make 100 000 segments.
    for radius in [0.0, 0.05, 5.0, 100.0, 1.0e5] {
        let points = border_points(radius);
        assert!(points < 20_000, "radius {radius}: {points} points");
    }
    assert!(border_points(5.0) > border_points(0.05), "small radii still get arcs");
}

#[cfg(feature = "html")]
#[test]
fn html_css_with_a_huge_border_radius_gives_bounded_paths() {
    if in_child_process("html_css_with_a_huge_border_radius_gives_bounded_paths", 120, 1500) {
        return;
    }
    for radius in ["10px", "300px", "200000px"] {
        let html = format!(
            "<html><body><div style=\"width:100px;height:60px;border-radius:{radius};\
             border-top:1px solid red;border-bottom:3px dashed blue;border-left:2px dotted green\">x</div></body></html>"
        );
        let pages = render_html(&html, 210.0, 297.0, [0.0; 4]).expect("renders");
        let points: usize = pages.iter().map(|page| path_points(&page.ops)).sum();
        assert!(points < 20_000, "border-radius {radius}: {points} points");
    }
}

/// Radii that `border-radius: 1e30px` can give: one segment per point of radius would be
/// `usize::MAX` segments.
#[cfg(feature = "html")]
#[test]
fn html_border_corner_arcs_survive_non_finite_and_enormous_radii() {
    if in_child_process("html_border_corner_arcs_survive_non_finite_and_enormous_radii", 60, 1500) {
        return;
    }
    for radius in [1.0e9, 1.0e30, f32::MAX, f32::INFINITY, f32::NAN, -1.0, f32::NEG_INFINITY] {
        let points = border_points(radius);
        assert!(points < 20_000, "radius {radius}: {points} points");
    }
}

// ---------------------------------------------------------------------------------------------
// The JSON API (src/wasm/)
// ---------------------------------------------------------------------------------------------

use printpdf::wasm::api::{
    Pdf_BytesToDocumentSync, Pdf_DecodeImageSync, Pdf_DocumentToBytesSync, Pdf_HtmlToDocumentSync,
    Pdf_PageToSvgSync, Pdf_RegisterFontsSync, Pdf_ResourcesForPageSync,
};
use serde_json::{json, Value};

type Endpoint = fn(String) -> String;

const ENDPOINTS: [(&str, Endpoint); 7] = [
    ("Pdf_HtmlToDocumentSync", Pdf_HtmlToDocumentSync),
    ("Pdf_BytesToDocumentSync", Pdf_BytesToDocumentSync),
    ("Pdf_ResourcesForPageSync", Pdf_ResourcesForPageSync),
    ("Pdf_PageToSvgSync", Pdf_PageToSvgSync),
    ("Pdf_DocumentToBytesSync", Pdf_DocumentToBytesSync),
    ("Pdf_DecodeImageSync", Pdf_DecodeImageSync),
    ("Pdf_RegisterFontsSync", Pdf_RegisterFontsSync),
];

/// Calls an endpoint like JS does. Returns the status of the envelope, or why the answer is wrong:
/// a panic, an answer that is not JSON, a status outside 0 to 3, an error without a message.
fn call_checked(name: &str, f: Endpoint, input: &str) -> Result<i64, String> {
    let owned = input.to_string();
    let out = std::panic::catch_unwind(move || f(owned)).map_err(|_| format!("{name}: panicked"))?;
    let envelope: Value = serde_json::from_str(&out).map_err(|e| format!("{name}: the answer is not JSON: {e}"))?;
    let status = envelope["status"].as_i64().ok_or_else(|| format!("{name}: no status"))?;
    if !(0..=3).contains(&status) {
        return Err(format!("{name}: status {status}"));
    }
    if status != 0 && !envelope["data"].is_string() {
        return Err(format!("{name}: status {status} without a message"));
    }
    Ok(status)
}

fn check_all(inputs: &[String], failures: &mut Vec<String>) {
    for (name, f) in ENDPOINTS {
        for input in inputs {
            if let Err(why) = call_checked(name, f, input) {
                let shown: String = input.chars().take(120).collect();
                failures.push(format!("{why} for {shown:?}"));
            }
        }
    }
}

#[test]
fn api_inputs_that_are_not_the_expected_json_give_an_error_envelope() {
    if in_child_process("api_inputs_that_are_not_the_expected_json_give_an_error_envelope", 120, 1500) {
        return;
    }
    let mut inputs: Vec<String> = [
        "", " ", "{", "}", "[", "]", "[]", "{}", "null", "true", "false", "0", "-1", "1e999", "\"\"", "\"x\"",
        "\u{0}", "\u{e9}", "\u{1F600}", "{\"\u{e9}\": 1}", "{\"html\":", "{\"html\":null}", "{\"html\":5}",
        "{\"html\":[]}", "{\"bytes\":null}", "{\"bytes\":\"\"}", "{\"bytes\":[]}", "{\"bytes\":\"!!!\"}",
        "{\"bytes\":\"AAAA====\"}", "{\"bytes\":\"data:\"}", "{\"bytes\":\"data:,\"}", "{\"bytes\":\"data:;base64,\"}",
        "{\"bytes\":\"data:\u{e9},\u{e9}\"}", "{\"bytes\":\"data:image/png;base64,@@@\"}", "{\"bytes\":\"\u{e9}\"}",
        "{\"bytes\":[256]}", "{\"bytes\":[-1]}", "{\"bytes\":[1.5]}", "{\"bytes\":[null]}", "{\"bytes\":1e999}",
        "{\"bytes\":{}}", "{\"page\":{}}", "{\"page\":null}", "{\"page\":[]}", "{\"doc\":{}}", "{\"doc\":null}",
        "{\"doc\":{\"pages\":[{}]}}", "{\"fonts\":null}", "{\"fonts\":[]}", "{\"fonts\":{\"a\":\"!!!\"}}",
        "{\"fonts\":{\"a\":5}}", "{\"fonts\":{\"a\":[1,2,3]},\"replace\":\"yes\"}", "{\"images\":{\"a\":\"!!!\"}}",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    // Deep nesting and long inputs.
    inputs.push("[".repeat(100_000));
    inputs.push(format!("{}1{}", "[".repeat(100_000), "]".repeat(100_000)));
    inputs.push(format!("{{\"a\":{}", "{\"a\":".repeat(100_000)));
    inputs.push(format!("{{\"html\":\"{}\"}}", "<div>".repeat(10)));
    inputs.push(format!("{{\"bytes\":\"{}\"}}", "A".repeat(1 << 20)));
    inputs.push(format!("{{\"bytes\":[{}0]}}", "0,".repeat(1 << 16)));
    let mut failures = Vec::new();
    check_all(&inputs, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    for (name, f) in ENDPOINTS {
        if name != "Pdf_RegisterFontsSync" {
            assert_eq!(call_checked(name, f, "").expect(name), 1, "{name}: empty input");
            assert_eq!(call_checked(name, f, "[").expect(name), 1, "{name}: truncated JSON");
        }
    }
    let _ = Pdf_RegisterFontsSync(json!({ "replace": true }).to_string());
}

#[test]
fn api_bytes_that_are_not_a_pdf_or_an_image_give_an_error_envelope() {
    if in_child_process("api_bytes_that_are_not_a_pdf_or_an_image_give_an_error_envelope", 120, 1500) {
        return;
    }
    use base64::Engine;
    let b64 = |bytes: &[u8]| base64::prelude::BASE64_STANDARD.encode(bytes);

    let mut doc = PdfDocument::new("api");
    doc.pages.push(PdfPage::new(Mm(50.0), Mm(50.0), vec![Op::Marker { id: "m".into() }]));
    let valid = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
    let mut byte_sets: Vec<Vec<u8>> = vec![
        vec![],
        b"%PDF-".to_vec(),
        b"%PDF-1.7\n".to_vec(),
        b"%PDF-1.7\n%%EOF".to_vec(),
        b"\x89PNG\r\n\x1a\n".to_vec(),
        b"\xFF\xD8\xFF".to_vec(),
        vec![0; 100],
        vec![0xFF; 100],
        valid.clone(),
    ];
    for cut in (0..valid.len()).step_by((valid.len() / 40).max(1)) {
        byte_sets.push(valid[..cut].to_vec());
    }
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..60 {
        let mut bytes = valid.clone();
        for _ in 0..(1 + xorshift(&mut state) % 5) {
            let at = (xorshift(&mut state) % bytes.len() as u64) as usize;
            if let Some(byte) = bytes.get_mut(at) {
                *byte = xorshift(&mut state) as u8;
            }
        }
        byte_sets.push(bytes);
    }
    let mut failures = Vec::new();
    for bytes in &byte_sets {
        for form in [json!({ "bytes": b64(bytes) }), json!({ "bytes": bytes }), json!({ "bytes": format!("data:application/pdf;base64,{}", b64(bytes)) })] {
            for (name, f) in [
                ("Pdf_BytesToDocumentSync", Pdf_BytesToDocumentSync as Endpoint),
                ("Pdf_DecodeImageSync", Pdf_DecodeImageSync as Endpoint),
            ] {
                if let Err(why) = call_checked(name, f, &form.to_string()) {
                    failures.push(format!("{why} for {} bytes", bytes.len()));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // The valid file is read.
    let r: Value = serde_json::from_str(&Pdf_BytesToDocumentSync(json!({ "bytes": b64(&valid) }).to_string())).expect("JSON");
    assert_eq!(r["status"], 0, "{r}");
}

/// A page whose every real number is the text `@@`, as JSON text.
fn page_json_with_number_marker() -> String {
    fn mark(v: &mut Value) {
        if matches!(v, Value::Number(n) if n.is_f64()) {
            *v = Value::String("@@".into());
            return;
        }
        match v {
            Value::Array(items) => items.iter_mut().for_each(mark),
            Value::Object(map) => map.values_mut().for_each(mark),
            _ => {}
        }
    }
    let m = 12.5;
    let page = PdfPage::new(
        Mm(m),
        Mm(m),
        vec![
            Op::SaveGraphicsState,
            Op::SetFillColor { col: Color::Rgb(Rgb { r: m, g: m, b: m, icc_profile: None }) },
            Op::SetOutlineColor { col: Color::Cmyk(Cmyk { c: m, m, y: m, k: m, icc_profile: None }) },
            Op::SetOutlineThickness { pt: Pt(m) },
            Op::SetMiterLimit { limit: Pt(m) },
            Op::SetLineDashPattern { dash: LineDashPattern::new(m, &[m, m]) },
            Op::DrawRectangle {
                rectangle: Rect { x: Pt(m), y: Pt(m), width: Pt(m), height: Pt(m), mode: Some(PaintMode::FillStroke), winding_order: None },
            },
            Op::DrawLine { line: Line { points: vec![lp(m, false), lp(m, true), lp(m, true), lp(m, false)], is_closed: true } },
            Op::DrawPolygon {
                polygon: Polygon {
                    rings: vec![PolygonRing { points: vec![lp(m, false), lp(m, false), lp(m, false)] }],
                    mode: PaintMode::FillStroke,
                    winding_order: WindingOrder::EvenOdd,
                },
            },
            Op::SetTransformationMatrix { matrix: CurTransMat::Raw([m; 6]) },
            Op::StartTextSection,
            Op::SetFont { font: PdfFontHandle::Builtin(BuiltinFont::Helvetica), size: Pt(m) },
            Op::SetTextCursor { pos: Point::new(Mm(m), Mm(m)) },
            Op::SetTextMatrix { matrix: TextMatrix::Raw([m; 6]) },
            Op::SetLineHeight { lh: Pt(m) },
            Op::SetWordSpacing { pt: Pt(m) },
            Op::SetCharacterSpacing { multiplier: m },
            Op::SetHorizontalScaling { percent: m },
            Op::ShowText { items: vec![TextItem::Text("h\u{e9}llo".into()), TextItem::Offset(m)] },
            Op::EndTextSection,
            Op::UseXobject {
                id: XObjectId("x".into()),
                transform: XObjectTransform {
                    translate_x: Some(Pt(m)),
                    translate_y: Some(Pt(m)),
                    rotate: None,
                    scale_x: Some(m),
                    scale_y: Some(m),
                    dpi: Some(m),
                    no_auto_scale: false,
                },
            },
            Op::RestoreGraphicsState,
        ],
    );
    let mut value = serde_json::to_value(&page).expect("serialize");
    mark(&mut value);
    value.to_string().replace("\"@@\"", "@@")
}

#[test]
fn api_pages_with_non_finite_negative_and_huge_numbers_give_a_result_or_an_error() {
    if in_child_process("api_pages_with_non_finite_negative_and_huge_numbers_give_a_result_or_an_error", 120, 1500) {
        return;
    }
    let template = page_json_with_number_marker();
    let mut failures = Vec::new();
    let mut accepted = 0;
    for literal in [
        "1e39", "-1e39", "1e-39", "0.0", "-0.0", "3.4028235e38", "-3.4028235e38", "1e300", "-1e300",
        "123456789012345678901234567890", "0.5", "-5.0", "1e9", "1e-9", "65535.0", "2147483648.0",
    ] {
        let page = template.replace("@@", literal);
        let doc = format!(
            "{{\"metadata\":{},\"resources\":{{}},\"bookmarks\":{{}},\"pages\":[{page}]}}",
            serde_json::to_string(&PdfDocument::new("n").metadata).expect("serialize")
        );
        for (name, f, input) in [
            ("Pdf_DocumentToBytesSync", Pdf_DocumentToBytesSync as Endpoint, format!("{{\"doc\":{doc}}}")),
            ("Pdf_PageToSvgSync", Pdf_PageToSvgSync as Endpoint, format!("{{\"page\":{page}}}")),
            ("Pdf_ResourcesForPageSync", Pdf_ResourcesForPageSync as Endpoint, format!("{{\"page\":{page}}}")),
        ] {
            match call_checked(name, f, &input) {
                Ok(0) => accepted += 1,
                Ok(_) => {}
                Err(why) => failures.push(format!("{why} for the number {literal}")),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(accepted > 0, "no variant was accepted: the template does not parse");
}

#[test]
fn api_documents_with_pages_naming_unknown_resources_are_saved_and_drawn() {
    if in_child_process("api_documents_with_pages_naming_unknown_resources_are_saved_and_drawn", 120, 1500) {
        return;
    }
    let mut doc = PdfDocument::new("dangling");
    doc.pages.push(PdfPage::new(
        Mm(100.0),
        Mm(100.0),
        vec![
            Op::SetFont { font: PdfFontHandle::External(FontId("nope".into())), size: Pt(12.0) },
            Op::StartTextSection,
            Op::ShowText { items: vec![TextItem::Text("x".into()), TextItem::GlyphIds(vec![Codepoint::new(9, 0.0)])] },
            Op::EndTextSection,
            Op::UseXobject { id: XObjectId("nope".into()), transform: XObjectTransform::default() },
            Op::LoadGraphicsState { gs: ExtendedGraphicsStateId("nope".into()) },
            Op::PaintShading { id: ShadingId("nope".into()) },
            Op::BeginLayer { layer_id: LayerInternalId("nope".into()) },
            Op::EndLayer,
        ],
    ));
    let doc_json = serde_json::to_value(&doc).expect("serialize");
    let page_json = doc_json["pages"][0].clone();
    for (name, f, input) in [
        ("Pdf_DocumentToBytesSync", Pdf_DocumentToBytesSync as Endpoint, json!({ "doc": doc_json })),
        ("Pdf_DocumentToBytesSync", Pdf_DocumentToBytesSync as Endpoint, json!({ "doc": doc_json, "returnByteArray": true, "options": { "optimize": false, "secure": false } })),
        ("Pdf_PageToSvgSync", Pdf_PageToSvgSync as Endpoint, json!({ "page": page_json })),
        ("Pdf_PageToSvgSync", Pdf_PageToSvgSync as Endpoint, json!({ "page": page_json, "resources": doc_json["resources"] })),
        ("Pdf_ResourcesForPageSync", Pdf_ResourcesForPageSync as Endpoint, json!({ "page": page_json })),
    ] {
        assert_eq!(call_checked(name, f, &input.to_string()), Ok(0), "{name}");
    }
}

#[test]
fn api_font_registry_rejects_what_is_not_base64_and_keeps_the_rest() {
    if in_child_process("api_font_registry_rejects_what_is_not_base64_and_keeps_the_rest", 180, 2000) {
        return;
    }
    let call = |input: Value| -> Value {
        serde_json::from_str(&Pdf_RegisterFontsSync(input.to_string())).expect("the answer is JSON")
    };
    let r = call(json!({ "replace": true }));
    assert_eq!(r["data"]["total"], 0);
    // An entry that is not base64 is an error, whatever else the call holds.
    let r = call(json!({ "fonts": { "a": "!!!" } }));
    assert_eq!(r["status"], 2, "{r}");
    // Bytes that are not a font are stored as they are; using them does not panic.
    let r = call(json!({ "fonts": { "a": "AAAA", "b": [], "c": [0, 1, 2, 3, 4, 5, 6, 7] }, "replace": true }));
    assert_eq!(r["status"], 0, "{r}");
    assert_eq!(r["data"]["total"], 3);
    let mut failures = Vec::new();
    check_all(&[json!({ "html": "<p>hi</p>" }).to_string()], &mut failures);
    let r = call(json!({ "replace": true }));
    assert_eq!(r["data"]["total"], 0);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[cfg(feature = "html")]
#[test]
fn api_html_documents_and_options_that_are_odd_give_a_result_or_an_error() {
    if in_child_process("api_html_documents_and_options_that_are_odd_give_a_result_or_an_error", 180, 2000) {
        return;
    }
    let mut failures = Vec::new();
    let nested = |n: usize| format!("{}x{}", "<div>".repeat(n), "</div>".repeat(n));
    let htmls = [
        String::new(),
        "<".to_string(),
        "<<<<".to_string(),
        "&".to_string(),
        "&#xFFFFFFFF;&#0;&#xD800;&bogus;".to_string(),
        "<html".to_string(),
        "<p style=\"width:-5px;height:1e9px;font-size:0;line-height:0;border-radius:-3px\">x</p>".to_string(),
        "<p style=\"width:1e39px;padding:1e39px;margin-top:1e39px;border:1e39px solid red\">x</p>".to_string(),
        "<img src=\"\u{e9}\"><img src=\"data:image/png;base64,!!!\"><img>".to_string(),
        "<meta name=\"pdf.options.pageWidth\" content=\"NaN\"><p>x</p>".to_string(),
        "<meta name=\"pdf.options.pageHeight\" content=\"0\"><p>x</p>".to_string(),
        "<title>\u{e9}</title><p>\u{1F600}</p>".to_string(),
        nested(30),
    ];
    let options = [
        json!({}),
        json!({ "page_width": 0, "page_height": 0 }),
        json!({ "page_width": -1, "page_height": -1, "margin_top": 1e30 }),
        json!({ "page_width": 1e39, "page_height": 1e39 }),
        json!({ "page_width": 210, "page_height": 297, "margin_top": 400, "margin_bottom": 400 }),
        json!({ "show_page_numbers": true, "header_text": "\u{e9}", "footer_text": "", "skip_first_page": true }),
    ];
    for html in &htmls {
        for opts in &options {
            let input = json!({ "html": html, "options": opts }).to_string();
            if let Err(why) = call_checked("Pdf_HtmlToDocumentSync", Pdf_HtmlToDocumentSync, &input) {
                let shown: String = input.chars().take(160).collect();
                failures.push(format!("{why} for {shown}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let r: Value = serde_json::from_str(&Pdf_HtmlToDocumentSync(json!({ "html": "<p>hi</p>" }).to_string())).expect("JSON");
    assert_eq!(r["status"], 0, "{r}");
}

// A box of absurd height makes the paged layout of azul-layout cut millions of empty pages: about
// 1 GB and several seconds for a 10 KB result. `cargo test -- --ignored` shows the numbers.
#[cfg(feature = "html")]
#[test]
#[ignore = "azul-layout cuts millions of empty pages for a box of absurd height"]
fn api_html_with_an_enormous_height_gives_a_result_or_an_error() {
    if in_child_process("api_html_with_an_enormous_height_gives_a_result_or_an_error", 120, 3000) {
        return;
    }
    for css in ["height:1e39px", "font-size:1e30px", "position:absolute;top:1e39px"] {
        let input = json!({ "html": format!("<p style=\"{css}\">x</p>") }).to_string();
        if let Err(why) = call_checked("Pdf_HtmlToDocumentSync", Pdf_HtmlToDocumentSync, &input) {
            panic!("{why} for {css}");
        }
    }
}

// Without `text_layout` a font does not keep its source bytes, so the JSON has no data URI to corrupt.
#[cfg(feature = "text_layout")]
#[test]
fn api_documents_with_corrupted_fonts_give_an_error_envelope() {
    if in_child_process("api_documents_with_corrupted_fonts_give_an_error_envelope", 180, 2000) {
        return;
    }
    use base64::Engine;
    let b64 = |bytes: &[u8]| base64::prelude::BASE64_STANDARD.encode(bytes);
    let fonts: [&[u8]; 4] = [
        MOCK_TTF,
        include_bytes!("./assets/fonts/mock/mock_cff_cid.otf"),
        include_bytes!("./assets/fonts/mock/mock_cff_cid_fm.otf"),
        include_bytes!("./assets/fonts/mock/mock_cff_named.otf"),
    ];
    let mut state = 0x6C07_8965_D3A1_F00Du64;
    let mut failures = Vec::new();
    for font_bytes in fonts {
        let font = ParsedFont::from_bytes(font_bytes, 0, &mut Vec::new()).expect("a mock font parses");
        let mut doc = PdfDocument::new("fonts");
        let id = doc.add_font(&font);
        doc.pages.push(PdfPage::new(
            Mm(50.0),
            Mm(50.0),
            vec![
                Op::SetFont { font: PdfFontHandle::External(id), size: Pt(10.0) },
                Op::StartTextSection,
                Op::ShowText {
                    items: vec![
                        TextItem::Text("AB".into()),
                        TextItem::GlyphIds(vec![Codepoint::with_cid(2, 0.0, "A".into()), Codepoint::new(3, 0.0)]),
                    ],
                },
                Op::EndTextSection,
            ],
        ));
        let doc_json = serde_json::to_string(&doc).expect("serialize");
        let uri = format!("data:font/ttf;base64,{}", b64(font_bytes));
        assert!(doc_json.contains(&uri), "the font is a data URI in the JSON");
        for _ in 0..40 {
            let mut bytes = font_bytes.to_vec();
            if xorshift(&mut state) % 4 == 0 {
                bytes.truncate((xorshift(&mut state) % bytes.len() as u64) as usize);
            } else {
                for _ in 0..(1 + xorshift(&mut state) % 3) {
                    let at = (xorshift(&mut state) % bytes.len() as u64) as usize;
                    if let Some(byte) = bytes.get_mut(at) {
                        *byte = xorshift(&mut state) as u8;
                    }
                }
            }
            let mutated = doc_json.replace(&uri, &format!("data:font/ttf;base64,{}", b64(&bytes)));
            let value: Value = serde_json::from_str(&mutated).expect("JSON");
            let page_input = json!({ "page": value["pages"][0], "resources": value["resources"] });
            for (name, f, input) in [
                ("Pdf_DocumentToBytesSync", Pdf_DocumentToBytesSync as Endpoint, format!("{{\"doc\":{mutated}}}")),
                ("Pdf_PageToSvgSync", Pdf_PageToSvgSync as Endpoint, page_input.to_string()),
            ] {
                if let Err(why) = call_checked(name, f, &input) {
                    failures.push(format!("{why} for a font of {} bytes", bytes.len()));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[cfg(feature = "html")]
#[test]
fn api_html_with_corrupted_fonts_gives_a_result_or_an_error() {
    if in_child_process("api_html_with_corrupted_fonts_gives_a_result_or_an_error", 180, 2000) {
        return;
    }
    use base64::Engine;
    let b64 = |bytes: &[u8]| base64::prelude::BASE64_STANDARD.encode(bytes);
    let fonts: [&[u8]; 4] = [
        MOCK_TTF,
        include_bytes!("./assets/fonts/mock/mock_cff_cid.otf"),
        include_bytes!("./assets/fonts/mock/mock_cff_cid_fm.otf"),
        include_bytes!("./assets/fonts/mock/mock_cff_named.otf"),
    ];
    let mut state = 0x1F83_D9AB_FB41_BD6Bu64;
    let mut failures = Vec::new();
    for font_bytes in fonts {
        for round in 0..12 {
            let mut bytes = font_bytes.to_vec();
            match round {
                0 => bytes.clear(),
                1 => bytes.truncate(bytes.len() / 2),
                _ => {
                    for _ in 0..(1 + xorshift(&mut state) % 3) {
                        let at = (xorshift(&mut state) % bytes.len() as u64) as usize;
                        if let Some(byte) = bytes.get_mut(at) {
                            *byte = xorshift(&mut state) as u8;
                        }
                    }
                }
            }
            let input = json!({
                "html": "<html><body><p style=\"font-family: mock\">AB \u{e9}</p></body></html>",
                "fonts": { "mock": b64(&bytes) },
            })
            .to_string();
            if let Err(why) = call_checked("Pdf_HtmlToDocumentSync", Pdf_HtmlToDocumentSync, &input) {
                failures.push(format!("{why} for a font of {} bytes", bytes.len()));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
