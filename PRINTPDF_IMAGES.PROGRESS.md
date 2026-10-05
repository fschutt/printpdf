# PRINTPDF-IMAGES progress

Branch `fix/html-images-decoded-per-page` (from origin/azul-codegen-api 84dce8c).
Issue 4 of pdfocr/results/engine-issues/README.md: every image in the map is
decoded for every page.

## DONE

(nothing yet)

## NEXT

1. RED: tests that a picture the page does not show is neither decoded nor fatal
   (unit tests in src/html/mod.rs with a decode-count hook, tests/html_images_on_demand.rs).
2. Fix: look up / decode only the `<img src>` the laid-out pages draw.
3. PRINTPDF_IMAGES_REPORT.md.
