# Contribution and License

The goal of `printpdf` is to be a complete PDF library so you could technically go ahead and build an alternative Illustrator 
just using this library. The library is commercially maintained by my company, Maps4Print (we make printable PDF maps from 
OpenStreetMap data). It was thought of a library for mapping and GIS. Instantiating content, minimal file size (useful when 
you have many polygons / roads / shapes) are the goals.

By contributing to this library, you license your code under the MIT license.

# Testing

I am a bit lazy with testing, I admit this. Each object should have, in an ideal case:

* One or multiple test testing that the object correctly serializes into an object or operation
* A test outputting the specific object with minimal extra PDF content to a `test_[object name].pdf` so that the effect of the object can be seen.
* A test showing that the object does not affect other objects (only sometimes necessary)

# General guides

* Keep your functions short (should fit vertically into one 1080p screen)
* Document at least on one line per function (or copy / paste from the Adobe reference)
* Files should be kept together by semantic function. It's OK to have many structs in one file as long as they belong together
(ex. the `extgstate.rs` file)
* Functions that add content to the document should begin with `add_`, functions that reuse references begin with `use_`,
functions that modify the state of something in the PDF should begin with `set_`.

Have fun!

-- Felix
