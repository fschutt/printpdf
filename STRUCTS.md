# printpdf.js Datastructures

This document describes the main datastructures used in the printpdf.js API. 

Note that while the Rust code uses specific unit types like `Mm`, `Pt`, and `Px`, 
these are serialized to simple numbers in the JSON API. Similarly, complex types 
like `RawImage` and `ParsedFont` are serialized to base64-encoded data URL strings.

## Document

```typescript
// Represents a parsed PDF Document, the main structure for the API.
interface PdfDocument {
    // Metadata about the document (author, info, XMP metadata, etc.)
    metadata: PdfMetadata;
    // Resources shared between pages, such as fonts, XObjects, images, forms, ICC profiles, etc.
    resources: PdfResources;
    // Document-level bookmarks (used for the outline)
    bookmarks: { [uuid: string]: PageAnnotation };
    // Page contents
    pages: PdfPage[];
}
```

```typescript
// Represents a single page in a PDF document.
interface PdfPage {
    // Media box of the page, defining the physical page size in points (pt).
    mediaBox: Rect;
    // Trim box of the page, defining the intended finished size of the page in points (pt).
    trimBox: Rect;
    // Crop box of the page, defining the region to which the contents of the page are clipped in points (pt).
    cropBox: Rect;
    // List of operations to render this page.
    ops: Op[];
}
```

```typescript
// Metadata wrapper to keep XMP and document info in sync.
interface PdfMetadata {
    // Document information dictionary.
    info: PdfDocumentInfo;
    // XMP metadata (XML Metadata Platform).
    xmp: XmpMetadata | null;
}
```

```typescript
// Document information dictionary, contains standard PDF document properties.
interface PdfDocumentInfo {
    // Is the document trapped?
    trapped: boolean;
    // PDF document version, default: 1.
    version: number;
    // Creation date of the document (ISO format string in JSON).
    creationDate: string;
    // Modification date of the document (ISO format string in JSON).
    modificationDate: string;
    // Creation date of the metadata (ISO format string in JSON).
    metadataDate: string;
    // PDF conformance standard (kebab-case string in JSON).
    conformance: string;
    // PDF document title.
    documentTitle: string;
    // PDF document author.
    author: string;
    // The creator of the document.
    creator: string;
    // The producer of the document.
    producer: string;
    // Keywords associated with the document.
    keywords: string[];
    // The subject of the document.
    subject: string;
    // Identifier associated with the document.
    identifier: string;
}
```

```typescript
// Initial struct for Xmp metatdata.
interface XmpMetadata {
    // Web-viewable or "default" or to be left empty. Usually "default".
    renditionClass?: string | null;
}
```

```typescript
// Resources shared between pages in the PDF document.
interface PdfResources {
    // Fonts used in the PDF, mapped by FontId.
    // Note: Parsed fonts are serialized as base64 data URLs in the JSON.
    fonts: { [uuid: string]: string };
    // XObjects (forms, images, embedded PDF contents, etc.), mapped by XObjectId.
    // Note: Images are serialized as base64 data URLs in the JSON.
    xobjects: { [uuid: string]: XObject };
    // Map of explicit extended graphics states, mapped by ExtendedGraphicsStateId.
    extgstates: { [uuid: string]: ExtendedGraphicsState };
    // Map of optional content groups (layers), mapped by LayerInternalId.
    layers: { [uuid: string]: Layer };
}
```

## Page operations

```typescript
// Operations that can occur in a PDF page, defining the page content.
// Tagged enum, see variants for possible operations.
type Op =
    | { type: "marker"; data: Marker }
    | { type: "begin-layer"; data: BeginLayer }
    | { type: "end-layer"; data: EndLayer }
    | { type: "save-graphics-state" }
    | { type: "restore-graphics-state" }
    | { type: "load-graphics-state"; data: LoadGraphicsState }
    | { type: "start-text-section" }
    | { type: "end-text-section" }
    | { type: "write-text"; data: WriteText }
    | { type: "write-text-builtin-font"; data: WriteTextBuiltinFont }
    | { type: "write-codepoints"; data: WriteCodepoints }
    | { type: "write-codepoints-with-kerning"; data: WriteCodepointsWithKerning }
    | { type: "add-line-break" }
    | { type: "set-line-height"; data: SetLineHeight }
    | { type: "set-word-spacing"; data: SetWordSpacing }
    | { type: "set-font-size"; data: SetFontSize }
    | { type: "set-text-cursor"; data: SetTextCursor }
    | { type: "set-fill-color"; data: SetFillColor }
    | { type: "set-outline-color"; data: SetOutlineColor }
    | { type: "set-outline-thickness"; data: SetOutlineThickness }
    | { type: "set-line-dash-pattern"; data: SetLineDashPattern }
    | { type: "set-line-join-style"; data: SetLineJoinStyle }
    | { type: "set-line-cap-style"; data: SetLineCapStyle }
    | { type: "set-miter-limit"; data: SetMiterLimit }
    | { type: "set-text-rendering-mode"; data: SetTextRenderingMode }
    | { type: "set-character-spacing"; data: SetCharacterSpacing }
    | { type: "set-line-offset"; data: SetLineOffset }
    | { type: "draw-line"; data: DrawLine }
    | { type: "draw-polygon"; data: DrawPolygon }
    | { type: "set-transformation-matrix"; data: SetTransformationMatrix }
    | { type: "set-text-matrix"; data: SetTextMatrix }
    | { type: "link-annotation"; data: LinkAnnotationOp }
    | { type: "use-xobject"; data: UseXobject }
    | { type: "move-text-cursor-and-set-leading"; data: MoveTextCursorAndSetLeading }
    | { type: "set-rendering-intent"; data: SetRenderingIntent }
    | { type: "set-horizontal-scaling"; data: SetHorizontalScaling }
    | { type: "begin-inline-image" }
    | { type: "begin-inline-image-data" }
    | { type: "end-inline-image" }
    | { type: "begin-marked-content"; data: BeginMarkedContent }
    | { type: "begin-marked-content-with-properties"; data: BeginMarkedContentWithProperties }
    | { type: "define-marked-content-point"; data: DefineMarkedContentPoint }
    | { type: "end-marked-content" }
    | { type: "begin-compatibility-section" }
    | { type: "end-compatibility-section" }
    | { type: "move-to-next-line-show-text"; data: MoveToNextLineShowText }
    | { type: "set-spacing-move-and-show-text"; data: SetSpacingMoveAndShowText }
    | { type: "unknown"; data: Unknown };
```

```typescript
// Represents a text segment or a spacing adjustment within text operations.
// Untagged enum, can be either Text or Offset.
type TextItem =
    | string // Text segment (UTF-8 String)
    | number; // Spacing adjustment (in thousandths of an em)
```

```typescript
// Debugging or section marker
interface Marker {
    // Arbitrary id to mark a certain point in a stream of operations
    id: string;
}
```

```typescript
// Starts a layer
interface BeginLayer {
    // Layer identifier
    layerId: string; // LayerInternalId
}
```

```typescript
// Ends a layer
interface EndLayer {
    // Layer identifier
    layerId: string; // LayerInternalId
}
```

```typescript
// Loads a specific graphics state (necessary for describing extended graphics)
interface LoadGraphicsState {
    // Extended graphics state identifier
    gs: string; // ExtendedGraphicsStateId
}
```

```typescript
// Writes text, only valid between `StartTextSection` and `EndTextSection`
interface WriteText {
    // Array of text items to write
    items: TextItem[];
    // Font size in points (number in JSON)
    size: number;
    // Font identifier
    font: string; // FontId
}
```

```typescript
// Writes text using a builtin font.
interface WriteTextBuiltinFont {
    // Array of text items to write
    items: TextItem[];
    // Font size in points (number in JSON)
    size: number;
    // Builtin font to use (kebab-case string in JSON)
    font: string; // BuiltinFont
}
```

```typescript
// Add text to the file at the current position by specifying
// font codepoints for an ExternalFont
interface WriteCodepoints {
    // Font identifier
    font: string; // FontId
    // Font size in points (number in JSON)
    size: number;
    // Array of codepoint-character tuples
    cp: Array<[number, string]>;
}
```

```typescript
// Add text to the file at the current position by specifying font
// codepoints with additional kerning offset
interface WriteCodepointsWithKerning {
    // Font identifier
    font: string; // FontId
    // Font size in points (number in JSON)
    size: number;
    // Array of kerning-codepoint-character tuples
    cpk: Array<[number, number, string]>;
}
```

```typescript
// Sets the line height for the text
interface SetLineHeight {
    // Line height in points (number in JSON)
    lh: number;
}
```

```typescript
// Sets the word spacing in percent (default: 100.0)
interface SetWordSpacing {
    // Word spacing in percent
    percent: number;
}
```

```typescript
// Sets the font size for a given font, only valid between `StartTextSection` and `EndTextSection`
interface SetFontSize {
    // Font size in points (number in JSON)
    size: number;
    // Font identifier
    font: string; // FontId
}
```

```typescript
// Positions the text cursor in the page from the bottom left corner
interface SetTextCursor {
    // Position of the text cursor (point coordinates as numbers in JSON)
    pos: Point;
}
```

```typescript
// Sets the fill color for texts / polygons
interface SetFillColor {
    // Color to use for filling (see Color types below)
    col: Color;
}
```

```typescript
// Sets the outline color for texts / polygons
interface SetOutlineColor {
    // Color to use for outlining (see Color types below)
    col: Color;
}
```

```typescript
// Sets the outline thickness for texts / lines / polygons
interface SetOutlineThickness {
    // Outline thickness in points (number in JSON)
    pt: number;
}
```

```typescript
// Sets the outline dash pattern
interface SetLineDashPattern {
    // Line dash pattern
    dash: LineDashPattern;
}
```

```typescript
// Line join style: miter, round or limit
interface SetLineJoinStyle {
    // Line join style (kebab-case string in JSON)
    join: string;
}
```

```typescript
// Line cap style: butt, round, or projecting-square
interface SetLineCapStyle {
    // Line cap style (kebab-case string in JSON)
    cap: string;
}
```

```typescript
// Set a miter limit in Pt
interface SetMiterLimit {
    // Miter limit in points (number in JSON)
    limit: number;
}
```

```typescript
// Sets the text rendering mode (fill, stroke, fill-stroke, clip, fill-clip)
interface SetTextRenderingMode {
    // Text rendering mode (kebab-case string in JSON)
    mode: string;
}
```

```typescript
// Sets the character spacing (default: 1.0)
interface SetCharacterSpacing {
    // Character spacing multiplier
    multiplier: number;
}
```

```typescript
// `Ts`: Sets the line offset (default: 1.0)
interface SetLineOffset {
    // Line offset multiplier
    multiplier: number;
}
```

```typescript
// Draw a line (colors, dashes configured earlier)
interface DrawLine {
    // Line to draw
    line: Line;
}
```

```typescript
// Draw a polygon
interface DrawPolygon {
    // Polygon to draw
    polygon: Polygon;
}
```

```typescript
// Set the transformation matrix for this page. Make sure to save the old graphics state before invoking!
interface SetTransformationMatrix {
    // Transformation matrix (kebab-case type in JSON)
    matrix: CurTransMat;
}
```

```typescript
// Sets a matrix that only affects subsequent text objects.
interface SetTextMatrix {
    // Text matrix (kebab-case type in JSON)
    matrix: TextMatrix;
}
```

```typescript
// Adds a link annotation (use `PdfDocument::add_link` to register the `LinkAnnotation` on the document)
interface LinkAnnotationOp {
    // Link annotation details
    link: LinkAnnotation;
}
```

```typescript
// Instantiates an XObject with a given transform (if the XObject has a width / height).
interface UseXobject {
    // XObject identifier
    id: string; // XObjectId
    // Transformation to apply when using the XObject
    transform: XObjectTransform;
}
```

```typescript
// `TD` operation
interface MoveTextCursorAndSetLeading {
    tx: number;
    ty: number;
}
```

```typescript
// `ri` operation
interface SetRenderingIntent {
    // Rendering intent (kebab-case string in JSON)
    intent: string;
}
```

```typescript
// `Tz` operation
interface SetHorizontalScaling {
    // Horizontal scaling percentage
    percent: number;
}
```

```typescript
// Begins a marked content sequence.
interface BeginMarkedContent {
    // Tag for marked content
    tag: string;
}
```

```typescript
// Begins a marked content sequence with an accompanying property list.
interface BeginMarkedContentWithProperties {
    // Tag for marked content
    tag: string;
    // Properties for marked content
    properties: DictItem[];
}
```

```typescript
// Defines a marked content point with properties.
interface DefineMarkedContentPoint {
    // Tag for marked content point
    tag: string;
    // Properties for marked content point
    properties: DictItem[];
}
```

```typescript
// Moves to the next line and shows text (the `'` operator).
interface MoveToNextLineShowText {
    // Text to show
    text: string;
}
```

```typescript
// Sets spacing, moves to the next line, and shows text (the `"` operator).
interface SetSpacingMoveAndShowText {
    // Word spacing value
    wordSpacing: number;
    // Character spacing value
    charSpacing: number;
    // Text to show
    text: string;
}
```

```typescript
// Unknown, custom key / value operation
interface Unknown {
    // Unknown operator key
    key: string;
    // Unknown operator value
    value: DictItem[];
}
```

## XObjects

```typescript
// External object that gets reference outside the PDF content stream.
// Tagged enum, see variants for possible XObject types.
type XObject =
    | { type: "image"; data: string } // Base64-encoded image data
    | { type: "form"; data: FormXObject }
    | { type: "external"; data: ExternalXObject };
```

```typescript
// Note: not a PDF form! Form `XObject` are just reusable content streams.
interface FormXObject {
    // Form type (currently only Type1)
    formType: string; // FormType (kebab-case in JSON)
    // Optional width / height, affects instantiation size
    size?: [number, number] | null; // Width, height in pixels
    // The actual content of this FormXObject
    bytes: number[]; // Uint8Array in JavaScript
    // Optional matrix, maps form to user space
    matrix?: CurTransMat | null;
    // (Optional, PDF 1.2+) Resources required by this form XObject
    resources?: { [key: string]: DictItem } | null;
    // (Optional; PDF 1.4) Group attributes dictionary
    group?: GroupXObject | null;
    // (Optional; PDF 1.4) Reference dictionary for page import
    refDict?: { [key: string]: DictItem } | null;
    // (Optional; PDF 1.4) Metadata stream for the form XObject
    metadata?: { [key: string]: DictItem } | null;
    // (Optional; PDF 1.3) Page-piece dictionary associated with the form
    pieceInfo?: { [key: string]: DictItem } | null;
    // (Optional; PDF 1.3, required if PieceInfo is present) Last modification date
    lastModified?: string | null; // ISO date string in JSON
    // (Optional; PDF 1.3, required for structural content) StructParent integer key
    structParent?: number | null;
    // (Optional; PDF 1.3, required for marked-content sequences) StructParents integer key
    structParents?: number | null;
    // (Optional; PDF 1.2) OPI version dictionary
    opi?: { [key: string]: DictItem } | null;
    // (Optional; PDF 1.5) Optional content group or membership dictionary
    oc?: { [key: string]: DictItem } | null;
    // (Optional; PDF 1.0, obsolescent) Name in XObject subdictionary
    name?: string | null;
}
```

```typescript
// External XObject, invoked by `/Do` graphics operator
interface ExternalXObject {
    // External stream of graphics operations
    stream: ExternalStream;
    // Optional width in pixels
    width?: number | null;
    // Optional height in pixels
    height?: number | null;
    // Optional DPI of the object
    dpi?: number | null;
}
```

```typescript
// External Stream, allows embedding arbitrary content streams
interface ExternalStream {
    // Stream description, for simplicity a simple map, corresponds to PDF dict
    dict: { [key: string]: DictItem };
    // Stream content
    content: number[]; // Uint8Array in JavaScript
    // Whether the stream can be compressed
    compress: boolean;
}
```

```typescript
// Simplified dict item for external streams
type DictItem =
    | { type: "array"; data: DictItem[] }
    | { type: "string"; data: DictItemString }
    | { type: "bytes"; data: number[] } // Uint8Array in JavaScript
    | { type: "bool"; data: boolean }
    | { type: "float"; data: number }
    | { type: "int"; data: number }
    | { type: "real"; data: number }
    | { type: "name"; data: number[] } // Uint8Array in JavaScript
    | { type: "ref"; data: DictItemRef }
    | { type: "dict"; data: DictItemDict }
    | { type: "stream"; data: DictItemStream }
    | { type: "null" };
```

```typescript
interface DictItemString {
    data: number[], // Uint8Array in JavaScript
    literal: boolean
}
```

```typescript
interface DictItemRef {
    obj: number,
    gen: number
}
```

```typescript
interface DictItemDict {
    map: { [key: string]: DictItem }
}
```

```typescript
interface DictItemStream {
    stream: ExternalStream
}
```

```typescript
// `/Type /Group`` (PDF reference section 4.9.2)
interface GroupXObject {
    groupType?: string | null; // GroupXObjectType (kebab-case in JSON)
}
```

## Color Types

```typescript
// Color wrapper
type Color =
    | { type: "rgb"; data: Rgb }
    | { type: "cmyk"; data: Cmyk }
    | { type: "greyscale"; data: Greyscale }
    | { type: "spot-color"; data: SpotColor };

// RGB color
interface Rgb {
    r: number; // 0.0-1.0
    g: number; // 0.0-1.0
    b: number; // 0.0-1.0
    iccProfile?: string | null; // ICC profile ID
}

// CMYK color
interface Cmyk {
    c: number; // 0.0-1.0
    m: number; // 0.0-1.0
    y: number; // 0.0-1.0
    k: number; // 0.0-1.0
    iccProfile?: string | null; // ICC profile ID
}

// Greyscale color
interface Greyscale {
    percent: number; // 0.0-1.0
    iccProfile?: string | null; // ICC profile ID
}

// Spot color (named vendor colors)
interface SpotColor {
    c: number; // 0.0-1.0
    m: number; // 0.0-1.0
    y: number; // 0.0-1.0
    k: number; // 0.0-1.0
}
```

## Geometry Types

```typescript
// Rectangle
interface Rect {
    x: number; // Points in JSON
    y: number; // Points in JSON
    width: number; // Points in JSON
    height: number; // Points in JSON
}

// Point
interface Point {
    x: number; // Points in JSON
    y: number; // Points in JSON
}

// Line
interface Line {
    points: Point[];
}

// Polygon
interface Polygon {
    points: Point[];
    fill: boolean;
    close: boolean;
}
```

## Other Common Types

```typescript
// Line dash pattern
interface LineDashPattern {
    array: number[];
    phase: number;
}

// XObject transformation
interface XObjectTransform {
    translateX?: number | null; // Points in JSON
    translateY?: number | null; // Points in JSON
    rotate?: XObjectRotation | null;
    scaleX?: number | null;
    scaleY?: number | null;
    dpi?: number | null;
}

// XObject rotation
interface XObjectRotation {
    angleCcwDegrees: number;
    rotationCenterX: number; // Pixels in JSON
    rotationCenterY: number; // Pixels in JSON
}
```