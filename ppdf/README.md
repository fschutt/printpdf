# ppdf

A command-line PDF tool built on [printpdf](https://github.com/fschutt/printpdf).
It converts a PDF to a JSON document model and back, so a PDF can be edited with
`jq` or any script, and it extracts text, text boxes, SVG pages, fonts and
images. It also renders HTML to PDF.

```sh
cargo install ppdf
```

Every command reads a file or stdin (`-` or no argument) and writes a file
(`-o`) or stdout, so commands combine in pipes.

## Edit a PDF with jq

```sh
# set every font size to 12 pt
ppdf json in.pdf | jq '(.pages[].ops[] | select(.type=="set-font").data.size) |= 12' | ppdf pdf - -o out.pdf

# drop page 3 and swap pages 1 and 2
ppdf json in.pdf | jq '.pages |= [.[1], .[0]] + .[3:]' | ppdf pdf - -o out.pdf
```

Fonts and image pixels are large base64 strings. `--strip-binaries DIR` moves
them out of the JSON and leaves `{"$file": "<name>.b64"}` in their place, and
`ppdf pdf --binaries DIR` puts them back:

```sh
ppdf json in.pdf --strip-binaries parts | jq '.pages[0]' > page1.json
ppdf json in.pdf --strip-binaries parts | jq '...' | ppdf pdf - --binaries parts -o out.pdf
```

The top-level `"$schema": "printpdf-json/1"` names the format. The structure of
the document is described in [STRUCTS.md](https://github.com/fschutt/printpdf/blob/master/docs/STRUCTS.md). Documents
without a `$schema` (for example from the wasm API) are accepted as well.

## Commands

| Command | Output |
| --- | --- |
| `ppdf json [IN] [-o OUT] [--pages 1-3,7] [--strip-binaries DIR] [--pretty]` | The document as JSON. |
| `ppdf pdf [IN] [-o OUT] [--binaries DIR] [--no-subset-fonts] [--no-optimize] [--optimize-images]` | A PDF from that JSON. |
| `ppdf text [IN] [--pages ..]` | Plain text, pages separated by a form feed. |
| `ppdf boxes [IN] [--pages ..] [--glyphs] [--pretty]` | Text lines and words with positions in pt, top-left origin. |
| `ppdf svg [IN] [--pages ..] [-o DIR]` | One SVG per page (`DIR/page-N.svg`), or one page on stdout. |
| `ppdf info [IN]` | Page sizes, fonts, image count, metadata and warning counts as JSON. |
| `ppdf fonts [IN] [-o DIR]` | The fonts as JSON. With `-o`, writes the font programs too. |
| `ppdf images [IN] [-o DIR]` | The images as JSON. With `-o`, writes them as PNG or JPEG files. |
| `ppdf html [IN] [-o OUT] [--css FILE] [--width MM] [--height MM] [--margin MM] [--images DIR] [--fonts DIR]` | A PDF from HTML. |
| `ppdf check [IN] [--profile general\|pdfa-1b\|pdfa-2b\|pdfx-1a\|pdfx-3] [--json]` | Preflight report: problems with a severity, a rule id, a page and an object. |

`ppdf pdf` writes images as they are, so a round trip does not change a pixel. The
file can get larger than the original; `--optimize-images` re-encodes them (lossy) to
make it smaller.

Pages are numbered from 1. `--pages` takes `3`, `1-3,7`, `5-` or `all`, and
keeps the order you write, so `--pages 3,1` reorders.

## Warnings and exit codes

printpdf reports what it could not read or had to skip as warnings on stderr.
`--warnings json` prints one JSON object per line, `--warnings off` prints
nothing, and `-v` adds info-level messages to the text output. `--strict` stops
at the first parse error instead of skipping what it cannot read.

| Exit code | Meaning |
| --- | --- |
| 0 | Success. |
| 1 | Error, nothing was written. |
| 2 | Success, but something in the input was skipped (an error-severity warning). |

`ppdf check` has its own exit codes, see [Check a PDF](#check-a-pdf).

## Check a PDF

```sh
ppdf check in.pdf                       # structure and resources, no standard
ppdf check in.pdf --profile pdfx-3      # also a subset of the PDF/X-3:2002 rules
ppdf check in.pdf --profile pdfa-1b --json | jq '.findings[] | select(.severity=="error")'
```

`check` has its own exit codes, because a file with errors is a normal result:

| Exit code | Meaning |
| --- | --- |
| 0 | No errors found. |
| 1 | At least one error was found. |
| 2 | The file cannot be read (it does not exist, or it is not a PDF that can be loaded). |

The profiles are `general`, `pdfa-1b`, `pdfa-2b`, `pdfx-1a` and `pdfx-3`. Text output
hides information messages; `-v` shows them, and the list of what is not checked.

This is a preflight of the rules that printpdf can decide from the file, not a
certification. No errors does not mean that the file conforms to PDF/A or PDF/X, and
it does not replace veraPDF or Acrobat Preflight. The rule ids and what is not
checked are in `printpdf::preflight` (`RULES`, `NOT_CHECKED`) and in the JSON output.

## Not covered

Encryption, linearization, signatures and repairing a damaged file at the
object level are better done with [qpdf](https://github.com/qpdf/qpdf).

See [docs/PPDF.md](https://github.com/fschutt/printpdf/blob/master/docs/PPDF.md) for the design and the plan for OCR.
