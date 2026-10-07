use std::ops::ControlFlow;

use super::{Component, Frame, Key, KeyEvent, key_code};

pub(super) struct Confirm;

impl Component for Confirm {
    type Output = bool;

    fn cancel_keys(&self) -> &[Key] {
        &[Key::Esc, Key::Char('q')]
    }

    fn render(&self, frame: &mut Frame) {
        frame.line(format!("{} [y/N]", frame.prompt()));
    }

    fn handle(&mut self, key: KeyEvent) -> ControlFlow<bool> {
        match key_code(key) {
            Some(Key::Char('y' | 'Y')) => ControlFlow::Break(true),
            Some(Key::Char('n' | 'N') | Key::Enter) => ControlFlow::Break(false),
            _ => ControlFlow::Continue(()),
        }
    }

    fn report(&self, output: &bool) -> Option<String> {
        Some(if *output { "yes" } else { "no" }.into())
    }
}
