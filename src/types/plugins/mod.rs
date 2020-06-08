//! Any kind of PDF objects. Every object that can be added to the PDF is a "plugin".
//!
//! Plugins are type that do not have to do with the core structure of PDF
//! (such as pagination, etc) but are "extensions" to the core PDF model.
//!
//! They can range from simple (graphics, video, audio, annotations, etc.)
//!
//! A public-facing plugin must implement `IntoPdfObject`. Internally, a plugin
//! may consist of sub-plugins (for example stream objects).

pub mod graphics;
pub mod interactive;
pub mod media;
pub mod misc;
pub mod security;
pub mod xmp;
