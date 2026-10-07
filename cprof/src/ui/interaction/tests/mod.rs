use std::collections::VecDeque;
use std::io;
use std::ops::ControlFlow;

use crossterm::event::{Event, KeyEvent, KeyEventKind, KeyModifiers};

use crate::error::{AppError, InteractionError};

use super::{
    Component, Frame, Key, confirm::Confirm, input::Input, is_cancel, prompt_text, run_on,
    terminal::Terminal,
};

mod select;
mod text;

const CTRL_C: KeyEvent = KeyEvent::new(Key::Char('c'), KeyModifiers::CONTROL);

#[derive(Default)]
struct TestTerminal {
    events: VecDeque<io::Result<Event>>,
    frames: Vec<Vec<String>>,
    reports: Vec<String>,
    closed: bool,
    fail: Option<&'static str>,
}

impl TestTerminal {
    fn keys<K: Into<KeyEvent>>(keys: impl IntoIterator<Item = K>) -> Self {
        Self {
            events: keys
                .into_iter()
                .map(|key| Ok(Event::Key(key.into())))
                .collect(),
            ..Self::default()
        }
    }

    fn check(&self, operation: &str) -> io::Result<()> {
        if self.fail == Some(operation) {
            Err(io::Error::other(format!("{operation} failed")))
        } else {
            Ok(())
        }
    }
}

impl Terminal for TestTerminal {
    fn size(&self) -> io::Result<(u16, u16)> {
        self.check("size")?;
        Ok((100, 24))
    }
    fn draw(&mut self, frame: &Frame) -> io::Result<()> {
        self.check("draw")?;
        self.frames.push(frame.lines.clone());
        Ok(())
    }
    fn read(&mut self) -> io::Result<Event> {
        self.check("read")?;
        self.events.pop_front().expect("component did not finish")
    }
    fn close(&mut self) -> io::Result<()> {
        self.closed = true;
        self.check("close")
    }
    fn report(&mut self, text: &str) -> io::Result<()> {
        assert!(
            self.closed,
            "reports must be written after terminal restoration"
        );
        self.check("report")?;
        self.reports.push(text.into());
        Ok(())
    }
}

fn assert_cancelled<T: std::fmt::Debug>(result: crate::error::Result<T>) {
    assert!(
        matches!(
            result,
            Err(AppError::Interaction(InteractionError::Cancelled))
        ),
        "{result:?}"
    );
}

#[test]
fn cancellation_is_distinct_from_a_negative_answer() {
    for key in [Key::Esc.into(), Key::Char('q').into(), CTRL_C] {
        let mut terminal = TestTerminal::keys([key]);
        assert_cancelled(run_on("Continue?", &mut Confirm, &mut terminal));
        assert!(terminal.closed);
        assert!(terminal.reports.is_empty());
    }
    for (key, answer) in [
        (Key::Char('y'), true),
        (Key::Char('Y'), true),
        (Key::Char('n'), false),
        (Key::Char('N'), false),
        (Key::Enter, false),
    ] {
        let mut terminal = TestTerminal::keys([key]);
        assert_eq!(
            run_on("Continue?", &mut Confirm, &mut terminal).unwrap(),
            answer
        );
        assert!(terminal.closed);
        assert_eq!(terminal.reports.len(), 1);
    }
}

#[test]
fn every_fallible_runner_stage_restores_the_terminal() {
    for stage in ["size", "draw", "read", "close", "report"] {
        let mut terminal = TestTerminal::keys([Key::Enter]);
        terminal.fail = Some(stage);
        let result = run_on("Continue?", &mut Confirm, &mut terminal);
        assert!(
            matches!(
                result,
                Err(AppError::Interaction(InteractionError::Prompt(_)))
            ),
            "{stage}: {result:?}"
        );
        assert!(terminal.closed, "{stage}");
    }
}

#[test]
fn cleanup_failure_does_not_replace_cancellation_or_read_failure() {
    let mut terminal = TestTerminal::keys([CTRL_C]);
    terminal.fail = Some("close");
    assert_cancelled(run_on("Continue?", &mut Confirm, &mut terminal));
    terminal
        .events
        .push_back(Err(io::Error::other("original read error")));
    let error = run_on("Continue?", &mut Confirm, &mut terminal).unwrap_err();
    assert!(error.to_string().contains("original read error"));
}

#[test]
fn input_ignores_empty_submission_and_keeps_q_as_text() {
    let mut terminal =
        TestTerminal::keys([Key::Enter, Key::Char('q'), Key::Char('工'), Key::Enter]);
    assert_eq!(
        run_on("Name", &mut Input::default(), &mut terminal).unwrap(),
        "q工"
    );
    assert!(terminal.closed);
    assert_eq!(terminal.reports, ["Name: q工"]);
}

#[test]
fn policies_and_hints_follow_component_state_and_always_reserve_ctrl_c() {
    struct Changing(bool);
    impl Component for Changing {
        type Output = ();
        fn cancel_keys(&self) -> &[Key] {
            if self.0 {
                &[]
            } else {
                &[Key::Esc, Key::Char('q')]
            }
        }
        fn render(&self, frame: &mut Frame) {
            let prompt = frame.prompt().to_owned();
            frame.line(prompt);
        }
        fn handle(&mut self, key: KeyEvent) -> ControlFlow<()> {
            assert_ne!(key, CTRL_C, "runner must intercept reserved keys");
            if key.code == Key::Char('/') {
                self.0 = true;
            }
            ControlFlow::Continue(())
        }
    }
    let mut terminal = TestTerminal::keys([
        Key::Char('/').into(),
        Key::Char('q').into(),
        Key::Esc.into(),
        CTRL_C,
    ]);
    assert_cancelled(run_on("Choose", &mut Changing(false), &mut terminal));
    assert_eq!(terminal.frames[0], ["Choose (Esc/q/Ctrl-C cancel)"]);
    for frame in &terminal.frames[1..] {
        assert_eq!(frame, &["Choose (Ctrl-C cancel)"]);
    }
    assert!(terminal.closed);
}

#[test]
fn resize_redraws_without_delivering_a_key_to_the_component() {
    let mut terminal = TestTerminal::keys([CTRL_C]);
    terminal.events.push_front(Ok(Event::Resize(100, 24)));
    assert_cancelled(run_on("Continue?", &mut Confirm, &mut terminal));
    assert_eq!(terminal.frames.len(), 2);
}

#[test]
fn release_events_and_modified_answer_keys_do_not_submit_or_cancel() {
    let mut terminal = TestTerminal::keys([
        KeyEvent::new_with_kind(Key::Char('c'), KeyModifiers::CONTROL, KeyEventKind::Release),
        KeyEvent::new(Key::Char('y'), KeyModifiers::CONTROL),
        KeyEvent::new(Key::Char('q'), KeyModifiers::ALT),
        KeyEvent::new_with_kind(Key::Enter, KeyModifiers::NONE, KeyEventKind::Release),
        Key::Enter.into(),
    ]);
    assert!(!run_on("Continue?", &mut Confirm, &mut terminal).unwrap());
    assert!(terminal.closed);
    for key in [
        CTRL_C,
        KeyEvent::new(Key::Char('C'), KeyModifiers::CONTROL | KeyModifiers::SHIFT),
    ] {
        let mut terminal = TestTerminal::keys([key]);
        assert_cancelled(run_on("Name", &mut Input::default(), &mut terminal));
        assert!(terminal.closed);
    }
}

#[test]
fn frames_clip_unicode_without_leaking_a_cursor_from_an_omitted_input_line() {
    for width in 0..12 {
        let mut frame = Frame::new(String::new(), (width, 2));
        frame.line("工作\nprofile\tname");
        frame.input("hidden", 3);
        assert_eq!(frame.lines.len(), 1);
        assert!(
            console::measure_text_width(&frame.lines[0]) <= usize::from(width).saturating_sub(1)
        );
        assert!(!frame.lines[0].contains(['\n', '\t']));
        assert_eq!(frame.cursor, None);
    }
}

#[test]
fn custom_cancel_keys_can_be_owned_by_a_component_without_duplicate_hints() {
    let keys = vec![Key::Char('x'), Key::Esc, Key::Char('x')];
    assert!(is_cancel(Key::Char('x').into(), &keys));
    assert!(!is_cancel(Key::Char('q').into(), &keys));
    assert!(is_cancel(CTRL_C, &keys));
    assert_eq!(prompt_text("Choose", &keys), "Choose (x/Esc/Ctrl-C cancel)");
}
