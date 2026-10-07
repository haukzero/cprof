use std::ops::ControlFlow;

use console::{measure_text_width, truncate_str};

use super::{Component, Frame, Key, KeyEvent, KeyModifiers, key_code};

#[derive(Default)]
pub(super) struct Input(Text);

impl Component for Input {
    type Output = String;

    fn render(&self, frame: &mut Frame) {
        self.0.render(frame, &format!("{}: ", frame.prompt()));
    }

    fn handle(&mut self, key: KeyEvent) -> ControlFlow<String> {
        if key_code(key) == Some(Key::Enter) && !self.0.is_empty() {
            return ControlFlow::Break(self.0.value());
        }
        self.0.handle(key);
        ControlFlow::Continue(())
    }

    fn report(&self, output: &String) -> Option<String> {
        Some(output.clone())
    }
}

/// Shared Unicode editing for the input and searchable selection components.
#[derive(Default)]
pub(super) struct Text {
    chars: Vec<char>,
    cursor: usize,
}

impl Text {
    pub(super) fn handle(&mut self, key: KeyEvent) -> bool {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, Key::Left)
            | (KeyModifiers::ALT, Key::Char('b') | Key::Left) => {
                while self.cursor > 0 && self.chars[self.cursor - 1].is_whitespace() {
                    self.cursor -= 1;
                }
                while self.cursor > 0 && !self.chars[self.cursor - 1].is_whitespace() {
                    self.cursor -= 1;
                }
                return false;
            }
            (KeyModifiers::CONTROL, Key::Right)
            | (KeyModifiers::ALT, Key::Char('f') | Key::Right) => {
                while self.cursor < self.chars.len() && !self.chars[self.cursor].is_whitespace() {
                    self.cursor += 1;
                }
                while self.cursor < self.chars.len() && self.chars[self.cursor].is_whitespace() {
                    self.cursor += 1;
                }
                return false;
            }
            _ => {}
        }
        let Some(key) = key_code(key) else {
            return false;
        };
        let mut changed = false;
        match key {
            Key::Char(ch) if !ch.is_control() => {
                self.chars.insert(self.cursor, ch);
                self.cursor += 1;
                changed = true;
            }
            Key::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
                changed = true;
            }
            Key::Delete if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
                changed = true;
            }
            Key::Left if self.cursor > 0 => self.cursor -= 1,
            Key::Right if self.cursor < self.chars.len() => self.cursor += 1,
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.chars.len(),
            _ => {}
        }
        changed
    }

    pub(super) fn value(&self) -> String {
        self.chars.iter().collect()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Scroll long input horizontally, leaving a cell for the cursor.
    pub(super) fn view(&self, width: usize) -> (String, usize) {
        let before: String = self.chars[..self.cursor].iter().collect();
        let mut before = before.as_str();
        while measure_text_width(before) >= width && !before.is_empty() {
            before = &before[before.chars().next().unwrap().len_utf8()..];
        }
        let cursor = measure_text_width(before);
        let after: String = self.chars[self.cursor..].iter().collect();
        let visible = format!(
            "{before}{}",
            truncate_str(&after, width.saturating_sub(cursor), "")
        );
        (visible, cursor)
    }
    pub(super) fn render(&self, frame: &mut Frame, prefix: &str) {
        let prefix = truncate_str(prefix, frame.width().saturating_sub(1), "");
        let prefix_width = measure_text_width(&prefix);
        let (value, cursor) = self.view(frame.width().saturating_sub(prefix_width));
        frame.input(format!("{prefix}{value}"), prefix_width + cursor);
    }
}
