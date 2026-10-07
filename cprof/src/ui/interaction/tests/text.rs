use console::measure_text_width;

use super::super::{Key, KeyEvent, KeyModifiers, input::Text};

fn type_text(input: &mut Text, text: &str) {
    for ch in text.chars() {
        assert!(input.handle(Key::Char(ch).into()));
    }
}

#[test]
fn text_editing_preserves_unicode_and_literal_q() {
    let mut input = Text::default();
    type_text(&mut input, "工作q");
    for key in [
        Key::Left,
        Key::Backspace,
        Key::Char('具'),
        Key::Home,
        Key::Delete,
        Key::Char('工'),
        Key::Right,
        Key::Delete,
        Key::End,
        Key::Char('q'),
    ] {
        input.handle(key.into());
    }
    assert_eq!(input.value(), "工具q");
}

#[test]
fn empty_input_and_control_keys_do_not_insert_text() {
    let mut input = Text::default();
    for key in [
        Key::Enter,
        Key::Backspace,
        Key::Delete,
        Key::Left,
        Key::Right,
        Key::Home,
        Key::End,
        Key::Char('\0'),
        Key::Char('\n'),
        Key::Tab,
    ] {
        assert!(!input.handle(key.into()));
    }
    assert_eq!(input.view(10), (String::new(), 0));
    type_text(&mut input, "q");
    assert_eq!(input.value(), "q");
}

#[test]
fn word_navigation_keeps_edits_at_character_boundaries() {
    let mut input = Text::default();
    type_text(&mut input, "工作 personal");
    input.handle(KeyEvent::new(Key::Char('b'), KeyModifiers::ALT));
    type_text(&mut input, "q");
    input.handle(Key::Home.into());
    input.handle(KeyEvent::new(Key::Char('f'), KeyModifiers::ALT));
    input.handle(Key::Delete.into());
    assert_eq!(input.value(), "工作 personal");
}

#[test]
fn long_input_scrolls_without_wrapping_or_losing_text() {
    let mut input = Text::default();
    let text = "工作-profile-q";
    type_text(&mut input, text);
    for width in 0..20 {
        input.handle(Key::End.into());
        for _ in 0..=text.chars().count() {
            let (visible, cursor) = input.view(width);
            assert!(
                measure_text_width(&visible) <= width,
                "{visible:?}, {width}"
            );
            assert!(cursor < width.max(1), "{cursor}, {width}");
            assert!(cursor <= measure_text_width(&visible));
            input.handle(Key::Left.into());
        }
    }
    assert_eq!(input.value(), text);
}
