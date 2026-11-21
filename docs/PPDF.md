# ppdf design notes

`ppdf` is the command-line front end of printpdf. It lives in [`ppdf/`](../ppdf)
(usage is in [`ppdf/README.md`](../ppdf/README.md)). This file records why it is
shaped the way it is and what comes next.

## Why

printpdf has a serde model of a whole PDF (`PdfDocument`, documented in
[STRUCTS.md](STRUCTS.md)). That makes a round trip possible that other tools do
not offer:

```sh
ppdf json in.pdf | jq '.pages[0].ops |= ...' | ppdf pdf - -o out.pdf
```

| Tool | Model | What it is good at |
| --- | --- | --- |
| `qpdf` | Objects (QDF) | Lossless structural changes, repair, encryption. |
| `mutool` | Render, clean, extract | Rendering. |
| `pdftk` | Pages | Merging and splitting. |
| `pdfcpu` | CLI and Go API | Validation, permissions. |

None of them lets you edit the content of a page as data. ppdf does not try to
replace them for encryption, linearization, signatures or repairing a damaged
file at the object level; use qpdf for that.

## Layout

- `ppdf/` is a separate crate with its own `Cargo.toml` and a path dependency on
  printpdf. It is excluded from the printpdf package, so the crates.io size limit
  is not affected, and it is published as `ppdf`.
- `clap` provides the commands. Every command reads a file or stdin and writes a
  file or stdout.
- Warnings from printpdf go to stderr, as text or JSON lines (`--warnings`).
  Exit code 0 is success, 1 is an error and 2 is success with an error-severity
  warning.

## The JSON format

The JSON is a public format, so it is versioned. `ppdf json` writes
`"$schema": "printpdf-json/1"` as the first key and `ppdf pdf` refuses another
major version. Documents without a `$schema` are accepted. The old array form of
raw pixel data stays readable.

Fonts and image pixels are base64 strings of several megabytes. `--strip-binaries
DIR` moves every string over 512 characters that is base64 into `DIR/<hash>.b64`
and leaves `{"$file": "<hash>.b64"}`; `ppdf pdf --binaries DIR` restores them. The
file holds the string exactly as it was, so the round trip is lossless, and a
reference may only name a plain file inside `DIR`. `ppdf fonts -o` and
`ppdf images -o` write the decoded files.

## Status

Implemented: `json`, `pdf`, `text`, `boxes`, `svg`, `info`, `fonts`, `images`,
`html`.

Next, roughly in this order:

1. `pages`: select, reorder and merge pages without `jq`.
2. `render`: PNG output, needed for OCR.
3. `optimize`: compress and prune.
4. `validate`.
5. `ocr`.

## OCR (planned)

Goal: complete text boxes for any PDF. Text that is already in the file is read
directly. Text in scanned images goes through tesseract. A searchable PDF can be
written back.

```
ppdf ocr in.pdf [-o out.json]
ppdf ocr in.pdf --searchable out.pdf [--lang eng+deu] [--dpi 300] [--tessdata DIR]
```

Per page:

1. Read the text boxes with `extract_text_boxes`.
2. Rasterize the page: SVG from `to_svg`, then resvg, at `--dpi` (default 300).
3. Paint every box from step 1 with the page background, with about 1 px of
   bleed. Tesseract's page segmentation is the slow part, and text that is
   already known should not be recognized again, imperfectly. Blanking also
   avoids duplicates that would have to be merged afterwards. White works for
   most pages; sampling the border colour of each box handles tinted ones.
4. Run tesseract and parse its hOCR output.
5. Convert the OCR boxes from pixels to points and merge them into the line
   structure by baseline. Mark each word `"source": "text"` or `"ocr"`, with the
   confidence for OCR words. If boxes still overlap, the text from the file wins.
6. For `--searchable`, add an invisible text run (`Tr 3`, Helvetica, size and
   horizontal scaling fitted to the box) at each OCR word, like OCRmyPDF.

### What limits the quality

These are gaps in the PDF reader that decide how good the text boxes are, not
whether OCR is possible:

- Resources are not scoped per page, so `/F1` can collide between pages.
- Text inside Form XObjects is not read, so it would be recognized twice.
- Page `/Rotate` is ignored.
- `/Widths` is not used as the source of glyph advances.
- MediaBox origins and CropBox are not handled by `to_svg`.

The first two matter most for OCR. They come from an earlier audit of the PDF
reader and have not been fixed yet.
