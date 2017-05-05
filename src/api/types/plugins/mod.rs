//! Plugins are type that do not have to do with the core structure of PDF
//! (such as pagination, etc) but are "extensions" to the core PDF model.
//! 
//! They can range from simple (graphics, video, audio, annotations, etc.)
//!
//! A public-facing plugin must implement IntoPdfObject. Internally, a plugin 
//! may consist of sub-plugins (for example stream objects).

pub mod graphics;
pub mod media;
pub mod interactive;

use graphics::*;
use media::*;
use interactive::*;