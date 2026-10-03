//! A checkbox tree shared by commands, independent of targets and profiles.

use std::io;

use dialoguer::console::{Key, Term, truncate_str};

use crate::error::Result;

use super::{
    interaction,
    style::{heading, label},
};

mod state;
mod style;
#[cfg(test)]
mod tests;

pub(crate) use state::Item;
use state::{Check, Selection};
pub(crate) use style::TreeStyle;

/// Return checked leaf values in display order. The tree starts unchecked.
/// Callers use `Item::is_empty` to report an operation-specific empty-selection error.
pub(crate) fn select<T>(prompt: &str, root: Item<T>, style: TreeStyle) -> Result<Vec<T>> {
    let mut selection = Selection::new(root, style);
    interaction::interact(prompt, &[Key::Char('q'), Key::CtrlC], |prompt| {
        interact(prompt, &mut selection).map_err(Into::into)
    })?;
    Ok(selection.into_selected())
}

fn interact<T>(
    prompt: &interaction::Prompt,
    selection: &mut Selection<T>,
) -> io::Result<Option<()>> {
    let mut screen = Screen {
        term: Term::buffered_stderr(),
        lines: 0,
    };
    screen.term.hide_cursor()?;
    let mut cursor = 0;
    let mut empty_submission = false;
    loop {
        screen.clear()?;
        let (height, width) = screen.term.size();
        let capacity = usize::from(height).saturating_sub(4).max(1);
        let width = usize::from(width).saturating_sub(1);
        let start = cursor / capacity * capacity;
        let end = (start + capacity).min(selection.rows.len());
        screen.line(&heading(&prompt.text).to_string(), width)?;
        screen.line("j/k move | Space toggle | a all | Enter confirm", width)?;
        for index in start..end {
            let marker = match selection.check(index) {
                Check::Empty | Check::None => "[ ]",
                Check::Partial => "[-]",
                Check::All => "[x]",
            };
            let line = format!(
                "{} {}{marker} {}",
                if cursor == index { ">" } else { " " },
                selection.rows[index].prefix,
                selection.rows[index].label,
            );
            let line = if cursor == index {
                label(&line).to_string()
            } else {
                line
            };
            screen.line(&line, width)?;
        }
        let selected = selection.selected();
        let status = if empty_submission && selected.is_empty() {
            "Select at least one item".to_string()
        } else {
            let pages = selection.rows.len().div_ceil(capacity);
            let paging = if pages > 1 {
                format!(" | page {}/{}", cursor / capacity + 1, pages)
            } else {
                String::new()
            };
            format!(
                "{} selected{paging} | [-] partially selected",
                selected.len(),
            )
        };
        screen.line(&status, width)?;
        screen.term.flush()?;
        let Some(key) = prompt.read_key(&screen.term)? else {
            return Ok(None);
        };
        match key {
            Key::ArrowDown | Key::Tab | Key::Char('j') => {
                cursor = (cursor + 1) % selection.rows.len();
            }
            Key::ArrowUp | Key::BackTab | Key::Char('k') => {
                cursor = (cursor + selection.rows.len() - 1) % selection.rows.len();
            }
            Key::Home => cursor = 0,
            Key::End => cursor = selection.rows.len() - 1,
            Key::Char(' ') => selection.toggle(cursor),
            Key::Char('a') => selection.toggle(0),
            Key::Enter if !selected.is_empty() => return Ok(Some(())),
            Key::Enter => empty_submission = true,
            _ => {}
        }
    }
}

/// Restore the cursor and remove the prompt on success, cancellation or I/O failure.
struct Screen {
    term: Term,
    lines: usize,
}

impl Screen {
    fn line(&mut self, line: &str, width: usize) -> io::Result<()> {
        self.term.write_line(&truncate_str(line, width, ""))?;
        self.lines += 1;
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.term.clear_last_lines(self.lines)?;
        self.lines = 0;
        Ok(())
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = self.clear();
        let _ = self.term.show_cursor();
        let _ = self.term.flush();
    }
}
