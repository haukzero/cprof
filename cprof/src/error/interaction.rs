/// Terminal availability and interactive prompt failures.
#[derive(Debug, thiserror::Error)]
pub enum InteractionError {
    #[error("Interactive input required but no interactive terminal is available")]
    InputRequired,

    #[error("Interaction cancelled")]
    Cancelled,

    #[error("Interactive prompt failed: {0}")]
    Prompt(#[from] std::io::Error),
}
