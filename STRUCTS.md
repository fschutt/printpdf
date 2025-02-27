# printpdf.js Datastructures

## Document

```typescript
// Represents a parsed PDF Document, the main structure for the API.
interface PdfDocument {
    // Metadata about the document (author, info, XMP metadata, etc.)
    metadata: PdfMetadata;
    // Resources shared between pages, such as fonts, XObjects, images, forms, ICC profiles, etc.
    resources: PdfResources;
    // Document-level bookmarks (used for the outline)
    bookmarks: { [key: string]: PageAnnotation };
    // Page contents
    pages: PdfPage[];
}
```

```typescript
// Represents a single page in a PDF document.
interface PdfPage {
    // Media box of the page, defining the physical page size.
    mediaBox: Rect;
    // Trim box of the page, defining the intended finished size of the page.
    trimBox: Rect;
    // Crop box of the page, defining the region to which the contents of the page are clipped when displayed or printed.
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
    // Creation date of the document.
    creationDate: OffsetDateTime;
    // Modification date of the document.
    modificationDate: OffsetDateTime;
    // Creation date of the metadata.
    metadataDate: OffsetDateTime;
    // PDF conformance standard.
    conformance: PdfConformance;
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
// Initial struct for Xmp metatdata. This should be expanded later for XML handling, etc.
 * Right now it just fills out the necessary fields
interface XmpMetadata {
    // Web-viewable or "default" or to be left empty. Usually "default".
    renditionClass?: string | null;
}
```

```typescript
// Resources shared between pages in the PDF document.
interface PdfResources {
    // Fonts used in the PDF, mapped by FontId.
    fonts: { [key: string]: ParsedFont };
    // XObjects (forms, images, embedded PDF contents, etc.), mapped by XObjectId.
    xobjects: { [key: string]: XObject };
    // Map of explicit extended graphics states, mapped by ExtendedGraphicsStateId.
    extgstates: { [key: string]: ExtendedGraphicsState };
    // Map of optional content groups (layers), mapped by LayerInternalId.
    layers: { [key: string]: Layer };
}
```

## Page operations

```typescript
// Operations that can occur in a PDF page, defining the page content.
 * Tagged enum, see variants for possible operations.
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
    layerId: LayerInternalId;
}
```

```typescript
// Ends a layer
interface EndLayer {
    // Layer identifier
    layerId: LayerInternalId;
}
```

```typescript
// Loads a specific graphics state (necessary for describing extended graphics)
interface LoadGraphicsState {
    // Extended graphics state identifier
    gs: ExtendedGraphicsStateId;
}
```

```typescript
// Writes text, only valid between `StartTextSection` and `EndTextSection`
interface WriteText {
    // Text to write
    text: string;
    // Font size in points
    size: Pt;
    // Font identifier
    font: FontId;
}
```

```typescript
// Writes text using a builtin font.
interface WriteTextBuiltinFont {
    // Text to write
    text: string;
    // Font size in points
    size: Pt;
    // Builtin font to use
    font: BuiltinFont;
}
```

```typescript
// Add text to the file at the current position by specifying
// font codepoints for an ExternalFont
interface WriteCodepoints {
    // Font identifier
    font: FontId;
    // Font size in points
    size: Pt;
    // Array of codepoint-character tuples
    cp: Array<[number, string]>;
}
```

```typescript
// Add text to the file at the current position by specifying font
// codepoints with additional kerning offset
interface WriteCodepointsWithKerning {
    // Font identifier
    font: FontId;
    // Font size in points
    size: Pt;
    // Array of kerning-codepoint-character tuples
    cpk: Array<[number, number, string]>;
}
```

```typescript
// Sets the line height for the text
interface SetLineHeight {
    // Line height in points
    lh: Pt;
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
    // Font size in points
    size: Pt;
    // Font identifier
    font: FontId;
}
```

```typescript
// Positions the text cursor in the page from the bottom left corner
interface SetTextCursor {
    // Position of the text cursor
    pos: Point;
}
```

```typescript
// Sets the fill color for texts / polygons
interface SetFillColor {
    // Color to use for filling
    col: Color;
}
```

```typescript
// Sets the outline color for texts / polygons
interface SetOutlineColor {
    // Color to use for outlining
    col: Color;
}
```

```typescript
// Sets the outline thickness for texts / lines / polygons
interface SetOutlineThickness {
    // Outline thickness in points
    pt: Pt;
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
    // Line join style
    join: LineJoinStyle;
}
```

```typescript
// Line cap style: butt, round, or projecting-square
interface SetLineCapStyle {
    // Line cap style
    cap: LineCapStyle;
}
```

```typescript
// Set a miter limit in Pt
interface SetMiterLimit {
    // Miter limit in points
    limit: Pt;
}
```

```typescript
// Sets the text rendering mode (fill, stroke, fill-stroke, clip, fill-clip)
interface SetTextRenderingMode {
    // Text rendering mode
    mode: TextRenderingMode;
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
    // Transformation matrix
    matrix: CurTransMat;
}
```

```typescript
// Sets a matrix that only affects subsequent text objects.
interface SetTextMatrix {
    // Text matrix
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
    id: XObjectId;
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
    // Rendering intent
    intent: RenderingIntent;
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
    tag: String;
}
```

```typescript
// Begins a marked content sequence with an accompanying property list.
interface BeginMarkedContentWithProperties {
    // Tag for marked content
    tag: String;
    // Properties for marked content
    properties: DictItem[];
}
```

```typescript
// Defines a marked content point with properties.
interface DefineMarkedContentPoint {
    // Tag for marked content point
    tag: String;
    // Properties for marked content point
    properties: DictItem[];
}
```

```typescript
// Moves to the next line and shows text (the `'` operator).
interface MoveToNextLineShowText {
    // Text to show
    text: String;
}
```

```typescript
// Sets spacing, moves to the next line, and shows text (the `"` operator).
interface SetSpacingMoveAndShowText {
    // Word spacing value
    wordSpacing: f32;
    // Character spacing value
    charSpacing: f32;
    // Text to show
    text: String;
}
```

```typescript
// Unknown, custom key / value operation
interface Unknown {
    // Unknown operator key
    key: String;
    // Unknown operator value
    value: DictItem[];
}
```

## XObjects

```typescript
// External object that gets reference outside the PDF content stream.
// Tagged enum, see variants for possible XObject types.
export type XObject =
    | { type: "image"; data: RawImage }
    | { type: "form"; data: FormXObject }
    | { type: "external"; data: ExternalXObject };
```

```typescript
// Image XObject, for images
export interface RawImage {
    pixels: RawImageData;
    width: usize;
    height: usize;
    data_format: RawImageFormat;
    tag: Vec<u8>;
}
```

```typescript
// Raw image pixel data, tagged enum to differentiate data types
export type RawImageData =
    | { tag: "u8"; data: Uint8Array }
    | { tag: "u16"; data: Uint16Array }
    | { tag: "f32"; data: Float32Array };
```

```typescript
// Describes the format the image bytes are compressed with.
export enum RawImageFormat {
    // 8-bit grayscale image
    R8 = "r8",
    // 8-bit grayscale with alpha
    RG8 = "rg8",
    // 8-bit RGB color
    RGB8 = "rgb8",
    // 8-bit RGBA color
    RGBA8 = "rgba8",
    // 16-bit grayscale image
    R16 = "r16",
    // 16-bit grayscale with alpha
    RG16 = "rg16",
    // 16-bit RGB color
    RGB16 = "rgb16",
    // 16-bit RGBA color
    RGBA16 = "rgba16",
    // 8-bit BGR color (used in some image formats)
    BGR8 = "bgr8",
    // 8-bit BGRA color (used in some image formats)
    BGRA8 = "bgra8",
    // 32-bit floating point RGB color (HDR)
    RGBF32 = "rgbf32",
    // 32-bit floating point RGBA color (HDR)
    RGBAF32 = "rgbaf32",
}
```

```typescript
// __THIS IS NOT A PDF FORM!__ Form `XObject` for reusable content streams.
export interface FormXObject {
    // Form type (currently only Type1)
    formType: FormType;
    // Optional width / height, affects instantiation size
    size?: [Px, Px] | null;
    // The actual content of this FormXObject
    bytes: Uint8Array;
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
    lastModified?: OffsetDateTime | null;
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
// Form type, currently only Type1 is supported
export enum FormType {
    // Type 1 form XObjects are the most common and versatile type.
    Type1 = "type1",
}
```

```typescript
// External XObject, invoked by `/Do` graphics operator
export interface ExternalXObject {
    // External stream of graphics operations
    stream: ExternalStream;
    // Optional width
    width?: Px | null;
    // Optional height
    height?: Px | null;
    // Optional DPI of the object
    dpi?: number | null;
}
```

```typescript
// External Stream, allows embedding arbitrary content streams
export interface ExternalStream {
    // Stream description, for simplicity a simple map, corresponds to PDF dict
    dict: { [key: string]: DictItem };
    // Stream content
    content: Uint8Array;
    // Whether the stream can be compressed
    compress: boolean;
}
```

```typescript
// Simplified dict item for external streams
export type DictItem =
    | { type: "array"; data: DictItem[] }
    | { type: "string"; data: DictItemString }
    | { type: "bytes"; data: Uint8Array }
    | { type: "bool"; data: boolean }
    | { type: "float"; data: number }
    | { type: "int"; data: number }
    | { type: "real"; data: number }
    | { type: "name"; data: Uint8Array }
    | { type: "ref"; data: DictItemRef }
    | { type: "dict"; data: DictItemDict }
    | { type: "stream"; data: DictItemStream }
    | { type: "null" };
```

```typescript
export interface DictItemString {
    data: Uint8Array, 
    literal: boolean
}
```

```typescript
export interface DictItemRef {
    obj: number, 
    gen: number 
}
```

```typescript
export interface DictItemDict {
    map: { [key: string]: DictItem }
}
```

```typescript
export interface DictItemStream {
    stream: ExternalStream 
}
```

```typescript
// `/Type /Group`` (PDF reference section 4.9.2)
export interface GroupXObject {
    groupType?: GroupXObjectType | null;
}
```

```typescript
// Type of a `/Group` XObject. Currently only Transparency groups are supported
export enum GroupXObjectType {
    // Transparency group XObject (currently the only valid GroupXObject type)
    TransparencyGroup = "transparency-group",
}
```
