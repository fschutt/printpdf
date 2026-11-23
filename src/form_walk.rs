//! Limits on how deeply forms nest.
//!
//! A form can draw other forms, and a document can be built so that this never ends or cannot be
//! finished: a form can draw itself, and ten forms that each draw the next one ten times are ten
//! billion visits. What reads or writes forms stops at [`MAX_FORM_DEPTH`] nested forms.

/// The deepest nesting of forms that is read from a PDF, written and walked: a form drawn by a
/// form drawn by a form and so on.
pub(crate) const MAX_FORM_DEPTH: usize = 16;
