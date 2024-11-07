# printpdf XML syntax

To optimize for small compile size and quick PDF rendering, the XML syntax does
not support all features like wkhtmltopdf does, but it's close enough to be usable
in practice.

The XML syntax intentionally doesn't support everything that HTML does, however, it's
easier to configure than LaTeX or other proprietary-ish XML formats.

## Basic structure

The `<body>` node will automatically break into pages, depending on whether 

```xml
<html title="Document title">
    <head>

        <header exclude-pages="1"> <!-- do not render header on page 1 -->
            <p style="color:red">This will be at the top of each page</p>
            <hr style="color:green;">
        </header>

        <footer>
            <hr style="color:blue;"> <!-- do not render footer on page 1 -->
            <p style="color:black">This will be at the bottom of each page</p>
        </footer>

        <!-- 
            CAREFUL: In difference to standard CSS, this 
            is applied AFTER the inline styling, to allow 
            for overriding the styles of preconfigured components 
        -->
        <style>
            * { color: red; }
        </style>
    </head>
    <body style="padding:10mm">

        <!-- 100% width / height corresponds to the page width / height -->
        <!-- so this will fill up exactly one page -->
        <div class="titlepage" style="width:100%;height:100%;">
            <h1>Title of my book</h1>
        </div>

        <!-- this will be rendered on the second page -->
        <div class="titlepage" style="width:100%;height:100%;">
            <h1 style="background:yellow;font-family:sans-serif;">Hello World!</h1>
            <!-- temp1.png has to be added to the XMLRenderingOptions image map-->
            <!-- 1px = 1mm in the PDF -->
            <img src="temp1.png" style="width:500px;height:500px"></img>
        </div>

        <!-- The layout system is similar to flexbox with minor differences: -->
        <!-- First, everything will be expanded to its MINIMUM size (text, images, etc.) -->
        <!-- Then, if space is still remaining and flex-grow > 0, the space will be expanded -->
        <!-- to its maximum, while respecting max-width / max-height -->

        <!-- So, this div will take up one entire page, since no max-height is set -->
        <div style="background:blue;padding:10px;display:flex;flex-grow:1;">

        </div>
    <body>
</html>
```

## CSS features

| key                            | example values                                                       |
| ------------------------------ | -------------------------------------------------------------------- |
| `display`                      | `block, inline-block, flex (default)`                                | 
| `float`                        | `left, right, both`                                                  | 
| `box-sizing`                   | `border-box, content-box`                                            | 
| `color`                        | `red, green, ..., #efefef, rgb(), rgba(), hsl(), hsla()`             | 
| `font-size`                    | `10px, 5pt, 40%, 10em, 5rem`                                         | 
| `font-family`                  | `sans-serif, serif, ..., "Times New Roman"`                          | 
| `text-align`                   | `left, center, right`                                                | 
| `letter-spacing`               | `0.0 - infinite`                                                     | 
| `line-height`                  | `0.0 - infinite`                                                     | 
| `word-spacing`                 | `0.0 - infinite`                                                     | 
| `tab-width`                    | `0.0 - infinite`                                                     | 
| `width`                        | `10px, 5%, 10rem, 5em`                                               | 
| `height`                       | `10px, 5%, 10rem, 5em`                                               | 
| `min-width`                    | `10px, 5%, 10rem, 5em`                                               | 
| `min-height`                   | `10px, 5%, 10rem, 5em`                                               | 
| `max-width`                    | `10px, 5%, 10rem, 5em`                                               | 
| `max-height`                   | `10px, 5%, 10rem, 5em`                                               | 
| `position`                     | `static (default), relative, absolute, fixed`                        | 
| `top`                          | `10px, 5%, 10rem, 5em (+position:absolute / fixed)`                  | 
| `right`                        | `10px, 5%, 10rem, 5em (+position:absolute / fixed)`                  | 
| `left`                         | `10px, 5%, 10rem, 5em (+position:absolute / fixed)`                  | 
| `bottom`                       | `10px, 5%, 10rem, 5em (+position:absolute / fixed)`                  | 
| `flex-wrap`                    | `wrap, no-wrap`                                                      | 
| `flex-direction`               | `row, column, row-reverse, column-reverse`                           | 
| `flex-grow`                    | `0.0 - infinite`                                                     | 
| `flex-shrink`                  | `0.0 - infinite`                                                     | 
| `justify-content`              | `stretch, center, flex-start, flex-end, space-between, space-around` | 
| `align-items`                  | `stretch, center, flex-start, flex-end`                              | 
| `align-content`                | `stretch, center, flex-start, flex-end, space-between, space-around` | 
| `overflow`                     | `overflow[-x, -y] auto (default), scroll, hidden, visible`           | 
| `padding`                      | `10px, 5%, 10rem, 5em`                                               | 
| `margin`                       | `10px, 5%, 10rem, 5em`                                               | 
| `background`                   | `red, [linear-, radial-, conic-]gradient(), image(id)`               | 
| `background-position`          | `10% 10%, 10px 10px, left top`                                       | 
| `background-size`              | `auto, cover, contain, 10% 40%, 100px 200px`                         | 
| `background-repeat`            | `repeat, no-repeat`                                                  | 
| `border-radius`                | `10px, 5%, 10rem, 5e`                                                | 
| `border-top-left-radius`       | `10px, 5%, 10rem, 5em`                                               | 
| `border-top-right-radius`      | `10px, 5%, 10rem, 5em`                                               | 
| `border-bottom-left-radius`    | `10px, 5%, 10rem, 5em`                                               | 
| `border-bottom-right-radius`   | `10px, 5%, 10rem, 5em`                                               | 
| `border, border-[top, ...`]    | `1px solid red, 10px dotted #efefef`                                 | 
| `border-top-width`             | `10px, 10rem, 5em (NO PERCENTAGE)`                                   | 
| `border-right-width`           | `10px, 10rem, 5em (NO PERCENTAGE)`                                   | 
| `border-left-width`            | `10px, 10rem, 5em (NO PERCENTAGE)`                                   | 
| `border-bottom-width`          | `10px, 10rem, 5em (NO PERCENTAGE)`                                   | 
| `border-top-style`             | `solid, dashed, dotted, ...`                                         | 
| `border-right-style`           | `solid, dashed, dotted, ...`                                         | 
| `border-left-style`            | `solid, dashed, dotted, ...`                                         |
| `border-bottom-style`          | `solid, dashed, dotted, ...`                                         | 
| `border-top-color`             | `red, green, ..., #efefef, rgb(), rgba(), hsl(), hsla()`             | 
| `border-right-color`           | `red, green, ..., #efefef, rgb(), rgba(), hsl(), hsla()`             | 
| `border-left-color`            | `red, green, ..., #efefef, rgb(), rgba(), hsl(), hsla()`             | 
| `border-bottom-color`          | `red, green, ..., #efefef, rgb(), rgba(), hsl(), hsla()`             | 
| `opacity`                      | `0.0 - 1.0`                                                          | 
| `transform`                    | ` matrix(), translate(), scale(), rotate(), ...`                     | 
| `perspective-origin`           | `100px 100px, 50% 50%`                                               | 
| `transform-origin`             | `100px 100px, 50% 50%`                                               | 
| `backface-visibility`          | `visible (default), hidden`                                          | 
| `box-shadow`                   | `0px 0px 10px black inset`                                           | 
| `background-color`             | `red, green, #efefefaa, rgb(), rgba(), hsl(), hsla()`                | 
| `background-image`             | `id("my-id")`                                                        | 
