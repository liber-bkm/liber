use crate::CoreError;

pub fn placeholder() -> Result<(), CoreError> {
    Err(CoreError::Invalid("not implemented".to_string()))
}
