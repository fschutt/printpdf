# Op Enum Design Notes

## Current State (as of Nov 2025)

The `Op` enum currently does NOT map 1:1 to PDF operators. Several operations carry high-level information (like font references) that should be tracked by renderers/consumers, not embedded in the ops themselves.

## Known Issues

### Font Information in Text Operations

**Problem**: Operations like `WriteText`, `WriteTextBuiltinFont`, `WriteCodepoints`, etc. carry font information:
```rust
WriteTextBuiltinFont { items: Vec<TextItem>, font: BuiltinFont }
WriteText { items: Vec<TextItem>, font: FontId }
```

**PDF Reality**: The PDF "Tj" and "TJ" operators don't specify fonts. The font must be set separately using the "Tf" operator (e.g., `/F5 12 Tf`), and then text operators just reference the current font state.

**Current Workaround**: 
- Serialization: When `WriteTextBuiltinFont` is used without a preceding `SetFontSizeBuiltinFont`, it emits "Tj" without "Tf", relying on implicit font state
- Deserialization: When "Tj" is encountered without a font set via "Tf", it falls back to a default font (currently `TimesRoman` for PDF spec compliance)
- A warning is generated: `"'Tj' outside of text mode!"` or when font state is missing

### Duplicate Operations

**Problem**: We have both:
- `SetFontSize` / `SetFontSizeBuiltinFont`  
- `WriteText` / `WriteTextBuiltinFont`

These are duplicates that handle built-in vs. external fonts differently.

**PDF Reality**: The "Tf" operator doesn't care if a font is "built-in" or external - it just references a font resource name (e.g., "/F5" or "/MyCustomFont").

### Annotations and Layers in Op Stream

**Problem**: `LinkAnnotation` and layer operations (`BeginLayer`, `EndLayer`) are in the `Op` enum as if they're content stream operators.

**PDF Reality**: 
- Link annotations belong in the page's `/Annots` array, not the content stream
- Optional content (layers) is referenced via `/OC` in the content stream, but the layer definitions belong in the document catalog's `/OCProperties`

See:
- https://github.com/fschutt/printpdf/pull/217
- https://github.com/fschutt/printpdf/issues/236

## Future Direction

The `Op` enum should be refactored to map 1:1 with PDF operators:

### Text Operations (PDF spec section 9.3-9.4)

Current:
```rust
SetFontSize { size: Pt, font: FontId }
SetFontSizeBuiltinFont { size: Pt, font: BuiltinFont }
WriteText { items: Vec<TextItem>, font: FontId }
WriteTextBuiltinFont { items: Vec<TextItem>, font: BuiltinFont }
```

Should be:
```rust
// Tf operator: set both font and size
SetFont { font_resource: String, size: Pt }  // e.g., "F5", not the actual font data

// Tj/TJ operators: show text (font must be set first)
ShowText { items: Vec<TextItem> }  // No font reference!
```

The renderer/consumer tracks which font "F5" refers to by looking at the page's `/Resources/Font` dictionary.

### State Tracking

Renderers (SVG renderer, text extractor, etc.) would maintain:
```rust
struct RenderState {
    current_font: Option<String>,  // e.g., "F5"
    font_size: Pt,
    // ... other state
}
```

And look up actual font data when needed:
```rust
let font = resources.fonts.get(&state.current_font)?;
```

### Annotations

Move `LinkAnnotation` out of `Op` enum entirely:
```rust
pub struct PdfPage {
    pub ops: Vec<Op>,
    pub annotations: Vec<Annotation>,  // Separate from content stream
}
```

### Layers/Optional Content

Move layer tracking out of ops:
```rust
pub enum Op {
    // Reference a layer (PDF "/OC" operator)
    BeginOptionalContent { oc_ref: String },  // Just the reference, not the definition
    // ...
}

pub struct PdfDocument {
    pub layers: BTreeMap<LayerId, LayerDefinition>,  // In catalog
    // ...
}
```

## Testing Strategy

When refactoring:

1. **Parser tests**: Verify that real PDF operators parse correctly
2. **Roundtrip tests**: Ops should serialize to PDF and parse back identically
3. **Renderer tests**: SVG/text extraction should work with stateful tracking
4. **Warning tests**: Missing state (no font set before text) should warn but not crash

## Backwards Compatibility

The refactoring should be done incrementally:

1. Add new 1:1 ops alongside old ones (with deprecation warnings)
2. Update serializer/deserializer to handle both
3. Migrate examples and tests
4. Remove old ops in a major version bump

## Related Issues

- #217: Refactor outline/annotation handling
- #236: Layer (optional content) structure issues
