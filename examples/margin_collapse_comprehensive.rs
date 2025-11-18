extern crate printpdf;
use printpdf::*;
use std::collections::BTreeMap;

/// Comprehensive margin-collapsing tests covering all CSS 2.1 Section 8.3.1 cases
fn main() {
    // Test 1: Empty block collapse through
    test_empty_block_collapse_through();
    
    // Test 2: Parent-child top margin escape
    test_parent_child_top_escape();
    
    // Test 3: Parent-child bottom margin escape
    test_parent_child_bottom_escape();
    
    // Test 4: Complex nested scenario
    test_complex_nested_collapse();
    
    // Test 5: Multiple empty blocks
    test_multiple_empty_blocks();
    
    // Test 6: Parent-child with blockers
    test_parent_child_with_blockers();
}

fn test_empty_block_collapse_through() {
    println!("\n=== Test 1: Empty Block Collapse Through ===");
    
    let html = r#"
<!DOCTYPE html>
<html>
<body style="margin: 0; padding: 20px;">
<div style="margin-bottom: 20px; height: 50px; background: lightblue;">Box 1</div>
<div style="margin-top: 10px; margin-bottom: 30px;"></div>
<div style="margin-top: 15px; height: 50px; background: lightgreen;">Box 2</div>
</body>
</html>
"#;

    println!("Expected: Box1(margin-bottom: 20px) -> EmptyDiv(10/30px) -> Box2(margin-top: 15px)");
    println!("  Empty div margins collapse with each other: max(10, 30) = 30px");
    println!("  Then collapse with siblings: max(20, 30, 15) = 30px");
    println!("  Expected gap: 30px (not 20+10+30+15=75px)");
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();
    
    match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            let mut save_warnings = Vec::new();
            let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut save_warnings);
            std::fs::write("margin_collapse_empty_through.pdf", &pdf_bytes).unwrap();
            println!("✓ Generated: margin_collapse_empty_through.pdf");
        }
        Err(e) => println!("✗ Error: {:?}", e),
    }
}

fn test_parent_child_top_escape() {
    println!("\n=== Test 2: Parent-Child Top Margin Escape ===");
    
    let html = r#"
<!DOCTYPE html>
<html>
<body style="margin: 0; padding: 20px;">
<div style="margin-top: 20px; background: lightgray;">
  <div style="margin-top: 30px; height: 50px; background: lightblue;">Child (margin-top: 30px)</div>
</div>
</body>
</html>
"#;

    println!("Expected: Parent has no border/padding, so child's margin-top escapes");
    println!("  Child margin-top: 30px should 'escape' outside parent");
    println!("  Parent's margin-top: 20px collapses with child's 30px");
    println!("  Result: max(20, 30) = 30px from body to child content");
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();
    
    match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            let mut save_warnings = Vec::new();
            let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut save_warnings);
            std::fs::write("margin_collapse_parent_child_top.pdf", &pdf_bytes).unwrap();
            println!("✓ Generated: margin_collapse_parent_child_top.pdf");
        }
        Err(e) => println!("✗ Error: {:?}", e),
    }
}

fn test_parent_child_bottom_escape() {
    println!("\n=== Test 3: Parent-Child Bottom Margin Escape ===");
    
    let html = r#"
<!DOCTYPE html>
<html>
<body style="margin: 0; padding: 20px;">
<div style="margin-bottom: 20px; background: lightgray;">
  <div style="margin-bottom: 30px; height: 50px; background: lightblue;">Child (margin-bottom: 30px)</div>
</div>
<div style="margin-top: 15px; height: 50px; background: lightgreen;">Next sibling</div>
</body>
</html>
"#;

    println!("Expected: Parent has no border/padding/height, so child's margin-bottom escapes");
    println!("  Child margin-bottom: 30px escapes parent");
    println!("  Parent margin-bottom: 20px collapses with child's 30px");
    println!("  Result collapses with next sibling: max(30, 15) = 30px gap");
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();
    
    match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            let mut save_warnings = Vec::new();
            let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut save_warnings);
            std::fs::write("margin_collapse_parent_child_bottom.pdf", &pdf_bytes).unwrap();
            println!("✓ Generated: margin_collapse_parent_child_bottom.pdf");
        }
        Err(e) => println!("✗ Error: {:?}", e),
    }
}

fn test_complex_nested_collapse() {
    println!("\n=== Test 4: Complex Nested Scenario ===");
    
    let html = r#"
<!DOCTYPE html>
<html>
<body style="margin: 0; padding: 20px;">
<div style="margin-top: 10px; margin-bottom: 10px; background: #f0f0f0;">
  <div style="margin-top: 20px; margin-bottom: 20px; background: #e0e0e0;">
    <div style="margin-top: 30px; margin-bottom: 30px; height: 50px; background: lightblue;">Inner content</div>
  </div>
</div>
<div style="margin-top: 15px; height: 50px; background: lightgreen;">After</div>
</body>
</html>
"#;

    println!("Expected: Triple-nested margin escape");
    println!("  Top: max(10, 20, 30) = 30px from body to inner content");
    println!("  Bottom: max(30, 20, 10, 15) = 30px from inner to after");
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();
    
    match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            let mut save_warnings = Vec::new();
            let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut save_warnings);
            std::fs::write("margin_collapse_complex_nested.pdf", &pdf_bytes).unwrap();
            println!("✓ Generated: margin_collapse_complex_nested.pdf");
        }
        Err(e) => println!("✗ Error: {:?}", e),
    }
}

fn test_multiple_empty_blocks() {
    println!("\n=== Test 5: Multiple Empty Blocks ===");
    
    let html = r#"
<!DOCTYPE html>
<html>
<body style="margin: 0; padding: 20px;">
<div style="margin-bottom: 20px; height: 50px; background: lightblue;">Box 1</div>
<div style="margin-top: 10px; margin-bottom: 25px;"></div>
<div style="margin-top: 15px; margin-bottom: 30px;"></div>
<div style="margin-top: 12px; height: 50px; background: lightgreen;">Box 2</div>
</body>
</html>
"#;

    println!("Expected: Multiple empty blocks collapse through");
    println!("  Empty1: max(10, 25) = 25px");
    println!("  Empty2: max(15, 30) = 30px");
    println!("  All collapse: max(20, 25, 30, 12) = 30px total gap");
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();
    
    match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            let mut save_warnings = Vec::new();
            let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut save_warnings);
            std::fs::write("margin_collapse_multiple_empty.pdf", &pdf_bytes).unwrap();
            println!("✓ Generated: margin_collapse_multiple_empty.pdf");
        }
        Err(e) => println!("✗ Error: {:?}", e),
    }
}

fn test_parent_child_with_blockers() {
    println!("\n=== Test 6: Parent-Child with Blockers ===");
    
    let html = r#"
<!DOCTYPE html>
<html>
<body style="margin: 0; padding: 20px;">
<div style="margin-top: 20px; background: #f0f0f0;">
  <div style="margin-top: 30px; height: 50px; background: lightblue;">Child (should escape)</div>
</div>
<div style="height: 30px;"></div>
<div style="margin-top: 20px; border-top: 2px solid black; background: #e0e0e0;">
  <div style="margin-top: 30px; height: 50px; background: lightblue;">Child (should NOT escape)</div>
</div>
</body>
</html>
"#;

    println!("Expected:");
    println!("  First parent: No blocker, child margin escapes = 30px from body");
    println!("  Second parent: Border blocker, margins don't collapse = 20px + 30px = 50px");
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();
    
    match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            let mut save_warnings = Vec::new();
            let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut save_warnings);
            std::fs::write("margin_collapse_parent_blockers.pdf", &pdf_bytes).unwrap();
            println!("✓ Generated: margin_collapse_parent_blockers.pdf");
        }
        Err(e) => println!("✗ Error: {:?}", e),
    }
}
