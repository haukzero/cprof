//! A checkbox tree shared by commands, independent of targets and profiles.

use std::io;

use dialoguer::console::Key;

use crate::error::Result;

use super::interaction;

mod screen;
mod state;
mod style;
#[cfg(test)]
mod tests;
mod view;

use screen::Screen;
pub(crate) use state::Item;
use state::Selection;
pub(crate) use style::TreeStyle;
use view::View;

/// Return checked leaf values in display order. The tree starts unchecked.
/// Callers use `Item::is_empty` to report an operation-specific empty-selection error.
pub(crate) fn select<T>(prompt: &str, root: Item<T>, style: TreeStyle) -> Result<Vec<T>> {
    let mut selection = Selection::new(root, style);
    interaction::interact(prompt, &[Key::Char('q'), Key::CtrlC], |interaction| {
        interact(prompt, interaction, &mut selection).map_err(Into::into)
    })?;
    Ok(selection.into_selected())
}

fn interact<T>(
    title: &str,
    prompt: &interaction::Prompt,
    selection: &mut Selection<T>,
) -> io::Result<Option<()>> {
    let mut screen = Screen::new()?;
    let mut view = View::new(&selection.rows);
    let mut empty_submission = false;
    loop {
        let title = if view.editing() {
            format!("{title} (Ctrl-C cancel)")
        } else if view.query().is_some() {
            format!("{title} (q/Ctrl-C cancel)")
        } else {
            prompt.text.clone()
        };
        screen.render(&title, selection, &view, empty_submission)?;
        let key = if view.query().is_some() {
            // Search owns Esc, and every printable character is text while editing.
            match screen.term.read_key_raw()? {
                Key::CtrlC => return Ok(None),
                Key::Char('q') if !view.editing() => return Ok(None),
                key => key,
            }
        } else {
            let Some(key) = prompt.read_key(&screen.term)? else {
                return Ok(None);
            };
            key
        };
        let Some(key) = view.handle(key, &selection.rows) else {
            continue;
        };
        match key {
            Key::Char(' ') => {
                if let Some(row) = view.current() {
                    selection.toggle(row);
                }
            }
            Key::Char('a') => selection.toggle_rows(&view.rows),
            Key::Enter if !selection.selected().is_empty() => return Ok(Some(())),
            Key::Enter => empty_submission = true,
            _ => {}
        }
    }
}
