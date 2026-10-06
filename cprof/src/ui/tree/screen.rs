use std::io;

use dialoguer::console::{Term, measure_text_width, truncate_str};

use super::{
    state::{Check, Selection},
    view::View,
};
use crate::ui::style::{heading, label};

/// Restore the cursor and remove the prompt on success, cancellation or I/O failure.
pub(super) struct Screen {
    pub(super) term: Term,
    lines: usize,
    input_line: bool,
}

impl Screen {
    pub(super) fn new() -> io::Result<Self> {
        let screen = Self {
            term: Term::buffered_stderr(),
            lines: 0,
            input_line: false,
        };
        screen.term.hide_cursor()?;
        Ok(screen)
    }

    pub(super) fn render<T>(
        &mut self,
        title: &str,
        selection: &Selection<T>,
        view: &View,
        empty_submission: bool,
    ) -> io::Result<()> {
        self.clear()?;
        let (height, width) = self.term.size();
        let capacity = usize::from(height)
            .saturating_sub(if view.query().is_some() { 5 } else { 4 })
            .max(1);
        let width = usize::from(width).saturating_sub(1);
        let start = view.cursor / capacity * capacity;
        let end = (start + capacity).min(view.rows.len());
        let hint = if view.editing() {
            "Type to search | Enter navigate | Esc clear"
        } else if view.query().is_some() {
            "j/k move | Space toggle | a matches | / search | Esc clear | Enter confirm"
        } else {
            "j/k move | Space toggle | a all | Enter confirm | / search"
        };
        self.line(&heading(title).to_string(), width)?;
        self.line(hint, width)?;
        for index in start..end {
            let row = view.rows[index];
            let marker = match selection.check(row) {
                Check::Empty | Check::None => "[ ]",
                Check::Partial => "[-]",
                Check::All => "[x]",
            };
            let row = &selection.rows[row];
            let (prefix, name) = if view.query().is_some() {
                ("", row.path.as_str())
            } else {
                (row.prefix.as_str(), row.label.as_str())
            };
            let line = format!(
                "{} {prefix}{marker} {name}",
                if view.cursor == index { ">" } else { " " },
            );
            let line = if view.cursor == index {
                label(&line).to_string()
            } else {
                line
            };
            self.line(&line, width)?;
        }
        if view.rows.is_empty() {
            self.line("No matches", width)?;
        }
        let selected = selection.selected().len();
        let status = if empty_submission && selected == 0 {
            "Select at least one item".to_string()
        } else {
            let pages = view.rows.len().div_ceil(capacity);
            let paging = if pages > 1 {
                format!(" | page {}/{}", view.cursor / capacity + 1, pages)
            } else {
                String::new()
            };
            format!("{selected} selected{paging} | [-] partially selected")
        };
        self.line(&status, width)?;
        if let Some(query) = view.query() {
            let line = search_line(query, width);
            if view.editing() {
                // Leave the cursor after the input, including trailing spaces.
                self.input_line = true;
                self.term.write_str(&line)?;
                self.term.show_cursor()?;
            } else {
                self.line(&line, width)?;
            }
        }
        self.term.flush()
    }

    fn line(&mut self, line: &str, width: usize) -> io::Result<()> {
        self.term.write_line(&truncate_str(line, width, ""))?;
        self.lines += 1;
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.term.hide_cursor()?;
        if self.input_line {
            self.term.clear_line()?;
            self.input_line = false;
        }
        self.term.clear_last_lines(self.lines)?;
        self.lines = 0;
        Ok(())
    }
}

/// Keep the most recently typed characters visible without splitting UTF-8.
pub(super) fn search_line(mut query: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    while measure_text_width(query) > width - 1 {
        query = &query[query.chars().next().unwrap().len_utf8()..];
    }
    format!("/{query}")
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = self.clear();
        let _ = self.term.show_cursor();
        let _ = self.term.flush();
    }
}
