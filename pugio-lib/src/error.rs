use thiserror::Error;

/// This type represents errors that occur during [`Template`](crate::template::Template)
/// construction.
#[derive(Error, Debug)]
#[error("invalid template: {0}")]
pub struct TemplateError(#[from] tinytemplate::error::Error);
