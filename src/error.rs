use std::fmt::{Display, Formatter};

use tonic::Status;

#[derive(Debug)]
pub enum ArisaError {
    InvalidArgument(String),
    NotFound(String),
    Internal(String),
}

impl Display for ArisaError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArgument(message) | Self::NotFound(message) | Self::Internal(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for ArisaError {}

impl From<ArisaError> for Status {
    fn from(error: ArisaError) -> Self {
        match error {
            ArisaError::InvalidArgument(message) => Status::invalid_argument(message),
            ArisaError::NotFound(message) => Status::not_found(message),
            ArisaError::Internal(message) => Status::internal(message),
        }
    }
}

impl From<String> for ArisaError {
    fn from(message: String) -> Self {
        Self::Internal(message)
    }
}
