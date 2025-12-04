use thiserror::Error;

#[derive(Error, Debug)]
#[error("invalid template: {0}")]
pub struct TemplateError(#[from] tinytemplate::error::Error);
