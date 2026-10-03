use std::io;

use dialoguer::console::{Key, Term, measure_text_width, truncate_str};

use super::Prompt;

pub(super) fn interact(prompt: &Prompt) -> io::Result<Option<String>> {
    let line = Line(Term::buffered_stderr());
    let mut input = Input::default();
    loop {
        line.render(&prompt.text, &input)?;
        let Some(key) = prompt.read_key(&line.0)? else {
            return Ok(None);
        };
        if let Some(value) = input.handle(key) {
            line.0.clear_line()?;
            line.0.write_line(&format!("{}: {value}", prompt.text))?;
            return Ok(Some(value));
        }
    }
}

#[derive(Default)]
pub(super) struct Input {
    chars: Vec<char>,
    cursor: usize,
}

impl Input {
    pub(super) fn handle(&mut self, key: Key) -> Option<String> {
        match key {
            Key::Char(ch) if !ch.is_control() => {
                self.chars.insert(self.cursor, ch);
                self.cursor += 1;
            }
            Key::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
            }
            Key::Del if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
            }
            Key::ArrowLeft if self.cursor > 0 => self.cursor -= 1,
            Key::ArrowRight if self.cursor < self.chars.len() => self.cursor += 1,
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.chars.len(),
            Key::UnknownEscSeq(seq) if seq == ['b'] => {
                while self.cursor > 0 && self.chars[self.cursor - 1].is_whitespace() {
                    self.cursor -= 1;
                }
                while self.cursor > 0 && !self.chars[self.cursor - 1].is_whitespace() {
                    self.cursor -= 1;
                }
            }
            Key::UnknownEscSeq(seq) if seq == ['f'] => {
                while self.cursor < self.chars.len() && !self.chars[self.cursor].is_whitespace() {
                    self.cursor += 1;
                }
                while self.cursor < self.chars.len() && self.chars[self.cursor].is_whitespace() {
                    self.cursor += 1;
                }
            }
            Key::Enter if !self.chars.is_empty() => return Some(self.chars.iter().collect()),
            _ => {}
        }
        None
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
}

/// Keep input on one terminal line so cancellation and I/O errors can erase it.
struct Line(Term);

impl Line {
    fn render(&self, prompt: &str, input: &Input) -> io::Result<()> {
        let width = usize::from(self.0.size().1).saturating_sub(1);
        let prompt = format!("{prompt}: ");
        let prompt = truncate_str(&prompt, width.saturating_sub(1), "");
        let (value, cursor) = input.view(width.saturating_sub(measure_text_width(&prompt)));
        self.0.clear_line()?;
        self.0.write_str(&format!("{prompt}{value}"))?;
        self.0
            .move_cursor_left(measure_text_width(&value).saturating_sub(cursor))?;
        self.0.flush()
    }
}

impl Drop for Line {
    fn drop(&mut self) {
        let _ = self.0.clear_line();
        let _ = self.0.flush();
    }
}
