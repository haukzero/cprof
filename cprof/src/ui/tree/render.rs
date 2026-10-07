use console::measure_text_width;

use super::{
    state::{Check, Selection},
    view::View,
};
use crate::ui::{
    interaction::Frame,
    style::{heading, label},
};

pub(super) fn render<T>(
    frame: &mut Frame,
    selection: &Selection<T>,
    view: &View,
    empty_submission: bool,
) {
    let capacity = frame
        .height()
        .saturating_sub(if view.query().is_some() { 4 } else { 3 })
        .max(1);
    let start = view.cursor / capacity * capacity;
    let end = (start + capacity).min(view.rows.len());
    let hint = if view.editing() {
        "Type to search | Enter navigate | Esc clear"
    } else if view.query().is_some() {
        "j/k move | Space toggle | a matches | / search | Esc clear | Enter confirm"
    } else {
        "j/k move | Space toggle | a all | Enter confirm | / search"
    };
    frame.line(heading(frame.prompt()).to_string());
    frame.line(hint);
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
            if view.cursor == index { ">" } else { " " }
        );
        frame.line(if view.cursor == index {
            label(&line).to_string()
        } else {
            line
        });
    }
    if view.rows.is_empty() {
        frame.line("No matches");
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
    frame.line(status);
    if let Some(query) = view.query() {
        let line = search_line(query, frame.width());
        let cursor = measure_text_width(&line);
        if view.editing() {
            frame.input(line, cursor);
        } else {
            frame.line(line);
        }
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
