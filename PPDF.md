# `ppdf` — the printpdf command-line PDF tool (design)

Target: the next printpdf point release. A single static binary that makes the
crate's whole pipeline scriptable: **PDF ⇄ JSON ⇄ jq ⇄ PDF**, plus rendering,
text/box extraction, and (later) smart OCR.

## Positioning

| tool | model | strength | gap ppdf fills |
|---|---|---|---|
| `qpdf` | object-level QDF | lossless structural surgery | no *semantic* model — you edit raw dicts/streams |
| `mutool` | render + clean + extract | best-in-class rendering | JSON output is per-purpose, not round-trippable |
| `pdftk` | page-level | merge/split ergonomics | nothing below page granularity |
| `pdfcpu` | CLI + Go API | validation, permissions | same: no editable content model |

None of them can do `pdf → json | jq '.pages[0].ops |= …' | json → pdf`.
printpdf already has the round-trippable serde model (STRUCTS.md is its
TypeScript mirror) — the CLI is mostly plumbing plus flag design.

## Architecture

- New **binary crate `ppdf/`** in this repo (own `Cargo.toml`, path-dep on
  printpdf, **excluded from the printpdf package** — the 10 MiB crates.io cap
  is at 88%). Published separately: `cargo install ppdf`.
- `clap` derive, subcommand per verb. Everything reads stdin/writes stdout when
  no file is given, so it composes with `jq`/pipes.
- Exit codes: 0 ok, 1 error, 2 ok-with-error-severity-warnings (printpdf's
  `PdfWarnMsg` stream goes to stderr as JSON-lines with `--warnings=json`).
- Feature-gated heavy deps: `render` (resvg rasterization), `ocr`
  (tesseract-static). Default build = pure printpdf.

## Command tree (v1)

```
ppdf json   in.pdf  [-o out.json]  [--pages 1-3,7] [--strip-binaries DIR] [--pretty]
ppdf pdf    in.json [-o out.pdf]   [--subset-fonts] [--no-optimize] [--binaries DIR]
ppdf text   in.pdf  [--pages …]                       # plain text (extract_text)
ppdf boxes  in.pdf  [--pages …] [--glyphs]            # hOCR-style JSON (extract_text_boxes)
ppdf svg    in.pdf  [--pages …] [-o DIR|-]            # one SVG per page (to_svg)
ppdf info   in.pdf                                    # pages, boxes, fonts, encryption, warnings
ppdf fonts  in.pdf  [-o DIR]                          # list / extract embedded font programs
ppdf images in.pdf  [-o DIR]                          # list / extract images
ppdf html   in.html [-o out.pdf] [--css …] [--width mm] [--height mm]   # from_html
```

v1.x candidates, in rough priority order: `pages` (select/reorder/merge —
trivially: json + jq + pdf, but a native verb is friendlier), `render` (PNG via
resvg, needed for `ocr` anyway), `optimize`/`compress`, `validate`, `ocr`.
Non-goals (use qpdf): encryption, linearization, signatures, object-level
repair.

### The jq workflow is the product

```sh
# change every 24pt font to 12pt
ppdf json in.pdf | jq '(.pages[].ops[] | select(.type=="set-font").data.size) |= 12' | ppdf pdf - -o out.pdf

# drop page 3, swap 1 and 2
ppdf json in.pdf | jq '.pages |= [.[1], .[0]] + .[3:]' | ppdf pdf - -o out.pdf
```

Two things make or break this:

1. **Binary payload ergonomics.** Fonts/images serialize as base64 data-URIs —
   a 4 MB font turns `jq` interactive use into molasses. `--strip-binaries DIR`
   writes each payload to `DIR/<id>` and replaces it with
   `{"$file": "<id>"}`; `ppdf pdf --binaries DIR` re-inlines them. jq then
   operates on a small, readable document. (The wasm demo's tree editor already
   does exactly this elision in JS — `stripResources`; this moves the idea into
   the format.)
2. **Format stability.** The serde model is now a public wire format. Add
   `"$schema": "printpdf-json/1"` at the top level, version it, and keep the
   existing "old array format stays readable forever" discipline. STRUCTS.md
   becomes the normative doc for it.

## Smart OCR (`ppdf ocr`, feature `ocr`)

Goal: one command that produces *complete* text boxes for any PDF — born-digital
text deterministically, scanned/rasterized text via tesseract — and optionally
writes a searchable PDF back out.

```
ppdf ocr in.pdf [-o out.json]        # merged hOCR-style JSON (like `boxes`)
ppdf ocr in.pdf --searchable out.pdf # + invisible text layer (Tr 3) over images
        [--lang eng+deu] [--dpi 300] [--tessdata DIR]
```

Pipeline per page:

1. **Deterministic pass** — `extract_text_boxes` → word/glyph boxes in pt
   (top-left origin; conveniently already the raster's coordinate orientation).
2. **Rasterize** — `to_svg` → resvg → RGBA at `--dpi` (default 300;
   scale = dpi/72, so pt→px is one multiply).
3. **Mask** — paint every deterministic word bbox (plus ~1px bleed) with the
   page background before handing the image to tesseract. Sampling the median
   border color of each box beats plain white on tinted backgrounds; start with
   white, it covers 99%.
   *Why:* tesseract's page segmentation is the slow part and rendered vector
   text is exactly what it would waste time re-recognizing (imperfectly —
   producing duplicate, slightly-off text that then has to be deduplicated
   against the deterministic layer). Blanking solves speed **and** merge
   ambiguity in one move.
4. **OCR** — tesseract-static `get_hocr_text(page)` → `ParsedHocr` (has rect
   bounds). Traineddata: embedded eng/deu from the crate, `--tessdata` /
   `TESSDATA_PREFIX` for others; write to a temp dir like the crate's example.
5. **Merge** — map hOCR px → pt (divide by scale), then interleave OCR words
   into the deterministic line structure by baseline proximity; tag every word
   `"source": "text" | "ocr"` (and carry tesseract's `x_wconf` as `confidence`
   for OCR words; deterministic words get 100). Overlap conflicts should be
   rare *because of step 3* — when they happen, deterministic wins.
6. **Searchable PDF** (optional) — for each OCR word, emit an invisible-text
   run (`SetTextRenderingMode(Invisible)`, builtin Helvetica, size fitted to
   the box height, `Tz` fitted to the box width) at the word's position, then
   re-save. This is the standard OCRmyPDF-style text layer, built from ops we
   already have.

### Prerequisites already in place vs. still open

In place: `extract_text_boxes` (pt, top-left, per-word/per-glyph), `to_svg`,
the JSON model, CID/CMap-correct decoding, tesseract-static with hOCR parsing.

Open (tracked from the parser audit — these bound *fidelity*, not feasibility):
per-page resource scoping (multi-page `/F1` collisions), Form-XObject content
recursion (text inside forms is invisible to both extraction and masking),
page `/Rotate`, `/Widths`-as-authoritative advances, MediaBox-origin/CropBox
handling in `to_svg`. The first two matter most for OCR correctness: unmasked
form text would get double-recognized.

## Release plan

1. `ppdf/` crate with the v1 verbs above (json/pdf/text/boxes/svg/info/fonts/
   images/html) — pure printpdf, no new deps beyond clap. CI: build + a
   roundtrip smoke test (`json | jq . | pdf` on the corpus PDFs).
2. Parser-audit Tier-1 fixes land in printpdf (they benefit the library
   regardless of the CLI).
3. `render` + `ocr` features; tesseract-static gets a maintenance pass
   (it predates the current printpdf; check build on current toolchains).
4. azul-layout bump rides whichever printpdf release is next, independently.
