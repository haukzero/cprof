use dialoguer::console::Key;

use super::state::Row;

#[derive(Default)]
struct Search {
    query: String,
    editing: bool,
}

/// Visible row indices keep filtering and navigation independent of selection.
pub(super) struct View {
    pub(super) rows: Vec<usize>,
    pub(super) cursor: usize,
    search: Option<Search>,
}

impl View {
    pub(super) fn new(rows: &[Row]) -> Self {
        Self {
            rows: (0..rows.len()).collect(),
            cursor: 0,
            search: None,
        }
    }

    pub(super) fn current(&self) -> Option<usize> {
        self.rows.get(self.cursor).copied()
    }

    pub(super) fn query(&self) -> Option<&str> {
        self.search.as_ref().map(|search| search.query.as_str())
    }

    pub(super) fn editing(&self) -> bool {
        self.search.as_ref().is_some_and(|search| search.editing)
    }

    /// Consume search and navigation keys; return selection or cancellation keys.
    pub(super) fn handle(&mut self, key: Key, rows: &[Row]) -> Option<Key> {
        if let Some(search) = &mut self.search
            && search.editing
        {
            match key {
                Key::Char(ch) if !ch.is_control() => search.query.push(ch),
                Key::Backspace => {
                    search.query.pop();
                }
                Key::Enter => {
                    search.editing = false;
                    return None;
                }
                _ => return self.navigate(key, rows),
            }
            self.filter(rows);
            return None;
        }
        self.navigate(key, rows)
    }

    fn navigate(&mut self, key: Key, rows: &[Row]) -> Option<Key> {
        match key {
            Key::Char('/') => {
                self.search = Some(Search {
                    editing: true,
                    ..Search::default()
                });
                self.filter(rows);
            }
            Key::Escape if self.search.is_some() => {
                let current = self.current().unwrap_or(0);
                self.search = None;
                self.rows = (0..rows.len()).collect();
                self.cursor = current;
            }
            Key::ArrowDown | Key::Tab | Key::Char('j') if !self.rows.is_empty() => {
                self.cursor = (self.cursor + 1) % self.rows.len();
            }
            Key::ArrowUp | Key::BackTab | Key::Char('k') if !self.rows.is_empty() => {
                self.cursor = (self.cursor + self.rows.len() - 1) % self.rows.len();
            }
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.rows.len().saturating_sub(1),
            _ => return Some(key),
        }
        None
    }

    fn filter(&mut self, rows: &[Row]) {
        let query = self.query().unwrap_or_default().to_lowercase();
        self.rows = rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| row.path.to_lowercase().contains(&query).then_some(index))
            .collect();
        self.cursor = 0;
    }
}
