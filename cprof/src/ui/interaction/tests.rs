use std::io;

use dialoguer::console::{Key, measure_text_width};

use crate::error::{AppError, InteractionError};

use super::{Prompt, finish, input::Input};

#[test]
fn cancellation_is_distinct_from_a_negative_answer_and_io_failure() {
    assert!(!finish(Ok(Some(false))).unwrap());
    assert!(matches!(
        finish::<bool>(Ok(None)),
        Err(AppError::Interaction(InteractionError::Cancelled))
    ));
    assert!(matches!(
        finish::<bool>(Err(io::Error::other("read failed").into())),
        Err(AppError::Interaction(InteractionError::Prompt(_)))
    ));
}

#[test]
fn extra_cancel_keys_are_local_to_each_prompt() {
    for extra in [&[][..], &[Key::Char('q')][..], &[Key::Char('x')][..]] {
        let prompt = Prompt::new("Choose", extra);
        assert_eq!(prompt.cancel_key(Key::Escape), None);
        assert_eq!(prompt.cancel_key(Key::Enter), Some(Key::Enter));
    }
    let select = Prompt::new("Choose", &[]);
    assert_eq!(select.text, "Choose (Esc cancel)");
    assert_eq!(select.cancel_key(Key::Char('q')), Some(Key::Char('q')));

    let tree = Prompt::new("Choose", &[Key::Char('q'), Key::CtrlC]);
    assert_eq!(tree.text, "Choose (Esc/q/Ctrl-C cancel)");
    assert_eq!(tree.cancel_key(Key::Char('q')), None);
    assert_eq!(tree.cancel_key(Key::CtrlC), None);

    let custom = Prompt::new("Choose", &[Key::Char('x'), Key::Escape, Key::Char('x')]);
    assert_eq!(custom.text, "Choose (Esc/x cancel)");
    assert_eq!(custom.cancel_key(Key::Char('x')), None);
    assert_eq!(custom.cancel_key(Key::Char('q')), Some(Key::Char('q')));
}

fn type_text(input: &mut Input, text: &str) {
    for ch in text.chars() {
        assert_eq!(input.handle(Key::Char(ch)), None);
    }
}

#[test]
fn text_editing_preserves_unicode_and_literal_q() {
    let mut input = Input::default();
    type_text(&mut input, "工作q");
    for key in [
        Key::ArrowLeft,
        Key::Backspace,
        Key::Char('具'),
        Key::Home,
        Key::Del,
        Key::Char('工'),
        Key::ArrowRight,
        Key::Del,
        Key::End,
        Key::Char('q'),
    ] {
        input.handle(key);
    }
    assert_eq!(input.handle(Key::Enter).as_deref(), Some("工具q"));
}

#[test]
fn empty_input_and_control_keys_do_not_submit_or_insert_text() {
    let mut input = Input::default();
    for key in [
        Key::Enter,
        Key::Backspace,
        Key::Del,
        Key::ArrowLeft,
        Key::ArrowRight,
        Key::Home,
        Key::End,
        Key::Char('\0'),
        Key::Char('\n'),
        Key::Tab,
    ] {
        assert_eq!(input.handle(key), None);
    }
    assert_eq!(input.view(10), (String::new(), 0));
    type_text(&mut input, "q");
    assert_eq!(input.handle(Key::Enter).as_deref(), Some("q"));
}

#[test]
fn word_navigation_keeps_edits_at_character_boundaries() {
    let mut input = Input::default();
    type_text(&mut input, "工作 personal");
    input.handle(Key::UnknownEscSeq(vec!['b']));
    type_text(&mut input, "q");
    input.handle(Key::Home);
    input.handle(Key::UnknownEscSeq(vec!['f']));
    input.handle(Key::Del);
    assert_eq!(input.handle(Key::Enter).as_deref(), Some("工作 personal"));
}

#[test]
fn long_input_scrolls_without_wrapping_or_losing_text() {
    let mut input = Input::default();
    let text = "工作-profile-q";
    type_text(&mut input, text);
    for width in 0..20 {
        input.handle(Key::End);
        for _ in 0..=text.chars().count() {
            let (visible, cursor) = input.view(width);
            assert!(
                measure_text_width(&visible) <= width,
                "{visible:?}, {width}"
            );
            assert!(cursor < width.max(1), "{cursor}, {width}");
            assert!(cursor <= measure_text_width(&visible));
            input.handle(Key::ArrowLeft);
        }
    }
    assert_eq!(input.handle(Key::Enter).as_deref(), Some(text));
}
