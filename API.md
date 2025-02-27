# printpdf.js WASM API

This document describes the **JavaScript** API exposed by the `printpdf.js` WebAssembly module, 
which wraps the Rust code in `wasm.rs`. These functions allow you to:

1. Generate a PDF document from HTML (`Pdf_HtmlToDocument`)
2. Parse an existing PDF document from PDF bytes (`Pdf_BytesToDocument`)
3. Extract resource IDs (images, fonts, layers) from a PDF page (`Pdf_GetResourcesForPage`)
4. Convert a PDF page into an SVG string (`Pdf_PageToSvg`)
5. Save a PDF document back into PDF bytes (`Pdf_DocumentToBytes`)

All functions take a stringified JS object as both input and output.
The output, when converted back to a JS object, has a 

```ts
{
  status: number,   // 0 = okay, non-zero = error
  data: T | string  // actual data or error string
}
```

**Enum fields** in the underlying Rust code are **always renamed** to `kebab-case` in JSON. 

Tagged enums are tagged with `type` for the variant and `data` for the payload. 

> `BuiltinFont::TimesRoman` becomes `"times-roman"`.
> 
> `MyEnum::VariantType { data }` becomes `{ type: "variant-type", data: ... }`.

**Struct fields** in Rust are **always renamed** to `camelCase` when serialized to JSON. 
> `foo.page_height` in Rust becomes `foo.pageHeight` in JSON

## Initialization

Before calling any of the below functions, **make sure** you have initialized the WASM module. 

```js
// printpdf_bg.wasm needs to be in the same directory as printpdf.js

import init, {
  Pdf_HtmlToDocument,
  Pdf_BytesToDocument,
  Pdf_GetResourcesForPage,
  Pdf_PageToSvg,
  Pdf_PdfDocumentToBytes,
} from './pkg/printpdf.js';

async function main() {
  // Initialize the WASM
  await init();

  // Now we can safely call our PDF functions
  // ...
}

main().catch(console.error);
```

## Pdf_HtmlToDocument

Generates a **new PDF document** from given HTML, optional images, 
optional fonts, and page generation options.  

```ts
interface PdfHtmlToDocumentInput {
  html: string;                   // Required: the source HTML to convert

  title?: string;                 // Title of the PDF document
  images?: Record<string, string> // filename => base64-encoded image data
  fonts?: Record<string, string>  // filename => base64-encoded font data
  options?: {                     // PDF generation options
    imageCompression?: number;    // e.g. 0.75 for 75% image quality, null/undefined to disable
    fontEmbedding?: boolean;      // default true
    pageWidth?: number;           // in mm; default 210
    pageHeight?: number;          // in mm; default 297
  }
}
```

```json5
{
  "status": 0,
  "data": {
    "metadata": { /* ... */ },
    "resources": { /* ... */ },
    "bookmarks": { /* ... */ },
    "pages": [
      {
        "mediaBox": { /* ... */ },
        "trimBox": { /* ... */ },
        "cropBox": { /* ... */ },
        "ops": [ ... ]
      }
      // ...
    ]
  }
}
```

### Example: Generating PDF from HTML

```js
const inputObject = {
  title: "My PDF!",
  html: "<!doctype html><html><body><h1>Hello World!</h1></body></html>",
  // Suppose we have a base64 version of 'dog.png' we want to embed:
  images: {
    "dog.png": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA..."
  },
  fonts: {
    "f1.woff2": "data:font/woff2;base64,..." // supports: ttf, otf, woff, woff2
  },
  options: {
    pageWidth: 210, // mm
    pageHeight: 297 // mm
  }
};

const inputJson = JSON.stringify(inputObject);
const outputJson = Pdf_HtmlToDocument(inputJson);
const result = JSON.parse(outputJson);

if (result.status === 0) {
  // result.data is the PdfDocument object
  console.log("PDF document:", result.data); // { }
} else {
  console.error("Error generating PDF:", result.data);
}
```

## Pdf_BytesToDocument

Parses an **existing PDF** from a **Base64-encoded** byte string. 
Outputs a structured JSON representation of the PDF and any warnings that occurred.

```ts
interface PdfBytesToDocumentInput {
  pdfBase64: string;          // Base64-encoded PDF bytes
  options?: {
    failOnError?: boolean;    // default false; if true, parse errors become fatal
  }
}
```

```json5
{
  "status": 0,
  "data": {
    "pdf": { /* ... */ },       // The PdfDocument JSON
    "warnings": [ ... ]   // Array of any PDF parse warnings
  }
}
```

### Example: Parsing a PDF file

```js
const myPdfBase64 = "data:application/pdf;base64,JVBEAwIG9C9M...";
const parseInput = {
  pdfBase64: myPdfBase64,
  options: { failOnError: false }
};

const parseOutputJson = Pdf_BytesToDocument(JSON.stringify(parseInput));
const parseResult = JSON.parse(parseOutputJson);

if (parseResult.status === 0) {
  console.log("PDF parsed!");
  const pdfDoc = console.log(parseResult.data.pdf);
  console.log("Warnings:", parseResult.data.warnings);
} else {
  console.error("Failed to parse PDF:", parseResult.data);
}
```

## Pdf_GetResourcesForPage

Given a **single PDF page** (in JSON), returns the **xobject IDs**, **font IDs**, and 
**layer IDs** that page references. This is especially useful if you want to render 
or further process only the resources used by that page, because all fonts / images
are decoded in / to base64 during the JSON reading process from the Rust side. If this
function didn't exist, we'd have to re-decode every single font in the entire PDF for 
rendering every page, even if the font isn't used by the page.

```ts
interface PdfGetResourcesForPageInput {
  page: PdfPage;  // single element from `pdfDocument.pages[index]`
```

```json5
{
  "status": 0,
  "data": {
    "xobjects": [ "X001", "X012", "X10", ... ], 
    "fonts": [ "F1", "F3", "F4", ... ],
    "layers": [ "Page1-Layer0138", ... ]
  }
}
```

### Example: Extracting resource IDs used on a page

```js
const inputObj = {
  page: pdfDocument.pages[0]
};

const resourcesJson = Pdf_GetResourcesForPage(JSON.stringify(inputObj));
const resourcesResult = JSON.parse(resourcesJson);

if (resourcesResult.status === 0) {
  console.log("Resources for this page:", resourcesResult.data);
  // data.xobjects, data.fonts, data.layers
} else {
  console.error("Error getting resources:", resourcesResult.data);
}
```

## Pdf_PageToSvg

Converts a **PDF page** into **SVG** for rendering or preview. The function 
also needs the **document resources** that page relies on and optional conversion options.

```ts
interface PdfPageToSvgInput {
  page: PdfPage;            // The PDF page you want to render as SVG
  resources?: PdfResources; // The subset (or entire) PDF resources 
  options?: PdfToSvgOptions;
}

interface PdfToSvgOptions {
  // The image formats you prefer in the SVG `<image xlink:href="data:image/...;base64,">`
  // tags, in order of preference. Depends on what image features the library was compiled with.
  imageFormats?: ["png"|"jpeg"|"gif"|"webp"|"pnm"|"tiff"|"tga"|"bmp"|"avif"]
}
```

```json5
{
  "status": 0,
  "data": {
    "svg": "<svg ...> ... </svg>" // raw SVG string
  }
}
```

### Example: Rendering a PDF page as SVG

```js
// Determine which resources this page needs
const pageResourcesRequest = {
  page: pdfDocument.pages[0]
};

const resourcesJson = Pdf_GetResourcesForPage(JSON.stringify(pageResourcesRequest));
const resourcesResult = JSON.parse(resourcesJson);

let pageResources = pdfDocument.resources; 
if (resourcesResult.status === 0) {
  // If needed, you could copy only the required IDs from pdfDocument.resources
  // into a new object. For this example, we'll just use the full resources:
  pageResources = pdfDocument.resources;
}

const svgRequest = {
  page: page,
  resources: pageResources,
  options: {
    imageFormats: ["png", "jpeg"] 
  }
};

const svgOutputJson = Pdf_PageToSvg(JSON.stringify(svgRequest));
const svgResult = JSON.parse(svgOutputJson);

if (svgResult.status === 0) {
  // Insert the SVG string into the DOM. Don't forget to use display=block on the parent!
  document.getElementById("mySvgContainer").innerHTML = svgResult.data.svg;
} else {
  console.error("SVG conversion error:", svgResult.data);
}
```

## Pdf_PdfDocumentToBytes

Takes a **`PdfDocument`** (the JSON structure) plus optional save options, 
and **serializes** it into a **Base64**-encoded PDF.

```ts
interface PdfDocumentToBytesInput {
  pdf: PdfDocument;          // The PdfDocument object you want to export
  options: PdfSaveOptions {
    optimize?: boolean;      // default true, compress/prune unreferenced objects
    subsetFonts?: boolean;   // default true, subsets embedded fonts
    secure?: boolean;        // default true, skip unknown PDF ops if encountered
  }
}
```

```json5
{
  "status": 0,
  "data": {
    "pdfBase64": "<base64-encoded PDF bytes>" // use atob(data.pdfBase64)
  }
}
```

### Example: Saving a PdfDocument back into PDF bytes

```js

const inputObj = {
  pdf: pdfDocument,
  options: {
    optimize: true, 
    subsetFonts: true, 
    secure: true
  }
};

const inputJson = JSON.stringify(inputObj);
const outputJson = Pdf_PdfDocumentToBytes(inputJson);
const outputResult = JSON.parse(outputJson);

if (outputResult.status === 0) {
  // outputResult.data.pdfBase64 is the PDF in base64 form
  const base64Pdf = outputResult.data.pdfBase64;

  const pdfBytes = atob(base64Pdf); // decode base64
  const pdfBuffer = new Uint8Array(pdfBytes.length);

  for (let i = 0; i < pdfBytes.length; i++) {
    pdfBuffer[i] = pdfBytes.charCodeAt(i);
  }

  const blob = new Blob([pdfBuffer], { type: 'application/pdf' });
  const url = URL.createObjectURL(blob);

  // Trigger a download
  const link = document.createElement('a');
  link.href = url;
  link.download = "my_exported.pdf";
  link.click();
  URL.revokeObjectURL(url);
  
  console.log("PDF exported successfully!");
} else {
  console.error("PDF export error:", outputResult.data);
}
```

## Datastructures

Many advanced fields appear in the `pdfDocument` JSON (fonts, xobjects, layers, color definitions, etc.). 
For most basic use cases, you only need to manipulate the top-level `pages`, or embed images/fonts. If you need 
to dig deeper, the datastructures are documented in the [/STRUCTS.md](/STRUCTS.md) file.

Enjoy creating, parsing, and manipulating PDFs with `printpdf.js`!
