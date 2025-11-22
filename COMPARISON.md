## printpdf vs. other PDF libraries

### printpdf vs. lopdf

- **printpdf**: Provides a higher-level API for both reading and writing PDFs with strong support for graphics, text rendering, and page layout.
- **lopdf**: Focuses on low-level PDF manipulation and is excellent for tasks like concatenating PDFs or performing post-processing operations. Often used alongside other libraries (including printpdf) when post-processing is needed.

### printpdf vs. genpdf

- **printpdf**: Offers direct control over PDF elements with a lower-level API that closely maps to PDF structures.
- **genpdf**: Built on top of printpdf, providing a more user-friendly, high-level document generation API with automatic page layout and text wrapping. Great for document-focused applications where layout should be handled automatically, but sacrifices some fine-grained control.

### printpdf vs. pdf-writer

- **printpdf**: Balanced approach with both high-level convenience functions and access to lower-level PDF structures.
- **pdf-writer**: Step-by-step PDF writer focusing on minimalism and performance with a strongly-typed API. More low-level than printpdf with less abstraction but potentially better performance for certain tasks.

### printpdf vs. Typst-based solutions (krilla, typst-pdf)

- **printpdf**: Direct PDF generation with programmatic control over all elements.
- **Typst-based libraries**: Much faster rendering speed than LaTeX-based solutions with modern document syntax. Particularly good for text-heavy documents with complex layouts, but requires learning Typst markup language.

### When to choose printpdf

printpdf is ideal when you need:

- Both reading and writing capabilities in a single library
- Strong graphics and SVG support
- Font embedding with Unicode support
- Direct control over PDF structures while still having convenience functions
- HTML conversion capabilities (experimental)

Choose one of the alternatives when:

- You need a higher-level document layout system (→ genpdf)
- You're working with extremely performance-sensitive applications (→ pdf-writer)
- You primarily need to manipulate existing PDFs and are comfortable with very low-level APIs (→ lopdf)
- You have complex document typesetting needs (→ Typst-based solutions)
