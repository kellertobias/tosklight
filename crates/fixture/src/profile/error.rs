use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ProfileError {
    #[error("invalid fixture profile: {0}")]
    Invalid(String),
}
