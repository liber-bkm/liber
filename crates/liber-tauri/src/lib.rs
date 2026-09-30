use liber_core::CoreError;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn placeholder() -> Result<(), CoreError> {
    Err(CoreError::Invalid("not implemented".to_string()))
}
