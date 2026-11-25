//! Crafted input for the CMap, font, text, shading, unit, document and optimizer code must come
//! back as an error, a skipped item or a value. It must not panic, hang or allocate without
//! bound.
//!
//! Every input is built in code. A call that can hang or allocate runs on its own thread, and
//! the test fails when it panics or does not finish in time.

use std::any::Any;
use std::cmp::Ordering;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::time::Duration;

use lopdf::{dictionary, Object, Stream, StringFormat};
use printpdf::*;

fn panic_text(e: &(dyn Any + Send)) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "(no message)".to_string()
    }
}

/// Runs `f` on its own thread. Fails the test when `f` panics or takes longer than `secs`.
fn bounded_for<T: Send + 'static>(
    secs: u64,
    what: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let r = catch_unwind(AssertUnwindSafe(f)).map_err(|e| panic_text(&*e));
        let _ = tx.send(r);
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(Ok(v)) => v,
        Ok(Err(msg)) => panic!("{what}: panicked: {msg}"),
        Err(_) => panic!("{what}: no result within {secs} s (hang or runaway allocation)"),
    }
}

fn bounded<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    // A debug build on a CI runner is about ten times slower than a release build.
    bounded_for(if cfg!(debug_assertions) { 60 } else { 5 }, what, f)
}

/// xorshift64, a fixed sequence for the mutation loops.
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
// ToUnicode and CID CMaps (src/cmap.rs)
// ---------------------------------------------------------------------------------------------

#[test]
fn to_unicode_many_wide_ranges_are_bounded() {
    // 40 ranges of 65536 codes each: 2.6 million entries from about 1.4 KB of text.
    let mut s = String::from("begincmap\n40 beginbfrange\n");
    for i in 0..40u32 {
        s.push_str(&format!("<{i:04X}0000> <{i:04X}FFFF> <0041>\n"));
    }
    s.push_str("endbfrange\nendcmap\n");
    let r = bounded("40 wide bfranges", move || {
        ToUnicodeCMap::parse(&s).map(|m| m.mappings.len())
    });
    if let Ok(n) = r {
        assert!(n <= 1 << 21, "{n} map entries from 1.4 KB of text");
    }
}

#[test]
fn to_unicode_range_with_a_long_target_is_bounded() {
    // One range of 65536 codes whose target has 300 characters: 20 million scalars.
    let target = "0041".repeat(300);
    let s = format!("beginbfrange\n<0000> <FFFF> <{target}>\nendbfrange\n");
    let r = bounded("bfrange with a long target", move || {
        ToUnicodeCMap::parse(&s).map(|m| m.mappings.values().map(Vec::len).sum::<usize>())
    });
    if let Ok(n) = r {
        assert!(n <= 1 << 21, "{n} scalars from a 1.2 KB target");
    }
}

#[test]
fn to_unicode_full_two_byte_code_space_still_parses() {
    // A CMap that maps every two-byte code, as a big CJK font can have, is not hostile.
    let mut chars = String::from("begincmap\n");
    for block in 0..656u32 {
        chars.push_str("100 beginbfchar\n");
        for k in 0..100u32 {
            let code = block * 100 + k;
            if code <= 0xFFFF {
                chars.push_str(&format!("<{code:04X}> <{:04X}>\n", 0x0100 + (code & 0x0FFF)));
            }
        }
        chars.push_str("endbfchar\n");
    }
    chars.push_str("endcmap\n");
    let ranges = "begincmap\n1 beginbfrange\n<0000> <FFFF> <0100>\nendbfrange\nendcmap\n".to_string();
    let (a, b) = bounded("full code space", move || {
        (
            ToUnicodeCMap::parse(&chars).map(|m| m.mappings.len()),
            ToUnicodeCMap::parse(&ranges).map(|m| m.mappings.len()),
        )
    });
    assert_eq!(a, Ok(65_536));
    assert_eq!(b, Ok(65_536));
}

#[test]
fn to_unicode_surrogates_and_wide_tokens() {
    let cmap = "beginbfchar\n<0001> <D835DC56>\n<0002> <D800>\n<0003> <DC00>\n<0004> <D800D800>\n\
                <0005> <FFFFFFFF>\nendbfchar\nbeginbfrange\n<0010> <0012> <FFFFFFFF>\n\
                <FFFFFFFE> <FFFFFFFF> <0041>\nendbfrange\n";
    let m = bounded("surrogates", move || ToUnicodeCMap::parse(cmap)).unwrap();
    assert_eq!(m.lookup(1), Some(&vec![0x1D456]));
    // A target of at most four digits is one number, which can be a surrogate; it is not text.
    assert_eq!(m.lookup(2), Some(&vec![0xD800]));
    assert_eq!(m.lookup(3), Some(&vec![0xDC00]));
    assert_eq!(m.lookup_string(2), None);
    // Longer targets are UTF-16, where a lone surrogate becomes U+FFFD.
    assert_eq!(m.lookup(4), Some(&vec![0xFFFD, 0xFFFD]));
    assert!(m.lookup(0xFFFF_FFFF).is_some(), "a range that ends at u32::MAX");
    // A nine-digit target does not fit a scalar.
    assert!(ToUnicodeCMap::parse("beginbfchar <0001> <FFFFFFFFF> endbfchar").is_err());
}

const MALFORMED_CMAPS: &[&str] = &[
    "",
    "<",
    ">",
    "<<",
    ">>",
    "[",
    "]",
    "% comment without a newline",
    "\0\0\0",
    "beginbfchar",
    "beginbfchar <0001>",
    "beginbfchar <0001> <> endbfchar",
    "beginbfchar <> <0041> endbfchar",
    "beginbfchar < > < > endbfchar",
    "beginbfchar <0001> <+041> endbfchar",
    "beginbfchar <0001> <-041> endbfchar",
    "beginbfchar <0x01> <0x41> endbfchar",
    "beginbfchar <0001> <004\u{e9}> endbfchar",
    "beginbfchar <\u{65e5}\u{672c}> <0041> endbfchar",
    "beginbfrange",
    "beginbfrange <0001>",
    "beginbfrange <0001> <0002>",
    "beginbfrange <0001> <0002> [",
    "beginbfrange <0001> <0002> [ <0041>",
    "beginbfrange <0001> <0002> [ [ <0041> ] ]",
    "beginbfrange <0001> <0002> [ <0041> <0042> <0043> <0044> ] endbfrange",
    "beginbfrange <0002> <0001> <0041> endbfrange",
    "beginbfrange <FFFFFFFF> <FFFFFFFF> <0041> endbfrange",
    "beginbfrange <FFFFFFFE> <FFFFFFFF> [<0041> <0042>] endbfrange",
    "beginbfrange <0000> <FFFFFFFF> <0041> endbfrange",
    "begincodespacerange <00> endcodespacerange",
    "begincodespacerange <> <> endcodespacerange",
    "begincodespacerange <0000> <FF> endcodespacerange",
    "begincodespacerange <000000000> <FFFFFFFFF> endcodespacerange",
    "begincodespacerange <0\u{e9}0> <FFFF> endcodespacerange",
    "begincidrange <00> <FF> endcidrange",
    "begincidrange <00> <FF> 99999999999 endcidrange",
    "begincidrange <FF> <00> 1 endcidrange",
    "begincidrange <0000> <FFFF> 4294967295 endcidrange",
    "begincidrange <00000000> <FFFFFFFF> 0 endcidrange",
    "beginnotdefrange <00> <1F> 1 endnotdefrange",
    "begincidchar <00> 4294967296 endcidchar",
    "begincidchar <00> -1 endcidchar",
    "begincidchar <00> x endcidchar",
    "usecmap",
    "/ usecmap",
    "/Identity-H usecmap /Identity-V usecmap",
    "/90ms-RKSJ-H usecmap",
    "/WMode",
    "/WMode 1 /WMode",
];

#[test]
fn malformed_cmaps_do_not_panic() {
    bounded("malformed CMaps", || {
        for case in MALFORMED_CMAPS {
            if let Ok(m) = ToUnicodeCMap::parse(case) {
                let _ = m.to_cmap_string("Test");
                let _ = m.lookup_string(1);
                let _ = m.map_bytes(&[0, 1, 2]);
            }
            if let Ok((c, _)) = CidCMap::parse(case) {
                let bytes = [0u8, 1, 2, 3, 0x41, 0x81, 0x40, 0xFF];
                let parts = c.split_codes(&bytes);
                for (code, len) in parts {
                    let _ = c.cid_for_code(code, len);
                }
            }
        }
    });
}

#[test]
fn cmap_mutation_loop_does_not_panic() {
    const SEED: &str = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
        /CMapName /Test def /WMode 0 def\n\
        2 begincodespacerange <00> <7F> <8000> <9FFF> endcodespacerange\n\
        2 beginbfchar <03> <0020> <04> <D835DC56> endbfchar\n\
        2 beginbfrange <10> <12> <0041> <20> <22> [<0061> <0062> <0063>] endbfrange\n\
        1 begincidrange <41> <5A> 500 endcidrange\n\
        1 begincidchar <8140> 7 endcidchar\n\
        /Identity-H usecmap\nendcmap\n";
    const POOL: &[&str] = &[
        "<", ">", "[", "]", "%", "\n", " ", "\u{e9}", "\u{65e5}", "\u{1d456}", "<FFFFFFFF>",
        "<0000>", "<D800>", "4294967295", "0", "9", "-", "+", "/Name", "begincidrange",
        "endbfrange", "beginbfchar", "usecmap", "<>", "<<", "\0",
    ];
    let failures = bounded_for(30, "CMap mutations", || {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let seed: Vec<char> = SEED.chars().collect();
        let mut failures = Vec::new();
        for _ in 0..300 {
            let mut text = seed.clone();
            for _ in 0..=rng.below(4) {
                let at = rng.below(text.len() + 1);
                match rng.below(3) {
                    0 => {
                        let n = rng.below(6).min(text.len().saturating_sub(at));
                        text.drain(at..at + n);
                    }
                    1 => {
                        let piece = POOL[rng.below(POOL.len())];
                        for (k, c) in piece.chars().enumerate() {
                            text.insert((at + k).min(text.len()), c);
                        }
                    }
                    _ => {
                        if let Some(c) = text.get_mut(at) {
                            *c = char::from_u32(rng.below(0x3000) as u32).unwrap_or('?');
                        }
                    }
                }
            }
            let text: String = text.into_iter().collect();
            let probe = text.clone();
            let r = catch_unwind(AssertUnwindSafe(move || {
                if let Ok(m) = ToUnicodeCMap::parse(&probe) {
                    let _ = m.to_cmap_string("T");
                    let _ = m.map_bytes(&[0, 3, 0, 4, 0, 0x10, 9]);
                    for cid in [0u32, 3, 4, 0x10, 0x22, u32::MAX] {
                        let _ = m.lookup_string(cid);
                    }
                }
                if let Ok((c, _)) = CidCMap::parse(&probe) {
                    let bytes = [0u8, 3, 4, 0x41, 0x5A, 0x81, 0x40, 0x7F, 0x80, 0xFF, 0xFF];
                    for (code, len) in c.split_codes(&bytes) {
                        let _ = c.cid_for_code(code, len);
                    }
                }
            }));
            if r.is_err() {
                failures.push(text);
            }
        }
        failures
    });
    assert!(failures.is_empty(), "{} inputs panicked, first: {:?}", failures.len(), failures.first());
}

#[test]
fn cid_cmap_with_thousands_of_codespace_ranges_is_bounded() {
    // 20000 two-byte codespace ranges, and a string of 100000 two-byte codes: every code is
    // compared with every range.
    let mut s = String::from("20000 begincodespacerange\n");
    for i in 0..20_000u32 {
        s.push_str(&format!("<{:04X}> <{:04X}>\n", i, i + 1));
    }
    s.push_str("endcodespacerange\nendcmap\n");
    let bytes: Vec<u8> = (0..200_000u32).map(|i| (i.wrapping_mul(7) % 251) as u8).collect();
    let n = bounded("20000 codespace ranges", move || match CidCMap::parse(&s) {
        Ok((c, _)) => c.split_codes(&bytes).len(),
        Err(_) => 0,
    });
    assert!(n <= 100_000);
}

#[test]
fn cid_cmap_with_a_normal_number_of_codespace_ranges_still_splits() {
    let mut s = String::from("200 begincodespacerange\n");
    for i in 0..200u32 {
        s.push_str(&format!("<{:04X}> <{:04X}>\n", i * 4, i * 4 + 3));
    }
    s.push_str("endcodespacerange\nendcmap\n");
    let (cmap, _) = CidCMap::parse(&s).unwrap();
    // 0x0001 and 0x0007 lie in ranges. No range starts with the byte 0x70, so it is a one-byte
    // code; the byte 0x00 after it starts a range but has no partner, so it is a one-byte code
    // too.
    let parts = cmap.split_codes(&[0x00, 0x01, 0x00, 0x07, 0x70, 0x00]);
    assert_eq!(parts, vec![(0x0001, 2), (0x0007, 2), (0x70, 1), (0x00, 1)]);
}

#[test]
fn many_usecmap_keywords_are_scanned_in_linear_time() {
    let s = "usecmap ".repeat(150_000);
    let ok = bounded("150000 usecmap keywords", move || CidCMap::parse(&s).is_ok());
    assert!(ok);
}

#[test]
fn usecmap_names_the_last_name_before_it() {
    let (_, base) = CidCMap::parse("/A def /B /C usecmap").unwrap();
    assert_eq!(base.as_deref(), Some("C"));
    let (_, base) = CidCMap::parse("/A usecmap 1 begincidchar <41> 1 endcidchar /Z usecmap").unwrap();
    assert_eq!(base.as_deref(), Some("Z"));
    let (_, base) = CidCMap::parse("usecmap").unwrap();
    assert_eq!(base, None);
}

#[test]
fn split_codes_partitions_every_input() {
    let src = "3 begincodespacerange <00> <7F> <8000> <9FFF> <A0000000> <FFFFFFFF> \
               endcodespacerange endcmap";
    let (cmap, _) = CidCMap::parse(src).unwrap();
    let identity = CidCMap::identity(0);
    bounded("split_codes", move || {
        let mut rng = Rng(0x0123_4567_89AB_CDEF);
        for len in 0..80usize {
            let bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
            for c in [&cmap, &identity] {
                let parts = c.split_codes(&bytes);
                let total: usize = parts.iter().map(|&(_, l)| usize::from(l)).sum();
                assert_eq!(total, bytes.len(), "{bytes:?}");
                assert!(parts.iter().all(|&(_, l)| l >= 1));
            }
        }
    });
}

#[test]
fn to_unicode_map_bytes_takes_any_length() {
    let m = ToUnicodeCMap::parse("beginbfchar <0102> <0041> endbfchar").unwrap();
    assert_eq!(m.map_bytes(&[]), "");
    assert_eq!(m.map_bytes(&[1]), "");
    assert_eq!(m.map_bytes(&[1, 2]), "A");
    assert_eq!(m.map_bytes(&[1, 2, 1]), "A");
    assert_eq!(m.map_bytes(&[1, 2, 1, 2]), "AA");
}

#[test]
fn to_unicode_stream_over_the_size_limit_is_an_error() {
    let mut doc = lopdf::Document::with_version("1.5");
    let mut content = b"begincmap 1 beginbfchar <0001> <0041> endbfchar endcmap".to_vec();
    let small = content.clone();
    content.resize(17 << 20, b' ');
    let mut big = Stream::new(dictionary! {}, content);
    big.compress().unwrap();
    let big_id = doc.add_object(big);
    let small_id = doc.add_object(Stream::new(dictionary! {}, small));

    let mut big_font = lopdf::Dictionary::new();
    big_font.set("ToUnicode", Object::Reference(big_id));
    let mut small_font = lopdf::Dictionary::new();
    small_font.set("ToUnicode", Object::Reference(small_id));

    let (big_result, small_result) = bounded_for(30, "ToUnicode streams", move || {
        (
            get_to_unicode_cmap_from_font(&big_font, &doc).map(|m| m.mappings.len()),
            get_to_unicode_cmap_from_font(&small_font, &doc).map(|m| m.mappings.len()),
        )
    });
    assert!(big_result.is_err(), "a 17 MiB CMap stream must be refused: {big_result:?}");
    assert_eq!(small_result, Ok(1));
}

// ---------------------------------------------------------------------------------------------
// Fonts (src/font.rs)
// ---------------------------------------------------------------------------------------------

#[cfg(feature = "text_layout")]
mod fonts {
    use super::*;

    const MOCK_TTF: &[u8] = include_bytes!("assets/fonts/mock/mock_ttf.ttf");

    /// A CFF INDEX with four-byte offsets.
    fn cff_index(items: &[Vec<u8>]) -> Vec<u8> {
        let mut out = (items.len() as u16).to_be_bytes().to_vec();
        if items.is_empty() {
            return out;
        }
        out.push(4);
        let mut off = 1u32;
        out.extend_from_slice(&off.to_be_bytes());
        for item in items {
            off += item.len() as u32;
            out.extend_from_slice(&off.to_be_bytes());
        }
        for item in items {
            out.extend_from_slice(item);
        }
        out
    }

    /// A DICT integer operand that always takes five bytes, so the size of a DICT does not
    /// depend on its values.
    fn int5(v: i32) -> [u8; 5] {
        let b = v.to_be_bytes();
        [29, b[0], b[1], b[2], b[3]]
    }

    /// `[0.0002 0 0 0.0002 0 0] FontMatrix`: 5000 units per em.
    const FONT_MATRIX_5000: [u8; 16] = [
        0x1e, 0x0a, 0x00, 0x02, 0xff, 0x8b, 0x8b, 0x1e, 0x0a, 0x00, 0x02, 0xff, 0x8b, 0x8b, 0x0c,
        0x07,
    ];

    /// A bare name-keyed CFF font with the given top dict prefix, charstrings and global
    /// subroutines.
    fn build_cff(top_prefix: &[u8], charstrings: &[Vec<u8>], global_subrs: &[Vec<u8>]) -> Vec<u8> {
        let header = [1u8, 0, 4, 4];
        let name = cff_index(&[b"T".to_vec()]);
        let strings = cff_index(&[]);
        let gsubrs = cff_index(global_subrs);
        let cs = cff_index(charstrings);
        let private = vec![0x8b, 20]; // defaultWidthX 0
        let top_len = top_prefix.len() + 6 + 6 + 11;
        let top_index_len = 2 + 1 + 4 * 2 + top_len;
        let cs_off = header.len() + name.len() + top_index_len + strings.len() + gsubrs.len();
        let private_off = cs_off + cs.len();
        let mut top = top_prefix.to_vec();
        top.extend_from_slice(&int5(0));
        top.push(15); // charset: ISOAdobe
        top.extend_from_slice(&int5(cs_off as i32));
        top.push(17); // CharStrings
        top.extend_from_slice(&int5(private.len() as i32));
        top.extend_from_slice(&int5(private_off as i32));
        top.push(18); // Private
        assert_eq!(top.len(), top_len);
        let top_index = cff_index(&[top]);
        assert_eq!(top_index.len(), top_index_len);
        [
            header.as_slice(),
            &name,
            &top_index,
            &strings,
            &gsubrs,
            &cs,
            &private,
        ]
        .concat()
    }

    /// `dup mul` three times: 32767^8, far beyond `u64`.
    fn huge_number() -> Vec<u8> {
        let mut c = vec![28, 0x7f, 0xff];
        for _ in 0..3 {
            c.extend_from_slice(&[0x0c, 0x1b, 0x0c, 0x18]);
        }
        c
    }

    #[test]
    fn handmade_cff_parses() {
        // The builder is right: a plain charstring (width 100, then `endchar`) gives the advance
        // 100.
        let width_100 = vec![28, 0, 100, 14];
        let cff = build_cff(&[], &[width_100], &[]);
        let f = bounded("plain CFF", move || ParsedFont::from_bytes(&cff, 0, &mut Vec::new()));
        assert!(f.is_some(), "the hand-made CFF must parse");
    }

    #[test]
    fn cff_subroutine_fan_out_does_not_hang() {
        // Ten levels of subroutines, each calling the next one 20 times: 20^10 calls before the
        // first stack-clearing operator, for one glyph.
        let calls = 20;
        let mut subrs = Vec::new();
        for level in 0..10u8 {
            let mut body = Vec::new();
            if level < 9 {
                for _ in 0..calls {
                    body.extend_from_slice(&[level + 1 + 32, 29]); // callgsubr level+1
                }
            }
            body.push(11); // return
            subrs.push(body);
        }
        let mut glyph = Vec::new();
        for _ in 0..calls {
            glyph.extend_from_slice(&[32, 29]); // callgsubr 0
        }
        glyph.push(14); // endchar
        let cff = build_cff(&[], &[glyph], &subrs);
        let _ = bounded("CFF subroutine fan-out", move || {
            ParsedFont::from_bytes(&cff, 0, &mut Vec::new()).is_some()
        });
    }

    #[test]
    fn cff_with_a_large_units_per_em_does_not_overflow() {
        // FontMatrix 0.0002 is 5000 units per em; the ascender 5000 * 8 / 10 does not fit an i16
        // when computed in one.
        let cff = build_cff(&FONT_MATRIX_5000, &[vec![14]], &[]);
        let _ = bounded("CFF with 5000 units per em", move || {
            ParsedFont::from_bytes(&cff, 0, &mut Vec::new()).is_some()
        });
    }

    #[test]
    fn cff_index_operator_with_a_huge_operand_does_not_overflow() {
        let mut glyph = huge_number();
        glyph.extend_from_slice(&[0x0c, 0x1d, 14]); // index, endchar
        let cff = build_cff(&[], &[glyph], &[]);
        let _ = bounded("CFF index operator", move || {
            ParsedFont::from_bytes(&cff, 0, &mut Vec::new()).is_some()
        });
    }

    #[test]
    fn cff_subroutine_call_with_a_huge_operand_does_not_overflow() {
        let mut glyph = huge_number();
        glyph.extend_from_slice(&[29, 14]); // callgsubr, endchar
        let cff = build_cff(&[], &[glyph], &[]);
        let _ = bounded("CFF callgsubr", move || {
            ParsedFont::from_bytes(&cff, 0, &mut Vec::new()).is_some()
        });
    }

    #[test]
    fn cff_roll_and_stack_operators_with_odd_operands_do_not_panic() {
        // roll with n larger than the stack, a negative n, a huge j; exch and drop on an empty
        // stack; every operand count in front of hstem and rmoveto.
        let mut glyphs: Vec<Vec<u8>> = vec![
            vec![0x8b, 0x8b, 0x0c, 0x1e, 14],             // 0 0 roll
            vec![32, 0x8c, 0x0c, 0x1e, 14],               // -107 1 roll
            vec![0x8c, 0xf7, 0x00, 0x0c, 0x1e, 14],       // 1 108 roll
            vec![0x0c, 0x1c, 14],                         // exch on an empty stack
            vec![0x0c, 0x12, 14],                         // drop on an empty stack
            vec![0x8c, 0x0c, 0x1d, 14],                   // 1 index
            vec![0x8c, 0x8c, 0x8c, 0x8c, 0x0c, 0x16, 14], // ifelse
            vec![0x8c, 1, 14],                            // 1 hstem
            vec![0x8c, 0x8c, 21, 14],                     // 1 1 rmoveto
            vec![0x8c, 0x8c, 0x8c, 21, 14],               // 1 1 1 rmoveto
            vec![0x0c, 0x0c, 14],                         // div on an empty stack
            vec![0x8b, 0x8b, 0x0c, 0x0c, 14],             // 0 0 div
            vec![255, 0, 0, 0, 0, 14],                    // 16.16 zero
            vec![255, 0xff, 0xff],                        // 16.16 number cut short
            vec![28, 0],                                  // 16-bit number cut short
            vec![0x0c],                                   // escape cut short
            vec![247],                                    // two-byte number cut short
            vec![251],
            vec![10],  // callsubr without a local subroutine index
            vec![29],  // callgsubr on an empty stack
            vec![11],  // return in the charstring itself
        ];
        glyphs.push(vec![14]);
        let cff = build_cff(&[], &glyphs, &[]);
        let _ = bounded("CFF odd operands", move || {
            ParsedFont::from_bytes(&cff, 0, &mut Vec::new()).is_some()
        });
    }

    #[test]
    fn width_arrays_accept_the_largest_glyph_id() {
        let (a, b) = bounded("width arrays", || {
            let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).unwrap();
            let ids = vec![
                (65535u16, "a".to_string()),
                (0u16, "b".to_string()),
                (65535, "c".to_string()),
            ];
            let entries = vec![(65535u16, 1u16), (0, 1), (65535, 2)];
            (
                get_normalized_widths_ttf(&font, &ids).len(),
                get_normalized_widths_codes(&font, &entries).len(),
            )
        });
        assert!(a > 0 && b > 0);
    }

    #[test]
    fn width_arrays_of_consecutive_codes_are_one_run() {
        let n = bounded("width run", || {
            let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).unwrap();
            get_normalized_widths_codes(&font, &[(1, 1), (2, 2), (3, 3)]).len()
        });
        assert_eq!(n, 2, "one `first [w1 w2 w3]` pair");
    }

    #[test]
    fn a_font_with_zero_units_per_em_has_sane_widths() {
        // head.unitsPerEm (offset 18 of the table) set to 0.
        let mut ttf = MOCK_TTF.to_vec();
        let tables = usize::from(u16::from_be_bytes([ttf[4], ttf[5]]));
        for t in 0..tables {
            let rec = 12 + 16 * t;
            if &ttf[rec..rec + 4] == b"head" {
                let off = u32::from_be_bytes(ttf[rec + 8..rec + 12].try_into().unwrap()) as usize;
                ttf[off + 18] = 0;
                ttf[off + 19] = 0;
            }
        }
        let widths = bounded("zero units per em", move || {
            let font = ParsedFont::from_bytes(&ttf, 0, &mut Vec::new())?;
            Some(get_normalized_widths_codes(&font, &[(1, 1), (2, 2)]))
        });
        // The parser may refuse such a font. When it does not, no width is absurd.
        if let Some(widths) = widths {
            for w in widths {
                if let Object::Array(a) = w {
                    for v in a {
                        let n = v.as_i64().unwrap();
                        assert!(n.abs() < 1_000_000, "width {n}");
                    }
                }
            }
        }
    }

    #[test]
    fn subsetting_odd_glyph_sets_does_not_panic() {
        bounded("subset_font", || {
            let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).unwrap();
            let none = std::collections::BTreeMap::new();
            let _ = subset_font(&font, &none);
            let mut only_notdef = std::collections::BTreeMap::new();
            only_notdef.insert(0u16, String::new());
            let _ = subset_font(&font, &only_notdef);
            let mut beyond = std::collections::BTreeMap::new();
            beyond.insert(1u16, "a".to_string());
            beyond.insert(60_000u16, "b".to_string());
            let _ = subset_font(&font, &beyond);
        });
    }

    #[test]
    fn empty_and_tiny_font_files_are_rejected() {
        bounded("tiny fonts", || {
            let inputs: [&[u8]; 6] = [&[], &[0u8], b"OTTO", b"ttcf", b"\0\x01\0\0", b"true"];
            for bytes in inputs {
                let _ = ParsedFont::from_bytes(bytes, 0, &mut Vec::new());
                let _ = extract_collection_face(bytes, 0);
                let _ = extract_cid_keyed_cff(bytes, 0);
                let _ = cff_charset_gid_to_cid_map(bytes, 0);
            }
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Text (src/text.rs, src/text_boxes.rs)
// ---------------------------------------------------------------------------------------------

#[test]
fn pdf_strings_of_any_length_decode() {
    let none = None::<&ToUnicodeCMap>;
    let cases: &[&[u8]] = &[
        &[],
        &[0xFE],
        &[0xFE, 0xFF],
        &[0xFE, 0xFF, 0x00],
        &[0xFE, 0xFF, 0x00, 0x41],
        &[0xFE, 0xFF, 0xD8, 0x00],
        &[0xFE, 0xFF, 0xD8, 0x00, 0xDC, 0x00],
        &[0xFF, 0xFE],
        &[0xEF, 0xBB, 0xBF],
        &[0xFE, 0xFF, 0xDC, 0x00, 0xD8, 0x00, 0x00],
    ];
    for case in cases {
        for format in [StringFormat::Hexadecimal, StringFormat::Literal] {
            let _ = decode_pdf_string(&Object::String(case.to_vec(), format), none);
        }
    }
    let hex = |b: &[u8]| decode_pdf_string(&Object::String(b.to_vec(), StringFormat::Hexadecimal), none);
    assert_eq!(hex(&[0xFE, 0xFF, 0x00, 0x41]), "A");
    assert_eq!(hex(&[0xFE, 0xFF, 0x00, 0x41, 0x42]), "A", "a trailing odd byte is dropped");
    assert_eq!(hex(&[0xFE, 0xFF]), "");
    assert_eq!(hex(&[]), "");
    assert_eq!(decode_pdf_string(&Object::Null, none), "");
}

#[test]
fn glyph_id_decoding_takes_any_string_length() {
    for len in 0..6usize {
        let bytes: Vec<u8> = (0..len as u8).collect();
        let items = decode_tj_string_as_glyph_ids(&bytes);
        let ops = [Object::String(bytes.clone(), StringFormat::Hexadecimal), Object::Integer(5)];
        let items2 = decode_tj_operands_as_glyph_ids(&ops);
        let count = |items: &[TextItem]| -> usize {
            items
                .iter()
                .map(|i| if let TextItem::GlyphIds(g) = i { g.len() } else { 0 })
                .sum()
        };
        let expected = if len >= 2 && len % 2 == 0 { len / 2 } else { len };
        assert_eq!(count(&items), expected, "length {len}");
        assert_eq!(count(&items2), expected, "length {len}");
    }
}

#[test]
fn text_items_encode_for_any_string() {
    for s in ["", "a", "(a)", "\u{e9}", "\u{65e5}\u{672c}\u{8a9e}", "\u{1d456}", "\0", "\\"] {
        let objs = encode_text_items(&[TextItem::Text(s.to_string())]);
        assert_eq!(objs.len(), 1);
        let _ = encode_pdf_string_minimal(s);
    }
}

#[test]
fn text_boxes_of_every_builtin_font() {
    for font in BuiltinFont::all_ids() {
        let ops = vec![
            Op::StartTextSection,
            Op::SetTextCursor { pos: Point { x: Pt(72.0), y: Pt(700.0) } },
            Op::SetFont { font: PdfFontHandle::Builtin(font), size: Pt(12.0) },
            Op::ShowText { items: vec![TextItem::Text("Hello world".to_string())] },
            Op::EndTextSection,
        ];
        let mut doc = PdfDocument::new("boxes");
        doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
        let pages = bounded("builtin text boxes", move || doc.extract_text_boxes());
        let words = &pages[0].lines[0].words;
        assert_eq!(words[0].text, "Hello", "{font:?}");
        assert_eq!(words[0].builtin, Some(font));
    }
}

#[test]
fn text_boxes_with_a_missing_font_use_a_fixed_advance() {
    let ops = vec![
        Op::StartTextSection,
        Op::SetFont { font: PdfFontHandle::External(FontId("nope".to_string())), size: Pt(10.0) },
        Op::ShowText { items: vec![TextItem::Text("ab".to_string())] },
        Op::EndTextSection,
    ];
    let mut doc = PdfDocument::new("boxes");
    doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
    let pages = bounded("missing font", move || doc.extract_text_boxes());
    let word = &pages[0].lines[0].words[0];
    let [x0, _, x1, _] = word.bbox;
    assert!((x1 - x0 - 10.0).abs() < 0.01, "two glyphs of 0.5 em at 10 pt: {:?}", word.bbox);
}

// ---------------------------------------------------------------------------------------------
// Shadings (src/shading.rs)
// ---------------------------------------------------------------------------------------------

#[test]
fn shading_dictionaries_for_odd_stops() {
    let stop = |offset: f32, r: f32| GradientStop { offset, color: [r, 0.0, 0.0] };
    let cases: Vec<Vec<GradientStop>> = vec![
        vec![],
        vec![stop(0.5, 1.0)],
        vec![stop(0.0, 1.0), stop(1.0, 0.0)],
        vec![stop(f32::NAN, 1.0), stop(f32::NAN, 0.0)],
        vec![stop(f32::NAN, 1.0), stop(0.5, 0.0), stop(f32::NAN, 0.5)],
        vec![stop(1.0, 1.0), stop(0.0, 0.0)],
        vec![stop(0.9, 1.0), stop(0.1, 0.5), stop(0.5, 0.0), stop(0.2, 0.2)],
        vec![stop(0.5, 1.0), stop(0.5, 0.0)],
        vec![stop(f32::INFINITY, 1.0), stop(f32::NEG_INFINITY, 0.0)],
        vec![stop(-5.0, 1.0), stop(5.0, 0.0)],
        vec![stop(0.0, f32::NAN), stop(1.0, f32::INFINITY)],
    ];
    bounded("shadings", move || {
        for stops in cases {
            for geometry in [
                ShadingGeometry::Axial { coords: [0.0; 4] },
                ShadingGeometry::Radial { coords: [f32::NAN; 6] },
            ] {
                let dict = Shading { geometry, stops: stops.clone(), extend: (true, false) }.to_dict();
                assert!(dict.has(b"Function"));
                assert!(dict.has(b"Coords"));
            }
        }
    });
}

#[test]
fn shading_stitching_function_has_matching_counts() {
    let stops = vec![
        GradientStop { offset: 0.2, color: [1.0, 0.0, 0.0] },
        GradientStop { offset: 0.6, color: [0.0, 1.0, 0.0] },
        GradientStop { offset: 0.8, color: [0.0, 0.0, 1.0] },
    ];
    let dict = Shading {
        geometry: ShadingGeometry::Axial { coords: [0.0, 0.0, 1.0, 0.0] },
        stops,
        extend: (true, true),
    }
    .to_dict();
    let f = dict.get(b"Function").unwrap().as_dict().unwrap();
    let functions = f.get(b"Functions").unwrap().as_array().unwrap().len();
    let bounds = f.get(b"Bounds").unwrap().as_array().unwrap().len();
    let encode = f.get(b"Encode").unwrap().as_array().unwrap().len();
    assert_eq!(functions, 4, "constant, two segments, constant");
    assert_eq!(bounds, functions - 1);
    assert_eq!(encode, 2 * functions);
}

// ---------------------------------------------------------------------------------------------
// Units (src/units.rs)
// ---------------------------------------------------------------------------------------------

const ODD_FLOATS: [f32; 15] = [
    f32::NAN,
    -f32::NAN,
    f32::INFINITY,
    f32::NEG_INFINITY,
    0.0,
    -0.0,
    1.0,
    -1.0,
    1.0004,
    2.5,
    f32::MAX,
    f32::MIN,
    f32::MIN_POSITIVE,
    1e-40,
    -1e-40,
];

fn check_total_order<T: Ord + Copy + std::fmt::Debug>(values: &[T]) {
    for &a in values {
        assert_eq!(a.partial_cmp(&a), Some(Ordering::Equal), "{a:?} against itself");
        for &b in values {
            assert_eq!(a.cmp(&b), b.cmp(&a).reverse(), "{a:?} {b:?}");
            assert_eq!(a.partial_cmp(&b), Some(a.cmp(&b)), "{a:?} {b:?}");
            for &c in values {
                if a.cmp(&b) != Ordering::Greater && b.cmp(&c) != Ordering::Greater {
                    assert_ne!(a.cmp(&c), Ordering::Greater, "{a:?} <= {b:?} <= {c:?}");
                }
            }
        }
    }
}

#[test]
fn pt_and_mm_are_totally_ordered() {
    let pts: Vec<Pt> = ODD_FLOATS.iter().map(|&f| Pt(f)).collect();
    let mms: Vec<Mm> = ODD_FLOATS.iter().map(|&f| Mm(f)).collect();
    check_total_order(&pts);
    check_total_order(&mms);
}

#[test]
fn pt_and_mm_order_zeros_and_nan_like_total_cmp() {
    assert_eq!(Pt(-0.0).cmp(&Pt(0.0)), Ordering::Less);
    assert_eq!(Mm(-0.0).cmp(&Mm(0.0)), Ordering::Less);
    assert_eq!(Pt(f32::NAN).cmp(&Pt(f32::INFINITY)), Ordering::Greater);
    assert_eq!(Pt(-f32::NAN).cmp(&Pt(f32::NEG_INFINITY)), Ordering::Less);
    assert_eq!(Pt(f32::NAN).cmp(&Pt(f32::NAN)), Ordering::Equal);
    assert_eq!(Pt(1.0).cmp(&Pt(2.0)), Ordering::Less);
    assert!(Pt(1.0) < Pt(2.0));
    assert!(Mm(2.0) > Mm(1.0));
    // Ordinary values order as before.
    assert_eq!(Pt(1.0001).cmp(&Pt(1.0004)), Ordering::Less);
}

#[test]
fn sorting_lengths_with_nan_does_not_panic() {
    let sorted = bounded("sort of Pt with NaN", || {
        let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
        let mut all_sorted = true;
        for round in 0..40 {
            let n = 40 + round * 13;
            let mut pts: Vec<Pt> = (0..n)
                .map(|_| match rng.below(5) {
                    0 => Pt(f32::NAN),
                    1 => Pt(-0.0),
                    2 => Pt(0.0),
                    _ => Pt((rng.next() % 1000) as f32 / 10.0 - 50.0),
                })
                .collect();
            let mut mms: Vec<Mm> = pts.iter().map(|p| Mm(p.0)).collect();
            pts.sort();
            mms.sort_unstable();
            all_sorted &= pts.windows(2).all(|w| w[0].cmp(&w[1]) != Ordering::Greater);
            all_sorted &= mms.windows(2).all(|w| w[0].cmp(&w[1]) != Ordering::Greater);
            let _ = pts.iter().max();
            let _ = mms.iter().min();
        }
        all_sorted
    });
    assert!(sorted);
}

#[test]
fn pt_and_mm_keep_the_tolerant_equality() {
    // Ordering is exact; equality still compares to three decimals.
    assert_eq!(Pt(1.0001), Pt(1.0004));
    assert_eq!(Mm(210.0).into_pt(), Pt(595.276));
    // `Eq` needs x == x for every value, so NaN equals NaN (issue #292).
    assert_eq!(Pt(f32::NAN), Pt(f32::NAN));
    assert_ne!(Pt(f32::NAN), Pt(1.0));
    assert_ne!(Pt(f32::INFINITY), Pt(f32::NEG_INFINITY));
}

#[test]
fn px_arithmetic_does_not_overflow() {
    bounded("Px arithmetic", || {
        assert_eq!(Px(1) - Px(2), Px(0));
        assert_eq!(Px(usize::MAX) + Px(1), Px(usize::MAX));
        assert_eq!(Px(5) - Px(2), Px(3));
        assert_eq!(Px(5) + Px(2), Px(7));
        let mut p = Px(1);
        p -= Px(5);
        assert_eq!(p, Px(0));
        let mut q = Px(usize::MAX);
        q += Px(1);
        assert_eq!(q, Px(usize::MAX));
    });
}

#[test]
fn unit_conversions_of_odd_values() {
    for f in ODD_FLOATS {
        let _ = Pt(f).into_px(300.0);
        let _ = Pt(1.0).into_px(f);
        let _ = Px(usize::MAX).into_pt(f);
        let _ = Px(3).into_pt(0.0);
        let _: Mm = Pt(f).into();
        let _: Pt = Mm(f).into();
        let _ = Pt(f) / Pt(0.0);
        let _ = Mm(f) / 0.0;
    }
}

// ---------------------------------------------------------------------------------------------
// The document (src/lib.rs)
// ---------------------------------------------------------------------------------------------

/// A page that `marks` identifies: it has that many operations.
fn marked_page(marks: usize) -> PdfPage {
    PdfPage::new(Mm(100.0), Mm(100.0), vec![Op::SaveGraphicsState; marks])
}

fn marks(doc: &PdfDocument) -> Vec<usize> {
    doc.pages.iter().map(|p| p.ops.len()).collect()
}

fn doc_with(pages: &[usize]) -> PdfDocument {
    let mut doc = PdfDocument::new("d");
    doc.pages = pages.iter().map(|&m| marked_page(m)).collect();
    doc
}

fn bookmark(doc: &mut PdfDocument, name: &str, page: usize) {
    doc.bookmarks.map.insert(
        PageAnnotId(name.to_string()),
        PageAnnotation { name: name.to_string(), page },
    );
}

#[test]
fn insert_document_at_every_position() {
    let expected = [
        (0, vec![10, 11, 12, 1, 2]),
        (1, vec![1, 10, 11, 12, 2]),
        (2, vec![1, 2, 10, 11, 12]),
        (3, vec![1, 2, 10, 11, 12]),
        (usize::MAX, vec![1, 2, 10, 11, 12]),
    ];
    for (at, want) in expected {
        let mut main = doc_with(&[1, 2]);
        main.insert_document(at, doc_with(&[10, 11, 12]));
        assert_eq!(marks(&main), want, "insert_at {at}");
    }
}

#[test]
fn insert_document_with_an_empty_side() {
    let mut a = PdfDocument::new("a");
    a.insert_document(5, PdfDocument::new("b"));
    assert!(a.pages.is_empty());

    let mut b = PdfDocument::new("a");
    b.insert_document(usize::MAX, doc_with(&[7]));
    assert_eq!(marks(&b), vec![7]);

    let mut c = doc_with(&[1, 2]);
    c.insert_document(1, PdfDocument::new("empty"));
    assert_eq!(marks(&c), vec![1, 2]);

    let mut d = doc_with(&[1]);
    d.append_document(PdfDocument::new("empty"));
    assert_eq!(marks(&d), vec![1]);
}

#[test]
fn bookmarks_follow_the_inserted_pages() {
    let mut main = doc_with(&[1, 2]);
    bookmark(&mut main, "second", 2);
    let mut other = doc_with(&[10]);
    bookmark(&mut other, "other", 1);
    main.insert_document(0, other);
    let pages: Vec<(String, usize)> =
        main.bookmarks.map.values().map(|b| (b.name.clone(), b.page)).collect();
    assert!(pages.contains(&("second".to_string(), 3)), "{pages:?}");
    assert!(pages.contains(&("other".to_string(), 1)), "{pages:?}");
}

#[test]
fn bookmark_page_numbers_saturate_instead_of_overflowing() {
    bounded("bookmarks with huge page numbers", || {
        // A bookmark of this document that is moved behind the inserted pages.
        let mut main = doc_with(&[1, 2]);
        bookmark(&mut main, "far", usize::MAX);
        main.insert_document(0, doc_with(&[10, 11]));
        assert_eq!(main.bookmarks.map.values().next().unwrap().page, usize::MAX);

        // A bookmark of the inserted document that is moved behind this document's pages.
        let mut main = doc_with(&[1, 2]);
        let mut other = doc_with(&[10]);
        bookmark(&mut other, "far", usize::MAX);
        main.insert_document(1, other);
        assert_eq!(main.bookmarks.map.values().next().unwrap().page, usize::MAX);

        let mut main = doc_with(&[1, 2]);
        let mut other = doc_with(&[10]);
        bookmark(&mut other, "far", usize::MAX);
        main.append_document(other);
        assert_eq!(main.bookmarks.map.values().next().unwrap().page, usize::MAX);
    });
}

#[test]
fn add_bookmark_takes_any_page_number() {
    let mut doc = PdfDocument::new("d");
    for page in [0, 1, usize::MAX] {
        let _ = doc.add_bookmark("b", page);
    }
    assert_eq!(doc.bookmarks.map.len(), 3);
}

#[test]
fn page_to_svg_out_of_range() {
    let mut warnings = Vec::new();
    let opts = PdfToSvgOptions::default();
    let empty = PdfDocument::new("empty");
    for page in [0, 1, 2, usize::MAX] {
        assert!(empty.page_to_svg(page, &opts, &mut warnings).is_none(), "page {page}");
    }
    let one = doc_with(&[0]);
    assert!(one.page_to_svg(0, &opts, &mut warnings).is_some(), "page 0 is page 1");
    assert!(one.page_to_svg(1, &opts, &mut warnings).is_some());
    assert!(one.page_to_svg(2, &opts, &mut warnings).is_none());
    assert!(one.page_to_svg(usize::MAX, &opts, &mut warnings).is_none());
}

#[test]
fn text_of_an_empty_document() {
    let doc = PdfDocument::new("empty");
    assert!(doc.extract_text().is_empty());
    assert!(doc.extract_text_boxes().is_empty());
    assert_eq!(doc.page_count(), 0);
}

#[test]
fn document_info_defaults_to_the_epoch() {
    let info = PdfDocumentInfo::default();
    assert_eq!(info.creation_date, OffsetDateTime::epoch());
    assert_eq!(info.modification_date, OffsetDateTime::epoch());
    assert_eq!(info.metadata_date, OffsetDateTime::epoch());
    assert_eq!(info.creation_date.year(), 1970);
}

#[test]
fn base64_values_of_any_shape_decode_or_fail() {
    for s in [
        "",
        "data:",
        "data:,",
        "data:;base64,",
        "data:base64,",
        "data:text/plain,abc",
        "data:\u{e9},base64",
        "\u{e9}",
        "data:;base64,\u{e9}",
        ",",
    ] {
        let _ = Base64OrRaw::B64(s.to_string()).decode_bytes();
    }
    assert_eq!(Base64OrRaw::B64("data:;base64,aGk=".to_string()).decode_bytes(), Ok(b"hi".to_vec()));
    assert_eq!(Base64OrRaw::B64("aGk=".to_string()).decode_bytes(), Ok(b"hi".to_vec()));
    assert_eq!(Base64OrRaw::B64("data:text/plain,aGk=".to_string()).decode_bytes().is_err(), true);
}

#[test]
fn generated_ids_are_32_letters() {
    for _ in 0..2000 {
        for id in [FontId::new().0, XObjectId::new().0, PageAnnotId::new().0] {
            assert_eq!(id.len(), 32);
            assert!(id.bytes().all(|b| (b'A'..=b'J').contains(&b)), "{id}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The optimizer (src/optimize.rs)
// ---------------------------------------------------------------------------------------------

const ONE_TO_ONE_CMAP: &[u8] = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
    1 begincodespacerange <00> <FF> endcodespacerange\n\
    1 beginbfchar <41> <0041> endbfchar\nendcmap\n";

/// A one-page PDF whose text `(A)` sits in a span with the redundant `/ActualText (A)`. The
/// font's ToUnicode stream is padded with `padding` spaces. `split` puts the span's `BDC` and
/// its `EMC` into two content streams.
fn actual_text_pdf(padding: usize, split: bool, pages: usize, share: bool) -> Vec<u8> {
    let head = b"BT /F1 12 Tf 10 10 Td /Span << /ActualText (A) >> BDC".to_vec();
    let tail = b"(A) Tj EMC ET".to_vec();
    let streams = if split { vec![head, tail] } else { vec![[head, b" ".to_vec(), tail].concat()] };
    text_pdf(padding, streams, pages, share)
}

/// A PDF of `pages` pages that show `streams` as their content. Font `/F1` maps the code 0x41
/// to "A" in its ToUnicode stream, which is padded with `padding` spaces. With `share` the
/// pages use the same content stream objects, else each has its own copy.
fn text_pdf(padding: usize, streams: Vec<Vec<u8>>, pages: usize, share: bool) -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let mut cmap = ONE_TO_ONE_CMAP.to_vec();
    cmap.resize(cmap.len() + padding, b' ');
    let mut to_unicode = Stream::new(dictionary! {}, cmap);
    if padding > 0 {
        to_unicode.compress().unwrap();
    }
    let to_unicode_id = doc.add_object(to_unicode);
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "ToUnicode" => Object::Reference(to_unicode_id),
    });

    let contents: Vec<Object> = streams
        .into_iter()
        .map(|data| Object::Reference(doc.add_object(Stream::new(dictionary! {}, data))))
        .collect();

    let mut kids = Vec::new();
    for _ in 0..pages {
        let contents = if share {
            contents.clone()
        } else {
            // A private copy of the streams for this page.
            contents
                .iter()
                .map(|c| {
                    let id = c.as_reference().unwrap();
                    let stream = doc.get_object(id).unwrap().clone();
                    Object::Reference(doc.add_object(stream))
                })
                .collect()
        };
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => Object::Reference(font_id) } },
            "Contents" => contents,
        });
        kids.push(Object::Reference(page_id));
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => pages as i64,
            "Kids" => kids,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn optimize_message(pdf: Vec<u8>) -> String {
    bounded_for(30, "optimize_text", move || {
        let mut warnings = Vec::new();
        optimize_text(&pdf, &mut warnings).expect("optimize_text");
        warnings.last().map(|w| w.msg.clone()).unwrap_or_default()
    })
}

#[test]
fn optimize_text_drops_a_redundant_span() {
    let msg = optimize_message(actual_text_pdf(0, false, 1, false));
    assert!(msg.contains("1 spans dropped, 0 kept"), "{msg}");
}

#[test]
fn optimize_text_drops_a_span_whose_page_has_several_content_streams() {
    let msg = optimize_message(actual_text_pdf(0, true, 1, false));
    assert!(msg.contains("1 spans dropped, 0 kept"), "{msg}");
    let msg = optimize_message(actual_text_pdf(0, true, 3, false));
    assert!(msg.contains("3 spans dropped, 0 kept"), "{msg}");
}

#[test]
fn optimize_text_leaves_shared_content_streams_alone() {
    let msg = optimize_message(actual_text_pdf(0, true, 2, true));
    assert!(msg.contains("0 spans dropped, 0 kept"), "{msg}");
}

#[test]
fn optimize_text_keeps_a_span_with_marked_content_inside() {
    // The inner span reads as its text and goes; the outer one has marked content inside and
    // stays.
    let content = b"BT /F1 12 Tf 10 10 Td /Span << /ActualText (A) >> BDC \
        /Span << /ActualText (A) >> BDC (A) Tj EMC EMC ET"
        .to_vec();
    let msg = optimize_message(text_pdf(0, vec![content], 1, false));
    assert!(msg.contains("1 spans dropped, 1 kept"), "{msg}");
}

#[test]
fn optimize_text_scans_deeply_nested_spans_in_linear_time() {
    // 200000 spans inside each other, the outer one with an /ActualText.
    let depth = 200_000;
    let mut content = b"BT /F1 12 Tf 10 10 Td /Span << /ActualText (A) >> BDC\n".to_vec();
    for _ in 0..depth {
        content.extend_from_slice(b"/Span BMC\n");
    }
    for _ in 0..depth {
        content.extend_from_slice(b"EMC\n");
    }
    content.extend_from_slice(b"EMC ET");
    let msg = optimize_message(text_pdf(0, vec![content], 1, false));
    assert!(msg.contains("0 spans dropped, 1 kept"), "{msg}");
}

#[test]
fn optimize_text_ignores_a_oversized_to_unicode_stream() {
    // 17 MiB of padding after a valid CMap, which a few KB of Flate holds. The font is not read,
    // so the span is kept.
    let msg = optimize_message(actual_text_pdf(17 << 20, false, 1, false));
    assert!(msg.contains("0 spans dropped, 1 kept"), "{msg}");
}
