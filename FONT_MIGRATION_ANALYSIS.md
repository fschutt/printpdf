# PrintPDF ParsedFont Migration Analysis

## Overview

This document analyzes the current state of the ParsedFont migration in printpdf and identifies the root causes of two critical issues:
1. All spaces disappearing from generated PDFs
2. Text selection/copying results in undefined glyphs

## 1. Architecture Changes

### Azul Layout Changes

#### Font API Evolution
- **Before**: `ParsedFont::from_bytes(bytes, index, parse_outlines: bool)`
- **After**: `ParsedFont::from_bytes(bytes, index, warnings: &mut Vec<FontParseWarning>)`

#### New PDF-Specific Features
- **FontMetrics**: PDF-specific font metrics structure with HEAD, HHEA, and OS/2 table data
- **FontType**: Enum distinguishing TrueType vs OpenType CFF fonts
- **SubsetFont**: Result structure for font subsetting operations
- **Glyph Mapping**: New `index_to_cid: BTreeMap<u16, u16>` for CFF fonts
- **PrepFont Trait**: Interface for PDF font preparation with `lgi()` and `index_to_cid()` methods
- **Serde Support**: ParsedFont is now serializable/deserializable
- **Font Name Extraction**: New `font_name: Option<String>` from NAME table

#### Enhanced Text Shaping
- **ParsedFontTrait**: New trait implementation for text shaping
- **Glyph Size Calculation**: `get_glyph_size()` method for layout calculations
- **Hyphenation/Kashida Support**: Methods for advanced typography features

### PrintPDF Changes

#### Deferred Subsetting Architecture
- **Before**: Fonts were subset immediately upon registration (`PreparedFont`)
- **After**: Fonts are stored as originals and subset only during serialization (`RuntimeSubsetInfo`)

#### New Operations API
- **Before**: 
  - `SetFontSizeBuiltinFont { size, font }`
  - `WriteTextBuiltinFont { items, font }`
  - `SetFontSize { size, font }`
  - `WriteText { items, font }`
- **After**:
  - `SetFont { font: PdfFontHandle, size }`
  - `ShowText { items }`

#### Runtime Glyph Collection
- **New Function**: `collect_used_glyphs_from_pages()` analyzes all PDF operations to determine which glyphs are actually used
- **Font State Tracking**: Operations track current font across multiple text operations
- **Glyph Usage Analysis**: Builds `BTreeMap<FontId, BTreeMap<u16, char>>` of used glyphs per font

#### Serialization Changes
- **Font Dictionary**: Built using `RuntimeSubsetInfo` instead of `PreparedFont`
- **Operation Translation**: Uses subset glyph mappings during PDF generation
- **CMap Generation**: ToUnicode CMap created from actual glyph usage

## 2. Root Cause Analysis

### Problem 1: Missing Spaces

**Location**: `collect_used_glyphs_from_pages()` in `src/serialize.rs`

**Root Cause**:
```rust
TextItem::GlyphIds(glyphs) => {
    for (glyph_id, _offset) in glyphs {
        // PROBLEM: All glyph IDs mapped to replacement character
        font_glyphs.insert(*glyph_id, '\u{FFFD}'); // Replacement character
    }
}
```

**Analysis**:
1. Azul-layout generates `TextItem::GlyphIds` for shaped text, including spaces
2. The code incorrectly maps ALL glyph IDs to the Unicode replacement character `\u{FFFD}`
3. This corrupts the character mapping, making all glyphs appear as the same character
4. Spaces (and other characters) lose their identity in the ToUnicode CMap
5. PDF renderers cannot distinguish between different glyphs

**Impact Flow**:
```
Azul Layout → TextItem::GlyphIds(spaces) → collect_used_glyphs_from_pages() 
→ All glyphs mapped to '\u{FFFD}' → ToUnicode CMap corrupted 
→ PDF renderer cannot display spaces
```

### Problem 2: Undefined Glyphs in Text Selection

**Location**: `generate_cmap_string()` in `src/font.rs`

**Root Cause**:
```rust
pub fn generate_cmap_string(font: &ParsedFont, font_id: &FontId, glyph_ids: &[(u16, char)]) -> String {
    let mappings = glyph_ids
        .iter()
        .map(|&(gid, unicode)| (gid as u32, vec![unicode as u32]))
        .collect();

    let cmap = crate::cmap::ToUnicodeCMap { mappings };
    cmap.to_cmap_string(&font_id.0)
}
```

**Analysis**:
1. The ToUnicode CMap generation depends on the `glyph_ids` collected from `collect_used_glyphs_from_pages()`
2. Since all glyphs are incorrectly mapped to `\u{FFFD}`, the CMap only contains replacement characters
3. PDF viewers cannot resolve correct Unicode values for glyphs during text selection
4. Text copying fails because no valid Unicode mappings exist

**Impact Flow**:
```
Corrupted glyph collection → CMap with only '\u{FFFD}' mappings 
→ PDF viewer cannot map glyphs to Unicode → Text selection returns undefined glyphs
```

### Problem 3: Missing Reverse Lookup Capability

**Core Issue**: ParsedFont lacks the ability to map from glyph ID back to Unicode character

**Missing Functionality**:
```rust
// This function doesn't exist but is needed
impl ParsedFont {
    pub fn glyph_id_to_char(&self, glyph_id: u16) -> Option<char> {
        // Need to find which Unicode codepoint maps to this glyph ID
        // Current implementation has no efficient way to do this
    }
}
```

## 3. Technical Details

### Font State Tracking Flow

1. **Font Registration**: Fonts stored in `PdfFontMap` as `PdfFont { parsed_font, meta }`
2. **Operation Processing**: `Op::SetFont` sets current font context
3. **Text Rendering**: `Op::ShowText` uses current font for glyph collection
4. **Glyph Collection**: `collect_used_glyphs_from_pages()` builds usage map
5. **Subsetting**: Fonts subset based on actual glyph usage
6. **Serialization**: Subset fonts embedded in PDF with ToUnicode CMap

### Current Glyph Collection Logic

```rust
fn collect_used_glyphs_from_pages(
    pages: &[PdfPage],
    fonts: &BTreeMap<FontId, crate::font::PdfFont>,
) -> BTreeMap<FontId, BTreeMap<u16, char>> {
    let mut used_glyphs = BTreeMap::new();
    
    for page in pages {
        let mut current_font_id = None;
        
        for op in &page.ops {
            match op {
                Op::SetFont { font, .. } => {
                    // Track current font
                    current_font_id = match font {
                        PdfFontHandle::External(id) => Some(id.clone()),
                        PdfFontHandle::Builtin(_) => None,
                    };
                }
                Op::ShowText { items } => {
                    // Collect glyphs for current font
                    if let Some(ref font_id) = current_font_id {
                        // Process TextItem::Text and TextItem::GlyphIds
                    }
                }
            }
        }
    }
}
```

### CMap Generation Process

1. **Glyph Collection**: Build `BTreeMap<u16, char>` of used glyphs
2. **Subset Creation**: Generate subset font with new glyph IDs
3. **Mapping Calculation**: Create mapping from original to subset glyph IDs
4. **CMap Building**: Generate ToUnicode CMap for PDF embedding
5. **PDF Integration**: Embed CMap in font dictionary

## 4. Proposed Solution

### Step 1: Implement Reverse Glyph Lookup

Add efficient reverse lookup capability to ParsedFont:

```rust
impl ParsedFont {
    /// Map glyph ID back to Unicode character
    pub fn glyph_id_to_char(&self, glyph_id: u16) -> Option<char> {
        // Strategy 1: Build reverse lookup cache
        // Strategy 2: Search common Unicode ranges
        // Strategy 3: Use font's CMAP table directly
    }
    
    /// Get all characters that map to a glyph ID
    pub fn glyph_id_to_chars(&self, glyph_id: u16) -> Vec<char> {
        // Handle cases where multiple chars map to same glyph
    }
}
```

### Step 2: Fix Glyph Collection

Correct the `TextItem::GlyphIds` handling:

```rust
TextItem::GlyphIds(glyphs) => {
    for (glyph_id, _offset) in glyphs {
        // Use proper reverse lookup instead of placeholder
        let character = pdf_font.parsed_font.glyph_id_to_char(*glyph_id)
            .unwrap_or('\u{FFFD}');
        font_glyphs.insert(*glyph_id, character);
    }
}
```

### Step 3: Validate CMap Generation

Ensure ToUnicode CMap correctly reflects actual character mappings:

```rust
pub fn generate_cmap_string(font: &ParsedFont, font_id: &FontId, glyph_ids: &[(u16, char)]) -> String {
    // Validate that glyph_ids contains correct character mappings
    let validated_mappings = glyph_ids
        .iter()
        .filter_map(|&(gid, unicode)| {
            // Verify this is a valid mapping
            if unicode != '\u{FFFD}' {
                Some((gid as u32, vec![unicode as u32]))
            } else {
                // Log warning for invalid mapping
                None
            }
        })
        .collect();
    
    let cmap = crate::cmap::ToUnicodeCMap { mappings: validated_mappings };
    cmap.to_cmap_string(&font_id.0)
}
```

### Step 4: Enhanced Font State Tracking

Improve font context tracking across operations:

```rust
struct FontTracker {
    current_font_id: Option<FontId>,
    font_stack: Vec<FontId>, // For save/restore graphics state
}

impl FontTracker {
    fn set_font(&mut self, font_handle: &PdfFontHandle) {
        self.current_font_id = match font_handle {
            PdfFontHandle::External(id) => Some(id.clone()),
            PdfFontHandle::Builtin(_) => None,
        };
    }
    
    fn save_state(&mut self) {
        if let Some(id) = &self.current_font_id {
            self.font_stack.push(id.clone());
        }
    }
    
    fn restore_state(&mut self) {
        self.current_font_id = self.font_stack.pop();
    }
}
```

## 5. Testing Strategy

### Phase 1: Basic Text Testing
1. Simple text examples with spaces
2. Verify glyph collection accuracy
3. Validate CMap generation
4. Test text selection/copying

### Phase 2: Complex Layout Testing
1. HTML examples with mixed fonts
2. Tables with multiple text elements
3. Lists and formatted text
4. Multi-page documents

### Phase 3: Edge Case Testing
1. Fonts with ligatures
2. Non-Latin scripts
3. Special characters and symbols
4. Empty or whitespace-only text

## 6. Implementation Priority

1. **Critical (P0)**: Fix reverse glyph lookup and space handling
2. **High (P1)**: Validate CMap generation and text copying
3. **Medium (P2)**: Enhanced font state tracking
4. **Low (P3)**: Performance optimizations and edge cases

## 7. Validation Checklist

- [ ] Spaces appear correctly in generated PDFs
- [ ] Text selection produces correct Unicode characters
- [ ] CMap contains valid character mappings
- [ ] All example files compile and run
- [ ] Font subsetting works correctly
- [ ] No regression in existing functionality
- [ ] Performance impact is acceptable

## 8. Files to Modify

1. **azul/layout/src/font/parsed.rs**: Add reverse lookup methods
2. **printpdf/src/serialize.rs**: Fix `collect_used_glyphs_from_pages()`
3. **printpdf/src/font.rs**: Validate CMap generation
4. **printpdf/examples/*.rs**: Test with various examples

---

*This analysis was generated on November 19, 2025, following the ParsedFont migration in the printpdf library.*