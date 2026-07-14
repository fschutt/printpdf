# Changelog

## `0.11.1`

printpdf linked **two copies of allsorts** — `0.17` directly, and `0.16` again underneath
`rust-fontconfig`. Two font parsers means two incompatible `ParsedFont` / `CmapTarget` /
`FontData`, so a type from one cannot cross into the other, and the binary carries the whole
parser twice. `rust-fontconfig 4.4.5` moves to allsorts 0.17, and printpdf now requires it,
so exactly one copy is linked. `only_one_allsorts_is_linked` keeps it that way.

The subset-cmap panic reported in 0.11.0's notes was fixed upstream rather than worked
around: it only ever existed in `allsorts-azul` ≤ 0.16.5
(`CmapSubtableFormat4::from_mappings` did `mappings.iter().next().unwrap()` under a "safe as
mappings is non-empty" comment — it isn't). 0.17 handles an empty mapping set correctly,
emitting just the mandatory `0xFFFF` sentinel segment. printpdf still declines to subset a
font whose used glyphs are all unreachable from its cmap, but for a different reason: the
glyph renumbering is recovered *from the subset's own cmap*, so an empty one would leave the
content stream emitting original glyph ids against a renumbered font.

Upstream, `azul` no longer patches `allsorts-azul` to a vendored tree — the drift between
that tree and the published crate, while both claimed version 0.17.0, is what shipped
azul-layout 0.0.9 broken. It is consumed from the registry now, and azul CI fails if a
`[patch]` is placed on anything its published crates depend on.

## `0.11.0`

The headline is that **printpdf no longer carries a `[patch.crates-io]` section**. It builds
against exactly the crates it publishes. `cargo publish` silently strips `[patch]`, so a
crate released while one exists is compiled against dependencies nobody ever tested — the
local checkout and the published artifact are different programs. That is precisely how
0.10.0 shipped with every embedded font empty (#277), and the trap bit azul twice more on
the way out. It is now a CI failure for a `[patch]` section to exist at all.

That required releasing the dependencies for real: **azul-css 0.0.9**, **azul-core 0.0.9**,
**azul-layout 0.0.10** and **allsorts-azul 0.17.1**. The published `azul-layout 0.0.9`
SIGSEGVs on any hosted OS — it carried 48 ungated
`core::ptr::write_volatile(0x400EC as *mut u32, …)` calls (web-lift diagnostic markers
writing to a hardcoded absolute address, unmapped on native targets). 0.0.10 routes them
through a no-op unless the `web_lift` feature is on, so the HTML/layout path works again.

### Breaking

- `LineDashPattern` is now `{ offset: f32, pattern: SmallVec<[f32; 8]> }` — an
  arbitrary-length array of reals — rather than three fixed `Option<i64>` dash/gap pairs.
  The PDF spec always allowed this (#262, PR #263 by @tower120). Build one with
  `LineDashPattern::new(offset, &[dash, gap, …])` or `LineDashPattern::solid()`; you do not
  need a `smallvec` dependency of your own.
- `printpdf::ParsedFont` is a printpdf type (a newtype over `azul_layout::ParsedFont`, with
  `Deref`), which guarantees the source bytes stay attached. Field access and method calls
  are unchanged; `into_inner()` / `as_azul()` reach the azul face.
- Without `text_layout`, `FontParseWarning` is a struct rather than a `String` alias, so
  `ParsedFont::from_bytes` has the same signature with and without the feature (#260).
- `lopdf` 0.39 → 0.44, and `allsorts-azul` 0.16 → 0.17. Both appear in printpdf's public
  API, so pinning either yourself means bumping in step.

### Fixed

- **RUSTSEC-2026-0187** (unbounded-recursion DoS) — lopdf is on 0.44 (#272, PR #275 by
  @Zynora-fr).
- **The HTML renderer could take the process down.** `allsorts::subset()` builds the
  subset's cmap from the original font's cmap restricted to the kept glyphs, and *panics*
  — not errors — when that comes out empty (`mappings.iter().next().unwrap()`, under a
  "safe as mappings is non-empty" comment). Shaping emits glyph ids directly, and ligatures
  have no character of their own, so a page whose glyphs were all unreachable from the cmap
  aborted. printpdf now declines to subset in that case and embeds the full font.
- **External fonts did nothing without `text_layout`** (#258). The fallback
  `ParsedFont::from_bytes` never parsed the cmap, so every glyph resolved to `.notdef` and
  pages came out blank — while the font embedded perfectly, which is why it looked so
  baffling. And `PdfDocument::parse`'s `Type0` branch was feature-gated, so it could not
  read an external font back either. The fallback now parses cmap/hmtx/head/hhea/OS-2 with
  allsorts, which is a non-optional dependency and was there all along.
- **FontDescriptor `/ItalicAngle`, `/Flags` and `/StemV`** were hardcoded to `0 / 32 / 80`
  for every font — "upright, non-symbolic, medium weight", whatever you embedded. Readers
  use these to synthesise or substitute a face. They are derived from the hhea caret slope
  and OS/2 `usWeightClass` now (#271, residual).
- wasm builds broke on the lopdf bump: `getrandom` must have `wasm_js` enabled per major
  version, and lopdf pulls 0.4 alongside the 0.3 other deps use.

### Changed

- CI runs the full suite on Linux again. It had been commented out as "fails because of
  SIMD issues, test on Windows only", so the suite only ever ran on Windows — which is why
  a Linux-only font-resolution panic sat undetected. The no-default-features job runs the
  whole suite too, not just `--lib`, which is why the blank-page bug above went unnoticed.
- `tests/no_text_layout.rs` covers the fallback font path; `tests/external_tools.rs` checks
  the output with poppler (`pdffonts`, `pdftotext`), which shares no code with printpdf.

## `0.10.1` — font embedding hotfix

**If you are on `0.10.0`, upgrade.** In 0.10.0 every external font embedded as an *empty*
`/FontFile2`: readers reported "Cannot extract the embedded font", `pdffonts` reported
"Embedded font file may be invalid", and no glyph rendered. A PDF that should have been
165 KB came out at 2.7 KB. Nothing warned and nothing failed.

0.10.0 cannot be repaired by releasing a fixed `azul-layout`. Cargo reads
`azul-layout = "0.0.9"` as `^0.0.9` := `>=0.0.9, <0.0.10`, so for `0.0.z` versions *only*
`0.0.9` ever satisfies it — 0.10.0 is permanently pinned to the broken dependency. The fix
had to come from printpdf, and it does: printpdf now retains font bytes itself and no
longer depends on azul's retention policy at all.

### Fixed

- **External fonts embed as an empty `/FontFile2`.** `azul_layout::ParsedFont::from_bytes`
  does not retain the source bytes (a deliberate perf change — layout and rasterization
  never read them, and retaining them duplicated a 4.27 MiB `.ttc` once per face).
  printpdf read them straight off the struct and `.unwrap_or_default()`'d the `None` into
  an empty `Vec`. `printpdf::ParsedFont` is now its own type, which attaches the source
  bytes explicitly, so embedding is correct against any `azul-layout`.
- **Subset fonts had no `.notdef`.** allsorts requires glyph 0 to be present and first in
  the subset glyph list; printpdf passed only the used glyphs, so the first *real* glyph
  was renumbered into slot 0. Subsetting "Roboto" produced `R→0, b→1, o→2, t→3`, and the
  `R` was drawn as `.notdef`.
- **CFF/OpenType fonts were mislabelled.** An `OTTO` font was written as `CIDFontType2` +
  `/FontFile2`, both of which mean "TrueType `glyf` outlines". The descendant subtype is
  now taken from the sfnt magic of the program actually being embedded, and a full `OTTO`
  sfnt is written to `/FontFile3` as `/Subtype /OpenType`.
- **Built-in fonts emitted UTF-8 into a `WinAnsiEncoding` stream** (#273). "Grüße aus Köln"
  extracted as "GrÃ¼ÃŸe aus KÃ¶ln" — the text was not copy-able or searchable. printpdf now
  owns the WinAnsi encoding table.
- **The `'` and `"` operators ignored the selected font**, emitting raw UTF-8 — wrong for a
  WinAnsi built-in font *and* for an Identity-H external font, where the bytes must be
  glyph ids. The used-glyph collector also ignored them, so a page that drew text only via
  `'`/`"` registered no glyphs, its font was skipped as unused, and the text disappeared
  from the PDF entirely.
- A font that cannot be embedded now raises a `PdfWarnMsg::error` and omits `/FontFile`
  rather than writing a zero-length one. A missing font is a legal font that readers
  substitute for; an empty font program is a corrupt one they reject.

### Changed

- `printpdf::ParsedFont` is now a printpdf type (a newtype over `azul_layout::ParsedFont`,
  with `Deref`, so the existing API is unchanged). It guarantees the source bytes are
  retained. `ParsedFont::from_bytes`, field access and method calls all work as before;
  `into_inner()` / `as_azul()` reach the underlying azul face, and
  `printpdf::font::AzulParsedFont` re-exports it.
- `text_layout` now pulls in `rust-fontconfig`, which is needed to name the type that
  carries the retained bytes.

### Added

- `tests/font_embedding.rs` — opens the produced PDF with `lopdf` (deliberately *not*
  printpdf's own parser, where a symmetric writer/reader bug would cancel itself out) and
  asserts on the bytes a real reader sees: the font program is a parseable sfnt, every
  content-stream glyph id exists in it, and `/W` and `/ToUnicode` are keyed by those same
  ids and round-trip the exact source text.
- `tests/external_tools.rs` — verifies against poppler, which shares no code with printpdf:
  `pdffonts` must accept the embedded program, and `pdftotext` must round-trip the source
  text exactly (this is literally what copy/paste and search do).
- CI job `published-deps` — rebuilds the *published* dependency graph by stripping
  `[patch.crates-io]`, then runs the font tests against it. `cargo publish` strips that
  section, so without this the local checkout and the published crate are different
  programs. This is the gate that 0.10.0 needed and did not have.

## `0.5.2`

- enable all features on docs.rs

## `0.5.0`

- added `Svg` class to directly add SVG files to the PDF and instantiate them on the page
- remove `embedded_images` feature from default features
- change default PDF conformance to not embed an entire ICC color profile in the PDF (save on file size)

## `0.4.1`

- added `PdfDocument::save_to_bytes()` to save the PDF document directly to a `Vec<u8>` (see #101)

## `0.4.0`

- no actual changes, just a re-release of 0.3.4 to fix semver breakage

## `0.3.4`

- Added bookmarks and clipping path support
- *Breaking*: PDFConformance default changed to not require XMP Metadata and embedded ICC profile by default

## `0.3.1`

- Fix issue with Fonts on iOS and macOS
- Updated dependencies

## `0.3.0`

- Upgrade `rusttype` to `0.8.2` (breaks semver for non-`edition = "2019"` compilers, hence the new version)
- Upgrade `time` to `0.2.1`
- Added `PdfDocument::empty`

## `0.2.12`

- Upgrade `image` to `0.22`

## `0.2.11`

- Update `lopdf`, fixes #27

## `0.2.10`

- Upgraded image to `0.20`
- Added `ColorType::Palette` for indexed colors
- Creating an image from a Dynamic image can't fail, so no Result is returned

## `0.2.9`

- Upgraded `lopdf` to 0.17, getting rid of large `chrono` dependency
- Removed unnecessary `rand` dependency
- Made `image` dependency optional
- Added function to create images from an `image::DynamicImage`
- **WARNING**: Image crate has now certain lesser-used image types disabled by default:
  - .ico (ICO format)
  - .tga (Targa Image File)
  - .hdr (High Dynamic Range Image)
  - .dxt (S3 Texture Compression)
  - .webp (WEBP format)
  **If you don't re-enable these features, image decoding might fail at runtime!**
  The reason they were removed was because of compile-time performance. For extra speed
  when JPEG decoding, please also turn on `jpeg_rayon`
- No other API removals or large API changes
- Notable: `cargo build --no-default-features` has now "only" 33 dependencies and
  `printpdf` has a debug build time of roughly 20 seconds

## `0.2.8`

- Firefox PDF viewer now works correctly due to a bugfix regarding the embedded TTF font type
- No API changes

## `0.2.7`

- Fixed a bug (https://github.com/fschutt/printpdf/issues/20#issuecomment-409988462)
  regarding incorrect generation of character map files for embedding fonts
- No API changes

## `0.2.6`

- Updated `image` dependency to 0.19.0
- Updated `rand` dependency to 0.5.0
- Removed `error-chain`-generated errors in favor of simpler error enums (slight code-breaking change)
- Removed `FontError`, since it wasn't used anywhere
- Publicly re-exported `rusttype::Error` because that prevented error handling in applications that use `printpdf`

## `0.2.5`

- Fixed important word-spacing bug. In any version from 0.2.3 to this release there was a bug
  where the spacing between words wasn't adjusted correctly, because the horizontal advance width
  wasn't been taken into account. This has been fixed
- `Pt` and `Mm` can now be multiplied and divided by `f64`, mostly to ease the use of using them with
  projections
- New `utils::calculate_points_for_rect` and `utils::calculate_points_for_circle` functions make
  it easier to create circles and squares in a PDF. They are only convenience functions, mostly
  because PDF has no built-in notation for circles or squares.

## `0.2.4`

- Nothing changed, just a dependency update, because `rusttype` was yanked, so `printpdf 0.2.3`
  doesn't build anymore

## `0.2.3`

- printpdf now uses rusttype and does not require freetype anymore! There was an ugly
  character-spacing hack that was fixed. You should now be able to build printpdf on windows
  without further setup.
- Millimeters and points are now strongly typed - instead of `f64`, you now must denote the
  scale with `Pt(f64)`, `Mm(f64)` or `Px(f64)`. The `mm_to_pt!` and `pt_to_mm!` macros have
  been dropped since you can now do true conversions between these types. The reason for this
  change was because this raw `f64`-based conversion bit me hard while using the library.
- The `Line` now has a different API and no `new()` function anymore. This is because
  `Line::new(true, false, true)` is less expressive than `Line { has_stroke: true, ... }`.

## `0.2.2`

- SVG functionality was removed (commented out), because it didn't work in the first place
  and only increased build times. So there's no point in keeping functionality that nobody
  ever used, because it didn't work.
- Removed dependency on `num`
- `PdfDocument::save()` now only has a `T: Write` bound instead of `T: Write + Seek`.

## `0.2.1`

- The `document.save()` method now needs a `BufWriter`, to enforce buffered output (breaking change).
- The `PdfDocument` now implements `Clone`, so you can write one document to multiple outputs.
- You can disable the automatic embedding of an ICC profile by using a `CustomPdfConformance`.
  See `examples/no_icc.rs` for usage information.
- `set_outline_thickness` now accepts floating-point units
