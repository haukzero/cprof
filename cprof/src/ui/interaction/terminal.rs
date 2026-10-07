use std::io::{self, BufWriter, Stderr, Write};

use console::{measure_text_width, truncate_str};
use crossterm::{cursor, event, event::Event, queue, style::Print, terminal};

/// The runner's I/O boundary also allows deterministic failure-path tests.
pub(super) trait Terminal {
    fn size(&self) -> io::Result<(u16, u16)>;
    fn draw(&mut self, frame: &Frame) -> io::Result<()>;
    fn read(&mut self) -> io::Result<Event>;
    fn close(&mut self) -> io::Result<()>;
    fn report(&mut self, text: &str) -> io::Result<()>;
}

/// Own raw mode for the entire interaction, including rendering between reads.
/// All cursor movement and prompt erasure live here; components only build frames.
pub(super) struct Session {
    out: BufWriter<Stderr>,
    rows: usize,
    cursor_row: usize,
    active: bool,
}

impl Session {
    pub(super) fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self {
            out: BufWriter::new(io::stderr()),
            rows: 0,
            cursor_row: 0,
            active: true,
        })
    }

    fn clear(&mut self) -> io::Result<()> {
        if self.rows == 0 {
            return Ok(());
        }
        queue!(self.out, cursor::MoveToColumn(0))?;
        if self.cursor_row > 0 {
            queue!(self.out, cursor::MoveUp(self.cursor_row as u16))?;
        }
        for row in 0..self.rows {
            if row > 0 {
                queue!(self.out, cursor::MoveDown(1))?;
            }
            queue!(self.out, terminal::Clear(terminal::ClearType::CurrentLine))?;
        }
        if self.rows > 1 {
            queue!(self.out, cursor::MoveUp((self.rows - 1) as u16))?;
        }
        self.rows = 0;
        self.cursor_row = 0;
        Ok(())
    }
}

impl Terminal for Session {
    fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    fn draw(&mut self, frame: &Frame) -> io::Result<()> {
        queue!(self.out, cursor::Hide)?;
        self.clear()?;
        queue!(self.out, cursor::MoveToColumn(0))?;
        for (row, line) in frame.lines.iter().enumerate() {
            if row > 0 {
                // Raw mode does not translate LF into CRLF.
                queue!(self.out, Print("\r\n"))?;
            }
            self.rows = row + 1;
            self.cursor_row = row;
            queue!(self.out, Print(line))?;
        }
        if let Some((row, column)) = frame.cursor {
            if self.cursor_row > row {
                queue!(self.out, cursor::MoveUp((self.cursor_row - row) as u16))?;
            }
            self.cursor_row = row;
            queue!(self.out, cursor::MoveToColumn(column as u16), cursor::Show)?;
        }
        self.out.flush()
    }

    fn read(&mut self) -> io::Result<Event> {
        event::read()
    }

    fn close(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        // Attempt every restoration step even if an earlier write failed.
        let clear = self.clear();
        let show = queue!(self.out, cursor::Show);
        let flush = self.out.flush();
        let raw = terminal::disable_raw_mode();
        let result = clear.and(show).and(flush).and(raw);
        self.active = result.is_err();
        result
    }

    fn report(&mut self, text: &str) -> io::Result<()> {
        writeln!(self.out, "{text}")?;
        self.out.flush()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// A bounded drawing description. Components cannot write terminal commands.
pub(crate) struct Frame {
    prompt: String,
    width: usize,
    height: usize,
    pub(super) lines: Vec<String>,
    pub(super) cursor: Option<(usize, usize)>,
}

impl Frame {
    pub(super) fn new(prompt: String, (width, height): (u16, u16)) -> Self {
        Self {
            prompt,
            // Leave a column to prevent automatic wrapping and a row for the shell.
            width: usize::from(width).saturating_sub(1),
            height: usize::from(height).saturating_sub(1).max(1),
            lines: Vec::new(),
            cursor: None,
        }
    }

    pub(crate) fn prompt(&self) -> &str {
        &self.prompt
    }
    pub(crate) fn width(&self) -> usize {
        self.width
    }
    pub(crate) fn height(&self) -> usize {
        self.height
    }

    pub(crate) fn line(&mut self, line: impl AsRef<str>) {
        if self.lines.len() < self.height {
            // Embedded newlines/tabs must not escape the accounted drawing area.
            let line = line.as_ref().replace(['\r', '\n', '\t'], " ");
            self.lines
                .push(truncate_str(&line, self.width, "").into_owned());
        }
    }

    /// Append an editing line and its cursor together, or omit both if it cannot fit.
    pub(crate) fn input(&mut self, line: impl AsRef<str>, column: usize) {
        if self.lines.len() < self.height {
            self.line(line);
            let row = self.lines.len() - 1;
            self.cursor = Some((row, column.min(measure_text_width(&self.lines[row]))));
        }
    }
}
