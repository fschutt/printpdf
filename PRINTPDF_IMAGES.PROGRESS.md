# PRINTPDF-IMAGES progress

Branch `fix/html-images-decoded-per-page` (from origin/azul-codegen-api 84dce8c).
Issue 4 of pdfocr/results/engine-issues/README.md: every image in the map is
decoded for every page.

## DONE

- ebd2b37 docs: this progress file.
- 3643f5a RED: `an_image_the_page_does_not_reference_is_not_decoded` (+ images/jpeg
  `the_picture_a_page_shows_is_decoded_once_and_no_other`) in src/html/mod.rs,
  tests/html_images_on_demand.rs (4 tests), decode-count hook `HTML_IMAGE_DECODES`
  (cfg(test), behaviour-neutral) in src/html/bridge.rs.
- 246c194 fix: the render collects the `<img src>` the display lists draw
  (`bridge::referenced_image_srcs`) and looks up / base64-decodes / decodes / embeds
  only those; from_html* no longer copies the images map; CHANGELOG entry.
- 1d805c9 unit test for `referenced_image_srcs`.
- PRINTPDF_IMAGES_REPORT.md (this commit).

## NEXT

- Coordinator: compile and run the tests (command in PRINTPDF_IMAGES_REPORT.md).
- Not done on purpose: a cross-call decoded-image cache (see the report).
