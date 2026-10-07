use std::ops::ControlFlow;

use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};

use super::{Component, Frame, Key, KeyEvent, input::Text, key_code};

/// Matches retain original indices, even when labels are equal or reordered.
pub(super) struct Select<'a> {
    items: &'a [String],
    query: Text,
    matches: Vec<usize>,
    selected: Option<usize>,
    matcher: SkimMatcherV2,
}

impl<'a> Select<'a> {
    pub(super) fn new(items: &'a [String]) -> Self {
        Self {
            items,
            query: Text::default(),
            matches: (0..items.len()).collect(),
            selected: None,
            matcher: SkimMatcherV2::default(),
        }
    }

    fn filter(&mut self) {
        let query = self.query.value();
        let mut matches: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                self.matcher
                    .fuzzy_match(item, &query)
                    .map(|score| (index, score))
            })
            .collect();
        matches.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        self.matches = matches.into_iter().map(|(index, _)| index).collect();
        self.selected = (!self.matches.is_empty()).then_some(0);
    }

    fn highlighted(&self, index: usize, query: &str) -> String {
        let text = &self.items[index];
        let Some((_, indices)) = self.matcher.fuzzy_indices(text, query) else {
            return text.clone();
        };
        let mut rendered = String::new();
        for (index, ch) in text.chars().enumerate() {
            if indices.contains(&index) {
                rendered.push_str(&console::style(ch).for_stderr().bold().to_string());
            } else {
                rendered.push(ch);
            }
        }
        rendered
    }
}

impl Component for Select<'_> {
    type Output = usize;

    fn render(&self, frame: &mut Frame) {
        self.query.render(frame, &format!("{}: ", frame.prompt()));
        let query = self.query.value();
        let capacity = frame.height().saturating_sub(1).max(1);
        let start = self.selected.unwrap_or(0) / capacity * capacity;
        for (row, &index) in self.matches.iter().enumerate().skip(start).take(capacity) {
            let line = format!(
                "{} {}",
                if self.selected == Some(row) { ">" } else { " " },
                self.highlighted(index, &query)
            );
            frame.line(line);
        }
        if self.matches.is_empty() {
            frame.line("No matches");
        }
    }

    fn handle(&mut self, key: KeyEvent) -> ControlFlow<usize> {
        let count = self.matches.len();
        match key_code(key) {
            Some(Key::Down | Key::Tab) if count > 0 => {
                self.selected = Some(self.selected.map_or(0, |row| (row + 1) % count));
            }
            Some(Key::Up | Key::BackTab) if count > 0 => {
                self.selected = Some(
                    self.selected
                        .map_or(count - 1, |row| (row + count - 1) % count),
                );
            }
            Some(Key::Enter) => {
                if let Some(row) = self.selected {
                    return ControlFlow::Break(self.matches[row]);
                }
            }
            _ => {
                if self.query.handle(key) {
                    self.filter();
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn report(&self, output: &usize) -> Option<String> {
        Some(self.items[*output].clone())
    }
}
