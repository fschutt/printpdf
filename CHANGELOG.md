# Changelog

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
