//! One loop owns cancellation and terminal lifetime. Components only build
//! frames, update their state and submit values.

use std::io::{self, IsTerminal};
use std::ops::ControlFlow;

use crossterm::event::{Event, KeyEventKind};
pub(super) use crossterm::event::{KeyCode as Key, KeyEvent, KeyModifiers};

use crate::error::{InteractionError, Result};

mod confirm;
mod input;
mod select;
mod terminal;
#[cfg(test)]
mod tests;

pub(super) use terminal::Frame;
use terminal::{Session, Terminal};

pub(super) trait Component {
    type Output;

    /// Current state's extra cancel keys. Ctrl-C is always handled by the loop.
    fn cancel_keys(&self) -> &[Key] {
        &[Key::Esc]
    }
    fn render(&self, frame: &mut Frame);
    fn handle(&mut self, key: KeyEvent) -> ControlFlow<Self::Output>;

    /// Optional submission transcript, written after restoring the terminal.
    fn report(&self, _output: &Self::Output) -> Option<String> {
        None
    }
}

pub(crate) fn input(prompt: &str) -> Result<String> {
    run(prompt, &mut input::Input::default())
}

pub(crate) fn confirm(prompt: &str) -> Result<bool> {
    run(prompt, &mut confirm::Confirm)
}

pub(super) fn select_one(prompt: &str, items: &[String]) -> Result<usize> {
    run(prompt, &mut select::Select::new(items))
}

pub(super) fn run<C: Component>(prompt: &str, component: &mut C) -> Result<C::Output> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Err(InteractionError::InputRequired.into());
    }
    run_on(
        prompt,
        component,
        &mut Session::new().map_err(InteractionError::from)?,
    )
}

fn run_on<C: Component>(
    prompt: &str,
    component: &mut C,
    terminal: &mut impl Terminal,
) -> Result<C::Output> {
    // Keep all fallible interaction work inside this scope so every return path
    // reaches close(). An original error takes precedence over cleanup failure.
    let result = (|| -> Result<_> {
        loop {
            let mut frame = Frame::new(
                prompt_text(prompt, component.cancel_keys()),
                terminal.size().map_err(InteractionError::from)?,
            );
            component.render(&mut frame);
            terminal.draw(&frame).map_err(InteractionError::from)?;
            if let Event::Key(key) = terminal.read().map_err(InteractionError::from)?
                && key.kind != KeyEventKind::Release
            {
                if is_cancel(key, component.cancel_keys()) {
                    return Err(InteractionError::Cancelled.into());
                }
                if let ControlFlow::Break(value) = component.handle(key) {
                    return Ok(value);
                }
            }
        }
    })();
    let cleanup = terminal.close();
    let output = result?;
    cleanup.map_err(InteractionError::from)?;
    if let Some(report) = component.report(&output) {
        terminal
            .report(&format!("{prompt}: {report}"))
            .map_err(InteractionError::from)?;
    }
    Ok(output)
}

fn is_cancel(key: KeyEvent, extra: &[Key]) -> bool {
    (key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, Key::Char('c' | 'C')))
        || key_code(key).is_some_and(|code| extra.contains(&code))
}

fn prompt_text(prompt: &str, extra: &[Key]) -> String {
    let mut labels = Vec::new();
    for key in extra {
        let label = match key {
            Key::Esc => "Esc".into(),
            Key::Char(' ') => "Space".into(),
            Key::Char(ch) => ch.to_string(),
            key => format!("{key:?}"),
        };
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels.push("Ctrl-C".into());
    format!("{prompt} ({} cancel)", labels.join("/"))
}

/// Ordinary keys and existing Home/End shortcuts. Text editing handles its
/// additional Ctrl/Alt word-motion bindings using the original KeyEvent.
pub(super) fn key_code(key: KeyEvent) -> Option<Key> {
    match (key.modifiers - KeyModifiers::SHIFT, key.code) {
        (KeyModifiers::CONTROL, Key::Char('a' | 'A')) => Some(Key::Home),
        (KeyModifiers::CONTROL, Key::Char('e' | 'E')) => Some(Key::End),
        (KeyModifiers::NONE, Key::Tab) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            Some(Key::BackTab)
        }
        (KeyModifiers::NONE, code) => Some(code),
        _ => None,
    }
}
