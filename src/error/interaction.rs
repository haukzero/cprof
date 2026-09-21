/// Terminal availability and interactive prompt failures.
#[derive(Debug, thiserror::Error)]
pub enum InteractionError {
    #[error("Interactive input required but no interactive terminal is available")]
    InputRequired,

    #[error("Interactive prompt failed: {0}")]
    Prompt(#[from] dialoguer::Error),
}
