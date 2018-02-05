# Changelog

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
