//! A checkbox tree shared by commands, independent of targets and profiles.

use std::ops::ControlFlow;

use crate::error::Result;

use super::interaction::{self, Component, Frame, Key, KeyEvent, key_code};

mod render;
mod state;
mod style;
#[cfg(test)]
mod tests;
mod view;

pub(crate) use state::Item;
use state::Selection;
pub(crate) use style::TreeStyle;
use view::View;

/// Return checked leaf values in display order. The tree starts unchecked.
/// Callers use `Item::is_empty` to report an operation-specific empty-selection error.
pub(crate) fn select<T>(prompt: &str, root: Item<T>, style: TreeStyle) -> Result<Vec<T>> {
    let mut selection = Selection::new(root, style);
    let mut tree = Tree::new(&mut selection);
    interaction::run(prompt, &mut tree)?;
    Ok(selection.into_selected())
}

struct Tree<'a, T> {
    selection: &'a mut Selection<T>,
    view: View,
    empty_submission: bool,
}

impl<'a, T> Tree<'a, T> {
    fn new(selection: &'a mut Selection<T>) -> Self {
        Self {
            view: View::new(&selection.rows),
            selection,
            empty_submission: false,
        }
    }
}

impl<T> Component for Tree<'_, T> {
    type Output = ();

    fn cancel_keys(&self) -> &[Key] {
        if self.view.editing() {
            &[]
        } else if self.view.query().is_some() {
            &[Key::Char('q')]
        } else {
            &[Key::Esc, Key::Char('q')]
        }
    }

    fn render(&self, frame: &mut Frame) {
        render::render(frame, self.selection, &self.view, self.empty_submission);
    }

    fn handle(&mut self, key: KeyEvent) -> ControlFlow<()> {
        let Some(key) = key_code(key).and_then(|key| self.view.handle(key, &self.selection.rows))
        else {
            return ControlFlow::Continue(());
        };
        match key {
            Key::Char(' ') => {
                if let Some(row) = self.view.current() {
                    self.selection.toggle(row);
                }
            }
            Key::Char('a') => self.selection.toggle_rows(&self.view.rows),
            Key::Enter if !self.selection.selected().is_empty() => return ControlFlow::Break(()),
            Key::Enter => self.empty_submission = true,
            _ => {}
        }
        ControlFlow::Continue(())
    }
}
