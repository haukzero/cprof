//! Shared terminal checks, cancellation and custom prompt input.

use std::io::{self, IsTerminal};

use dialoguer::console::{Key, Term};

use crate::error::{InteractionError, Result};

mod input;
#[cfg(test)]
mod tests;

pub(crate) fn input(prompt: &str) -> Result<String> {
    interact(prompt, &[Key::CtrlC], |prompt| {
        input::interact(prompt).map_err(Into::into)
    })
}

/// Prompts return `None` on cancellation, including dialoguer's `interact_opt`.
/// Custom prompts use `Prompt::read_key`; dialoguer prompts declare their built-in cancel keys.
pub(super) fn interact<T>(
    prompt: &str,
    extra_cancel_keys: &[Key],
    interaction: impl FnOnce(&Prompt) -> dialoguer::Result<Option<T>>,
) -> Result<T> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Err(InteractionError::InputRequired.into());
    }
    finish(interaction(&Prompt::new(prompt, extra_cancel_keys)))
}

fn finish<T>(result: dialoguer::Result<Option<T>>) -> Result<T> {
    result
        .map_err(InteractionError::from)?
        .ok_or_else(|| InteractionError::Cancelled.into())
}

/// One key list drives both the displayed hint and custom prompt cancellation.
pub(super) struct Prompt {
    pub(super) text: String,
    cancel_keys: Vec<Key>,
}

impl Prompt {
    fn new(prompt: &str, extra_cancel_keys: &[Key]) -> Self {
        let mut cancel_keys = vec![Key::Escape];
        for key in extra_cancel_keys {
            if !cancel_keys.contains(key) {
                cancel_keys.push(key.clone());
            }
        }
        let keys = cancel_keys
            .iter()
            .map(key_label)
            .collect::<Vec<_>>()
            .join("/");
        Self {
            text: format!("{prompt} ({keys} cancel)"),
            cancel_keys,
        }
    }

    pub(super) fn read_key(&self, term: &Term) -> io::Result<Option<Key>> {
        Ok(self.cancel_key(term.read_key_raw()?))
    }

    fn cancel_key(&self, key: Key) -> Option<Key> {
        (!self.cancel_keys.contains(&key)).then_some(key)
    }
}

fn key_label(key: &Key) -> String {
    match key {
        Key::Escape => "Esc".into(),
        Key::CtrlC => "Ctrl-C".into(),
        Key::Enter => "Enter".into(),
        Key::Char(' ') => "Space".into(),
        Key::Char(ch) => ch.to_string(),
        key => format!("{key:?}"),
    }
}
