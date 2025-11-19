# PDF Text Positioning Analysis

## Current Problem

The characters are "wildly thrown everywhere" because the current `render_unified_layout()` function calls `SetTextCursor` (PDF `Td` operator) for every single glyph cluster, resulting in absolute positioning that's completely wrong.

### Example from generated PDF:
```
60.40627 820.4511 Td
[<0002>] TJ
183.51564 820.4511 Td  <- WRONG: Each character gets absolute positioning!
[<0004>] TJ
203.06253 820.4511 Td  <- WRONG: Should be relative to previous glyph!
[<0000>] TJ
```

## PDF Text Positioning Operators

### 1. Text Matrix vs Current Transformation Matrix

- **Text Matrix (Tm)**: Controls text positioning, scaling, rotation
- **Current Transformation Matrix (CTM)**: Controls graphics coordinate system
- These are SEPARATE coordinate systems in PDF

### 2. Text Positioning Operators

| Operator | Name | Function | Usage Model |
|----------|------|----------|-------------|
| `Td` | Move Text Position | **RELATIVE** offset from current position | Use for line breaks, paragraph spacing |
| `TD` | Move Text Position + Set Leading | Same as Td + sets line spacing | Use for line breaks with specific spacing |
| `Tm` | Set Text Matrix | **ABSOLUTE** positioning + transformation | Use for initial positioning, major repositioning |
| `T*` | Move to Next Line | Move down by current leading value | Use for simple line breaks |

### 3. Text Rendering Operators

| Operator | Name | Function | Positioning Behavior |
|----------|------|----------|---------------------|
| `Tj` | Show Text | Show text string | **Advances current position by glyph widths** |
| `TJ` | Show Text with Positioning | Show text with individual glyph positioning | **Allows precise glyph positioning within run** |
| `'` | Move to Next Line + Show Text | T* followed by Tj | Combines positioning + rendering |

## Key Insight: PDF Text Positioning is ADDITIVE

### Current Position Tracking
PDF maintains an **internal text cursor position** that:
1. Starts at (0,0) when `BT` (Begin Text) is called
2. **Automatically advances** after each glyph is rendered based on glyph advance width
3. Can be **offset** with `Td` (relative) or **set absolutely** with `Tm`

### The Problem with Current Code
```rust
// WRONG: This sets absolute position for each glyph
ops.push(Op::SetTextCursor {
    pos: Point::new(absolute_x, absolute_y)  // This is Td operator - RELATIVE!
});
ops.push(Op::ShowText { ... });
```

The `SetTextCursor` (Td) operator is **RELATIVE**, not absolute! So each call adds to the previous position.

## Correct PDF Text Rendering Models

### Model 1: Line-Based Positioning (Recommended)
```
BT                              % Begin text
/F1 12 Tf                       % Set font
100 700 Td                      % Move to start of first line (relative to origin)
[(Hello )] TJ                   % Render text, cursor auto-advances
20 0 Td                         % Small horizontal adjustment if needed
[(World)] TJ                    % Continue rendering
0 -14 Td                        % Move to next line (down 14 points)
[(Next line)] TJ                % Render next line
ET                              % End text
```

### Model 2: Absolute Matrix Positioning (For Complex Layouts)
```
BT
/F1 12 Tf
1 0 0 1 100 700 Tm               % Set text matrix (absolute position)
[(Hello World)] TJ               % Render text
1 0 0 1 100 686 Tm               % New absolute position for next line
[(Next line)] TJ
ET
```

### Model 3: TJ Array with Positioning (For Precise Glyph Control)
```
BT
/F1 12 Tf
100 700 Td                       % Initial position
[(H) -30 (e) 0 (l) 0 (l) 0 (o)] TJ  % Text with per-glyph adjustments
ET                               % Values in TJ array are in 1/1000 em units
```

## Recommended Fix for render_unified_layout()

### Strategy: Group by Lines + Use Relative Positioning

1. **Group clusters by line**: Collect all clusters that have the same Y coordinate (within tolerance)
2. **Set position once per line**: Use `Td` to position at start of each line
3. **Let PDF auto-advance**: Within each line, let PDF automatically advance the cursor
4. **Use TJ array for spacing**: If precise glyph positioning is needed, use TJ array with adjustments

### Example Implementation:
```rust
// Group clusters by line
let mut lines: Vec<Vec<Cluster>> = group_clusters_by_line(layout);

ops.push(Op::StartTextSection);

for (line_idx, line_clusters) in lines.iter().enumerate() {
    // Position at start of line
    let line_y = line_clusters[0].position.y;
    let start_x = line_clusters[0].position.x;
    
    if line_idx == 0 {
        // First line: absolute positioning via Td
        ops.push(Op::SetTextCursor { 
            pos: Point::new(Mm(start_x * 0.352777), Mm((page_height - line_y) * 0.352777)) 
        });
    } else {
        // Subsequent lines: relative positioning
        let prev_line_y = lines[line_idx - 1][0].position.y;
        let y_offset = prev_line_y - line_y;  // Move down by line spacing
        ops.push(Op::SetTextCursor { 
            pos: Point::new(Mm(0.0), Mm(-y_offset * 0.352777))  // Relative offset
        });
    }
    
    // Render all glyphs in this line as a single TJ operation
    let text_items = build_text_items_for_line(line_clusters);
    ops.push(Op::ShowText { items: text_items });
}

ops.push(Op::EndTextSection);
```

## Alternative: Use Absolute Matrix Positioning (Tm)

For complex layouts with rotated text, overlapping elements, etc., use `Tm` operator:

```rust
for cluster in clusters {
    // Set absolute position for each text run using Tm matrix
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::translate(absolute_x, absolute_y)
    });
    ops.push(Op::ShowText { items: cluster_text });
}
```

This requires adding a `SetTextMatrix` operation to the Op enum.

## Immediate Fix Needed

The current code treats `Td` as absolute positioning, but it's **relative**. This causes every glyph to be positioned relative to the previous glyph's end position, creating the "scattered characters" problem.

**Solution**: Group by lines and use proper relative positioning, or switch to absolute matrix positioning with `Tm`.