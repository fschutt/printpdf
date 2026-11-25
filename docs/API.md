# printpdf.js WASM API

The WebAssembly build of printpdf (`src/wasm/`) exports seven functions. They
work on JSON strings:

| Function | Does |
| --- | --- |
| `Pdf_HtmlToDocument` | HTML to a `PdfDocument`. |
| `Pdf_BytesToDocument` | PDF bytes to a `PdfDocument`. |
| `Pdf_DocumentToBytes` | A `PdfDocument` to PDF bytes. |
| `Pdf_ResourcesForPage` | The fonts, images and layers one page uses. |
| `Pdf_PageToSvg` | One page to an SVG string. |
| `Pdf_RegisterFonts` | Keeps fonts for every later `Pdf_HtmlToDocument` call. |
| `Pdf_DecodeImage` | An image file to the `RawImage` the document model uses. |

Each function takes one JSON string and returns one JSON string:

```ts
{
  status: number,   // 0 ok, 1 the input is not valid for this function, 2 the operation failed,
                    // 3 the result could not be serialized
  data: T | string  // the result, or an error message when status is not 0
}
```

Every function also exists with a `Sync` suffix (`Pdf_HtmlToDocumentSync`), which
returns the string directly. The versions without the suffix are `async` and are
only built with the `js-sys` feature.

## JSON conventions

- Enums are `kebab-case` strings: `BuiltinFont::TimesRoman` is `"times-roman"`.
- Enums with data are `{ "type": "<variant>", "data": ... }`.
- Field names are `camelCase` for the document model (`PdfDocument`, `PdfPage`) and for
  the request and response objects (`returnByteArray`, `maxImageSize`).
- Two option structs keep `snake_case`, and a wrongly spelled name is silently
  ignored: `GeneratePdfOptions` (`page_width`) and `PdfSaveOptions` (`subset_fonts`).
- Binary data (images, fonts, PDF bytes) is a base64 string. A `data:...;base64,`
  prefix is accepted on input. A number array is accepted as well.

The structure of a `PdfDocument` is described in [STRUCTS.md](STRUCTS.md).

## A complete page

This is all it takes to make a PDF in the browser: one HTML file, no build. It imports the
package of the latest release from the demo site, which allows cross-origin loading, so it also
works when opened from disk. The file is `web/minimal.html` in the repository, and it is served at
<https://fschutt.github.io/printpdf/minimal.html>.

To pin a version or to work offline, build your own package with `wasm-pack build --target web`
and import `./pkg/printpdf.js` instead. The wasm file is about 14 MB, so the first load takes a moment.

```html
<!doctype html>
<meta charset="utf-8">
<title>printpdf: HTML to PDF in the browser</title>

<textarea id="html" rows="8" cols="60">
<h1 style="color: #b7410e">Hello PDF</h1>
<p>This page was laid out and written by printpdf, compiled to WebAssembly.</p>
</textarea>
<p><button id="make" disabled>Make PDF</button> <span id="status">loading...</span></p>

<script type="module">
  // The package of the latest release, served by the demo site (it allows cross-origin loading),
  // so this file works on its own, even opened from disk. To pin a version or work offline, build
  // your own with `wasm-pack build --target web` and import './pkg/printpdf.js' instead.
  import init, { Pdf_HtmlToDocument, Pdf_DocumentToBytes } from 'https://fschutt.github.io/printpdf/pkg/printpdf.js';

  // Every function takes one JSON string and returns one: { status, data }.
  // status 0 is success and `data` is the result, anything else is an error and `data` is the message.
  async function call(fn, input) {
    const result = JSON.parse(await fn(JSON.stringify(input)));
    if (result.status !== 0) throw new Error(result.data);
    return result.data;
  }

  const status = document.getElementById('status');
  const button = document.getElementById('make');
  await init(); // loads and compiles printpdf_bg.wasm
  button.disabled = false;
  status.textContent = 'ready';

  button.onclick = async () => {
    try {
      // 1. HTML to a document. The result is the PdfDocument as a JSON object (and warnings).
      const { doc } = await call(Pdf_HtmlToDocument, { html: document.getElementById('html').value });

      // 2. Document to PDF bytes, returned as base64.
      const { bytes } = await call(Pdf_DocumentToBytes, { doc });

      // 3. Download it.
      const pdf = Uint8Array.from(atob(bytes), (c) => c.charCodeAt(0));
      const url = URL.createObjectURL(new Blob([pdf], { type: 'application/pdf' }));
      Object.assign(document.createElement('a'), { href: url, download: 'hello.pdf' }).click();
      status.textContent = `done, ${pdf.length} bytes`;
    } catch (e) {
      status.textContent = 'error: ' + e.message;
    }
  };
</script>
```

Two calls are enough: `Pdf_HtmlToDocument` turns HTML into a document, `Pdf_DocumentToBytes` turns
the document into a PDF. The other functions below are for reading existing PDFs, rendering
pages to SVG, and adding fonts and images.

## Initialization

`printpdf_bg.wasm` has to be next to `printpdf.js`. Initialize the module once:

```js
import init, {
  Pdf_HtmlToDocument,
  Pdf_BytesToDocument,
  Pdf_DocumentToBytes,
  Pdf_ResourcesForPage,
  Pdf_PageToSvg,
  Pdf_RegisterFonts,
  Pdf_DecodeImage,
} from './pkg/printpdf.js';

await init();
```

## Pdf_HtmlToDocument

```ts
interface HtmlToDocumentInput {
  html: string;                     // required
  images?: Record<string, string>;  // name -> base64, used by <img src="name">
  fonts?: Record<string, string>;   // name -> base64 (ttf, otf, woff, woff2), used by font-family
  options?: GeneratePdfOptions;
}

interface GeneratePdfOptions {      // snake_case
  page_width?: number;              // mm, default 210
  page_height?: number;             // mm, default 297
  margin_top?: number;              // mm, default 0 (same for margin_right, margin_bottom, margin_left)
}

interface ImageOptimizationOptions { // camelCase
  quality?: number;                 // 0 to 1, default 0.85
  maxImageSize?: string;            // for example "300kb", default "2MB"
  ditherGreyscale?: boolean;
  convertToGreyscale?: boolean;
  autoOptimize?: boolean;           // default true: drop unused alpha, detect greyscale
  format?: "auto" | "jpeg" | "jpeg2000" | "flate" | "lzw" | "runlength" | "none";
}
```

The result is `{ doc: PdfDocument, warnings: PdfWarnMsg[] }`. See
[HTML.md](HTML.md) for the HTML and CSS that is supported.

```js
const result = JSON.parse(await Pdf_HtmlToDocument(JSON.stringify({
  html: "<html><body><h1>Hello World!</h1><img src='dog.png' style='width:100px;height:80px'></body></html>",
  images: { "dog.png": "data:image/png;base64,iVBORw0KGgo..." },
  options: { page_width: 210, page_height: 297 },
})));

if (result.status === 0) {
  console.log(result.data.doc, result.data.warnings);
} else {
  console.error(result.data);
}
```

## Pdf_BytesToDocument

```ts
interface BytesToDocumentInput {
  bytes: string;                  // base64 PDF, with or without the data: prefix
  options?: { failOnError?: boolean };
}
```

The result is `{ doc: PdfDocument, warnings: PdfWarnMsg[] }`. A warning is
`{ page, opId, severity: "error" | "warning" | "info", msg }`.

`failOnError` is accepted but has no effect yet: parsing always skips what it cannot read.

```js
const result = JSON.parse(await Pdf_BytesToDocument(JSON.stringify({ bytes: pdfBase64 })));
if (result.status === 0) {
  const { doc, warnings } = result.data;
}
```

## Pdf_DocumentToBytes

```ts
interface DocumentToBytesInput {
  doc: PdfDocument;
  options?: PdfSaveOptions;       // snake_case
  returnByteArray?: boolean;      // default false: bytes come back as base64
}

interface PdfSaveOptions {
  optimize?: boolean;             // default true: compress streams, prune unused objects
  subset_fonts?: boolean;         // default true
  secure?: boolean;               // default true: skip unknown operations
  image_optimization?: ImageOptimizationOptions;
}
```

The result is `{ bytes: string | number[], warnings: PdfWarnMsg[] }`.

```js
const result = JSON.parse(await Pdf_DocumentToBytes(JSON.stringify({
  doc,
  options: { optimize: true, subset_fonts: true },
})));

if (result.status === 0) {
  const bytes = Uint8Array.from(atob(result.data.bytes), (c) => c.charCodeAt(0));
  const url = URL.createObjectURL(new Blob([bytes], { type: "application/pdf" }));
}
```

## Pdf_ResourcesForPage

Lists what one page uses, so a client can send only those resources to
`Pdf_PageToSvg` instead of every font and image of the document.

```ts
interface ResourcesForPageInput {
  page: PdfPage;                  // an element of doc.pages
}
```

The result is `{ xobjects: string[], fonts: string[], layers: string[] }`, the ids
into `doc.resources`.

## Pdf_PageToSvg

```ts
interface PageToSvgInput {
  page: PdfPage;
  resources?: PdfResources;       // the resources the page uses, or the whole document's
  options?: {
    // Image formats for the <image> tags, in order of preference
    imageFormats?: ("png" | "jpeg" | "webp" | "avif" | "gif" | "bmp" | "tiff" | "tga" | "pnm")[];
  };
}
```

The result is `{ svg: string, warnings: PdfWarnMsg[] }`.

```js
const page = doc.pages[0];
const used = JSON.parse(await Pdf_ResourcesForPage(JSON.stringify({ page }))).data;

const result = JSON.parse(await Pdf_PageToSvg(JSON.stringify({
  page,
  resources: doc.resources,
  options: { imageFormats: ["png", "jpeg"] },
})));
if (result.status === 0) {
  container.innerHTML = result.data.svg;
}
```

## Pdf_RegisterFonts

Keeps fonts in the module, so they need not be sent with every `Pdf_HtmlToDocument`
call. Fonts passed to a call win over registered ones of the same name.

```ts
interface RegisterFontsInput {
  fonts: Record<string, string>;  // name -> base64
  replace?: boolean;              // clear the registered fonts first, default false
}
```

The result is `{ registered: number, total: number }`.

```js
JSON.parse(await Pdf_RegisterFonts(JSON.stringify({ fonts: { "Helvetica.ttf": helveticaBase64 } })));
```

## Pdf_DecodeImage

Decodes a PNG, JPEG or another image file that an enabled image feature supports
into the `RawImage` of the document model. Put the result into
`doc.resources.xobjects` and draw it with a `use-xobject` operation.

```ts
interface DecodeImageInput {
  bytes: string;                  // base64
}
```

The result is `{ image: RawImage, warnings: PdfWarnMsg[] }`. Without an image
format feature the call fails with status 2.
