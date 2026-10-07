use std::ops::ControlFlow;

use super::super::{Component, Frame, Key, KeyEvent, KeyModifiers, select::Select};
use super::{CTRL_C, TestTerminal, assert_cancelled, run_on};

#[test]
fn fuzzy_search_returns_original_indices_and_accepts_q_as_text() {
    let items = ["alpha".into(), "quick-work".into(), "beta".into()];
    let mut terminal = TestTerminal::keys([Key::Char('q'), Key::Char('w'), Key::Enter]);
    assert_eq!(
        run_on("Choose", &mut Select::new(&items), &mut terminal).unwrap(),
        1
    );
    assert_eq!(terminal.reports, ["Choose: quick-work"]);
}

#[test]
fn duplicate_labels_keep_distinct_identity() {
    let items = ["same".into(), "same".into()];
    let mut terminal = TestTerminal::keys([Key::Down, Key::Down, Key::Enter]);
    assert_eq!(
        run_on("Choose", &mut Select::new(&items), &mut terminal).unwrap(),
        1
    );
}

#[test]
fn no_matches_can_be_edited_back_to_a_valid_selection() {
    let items = ["alpha".into(), "工作".into()];
    let mut terminal = TestTerminal::keys([
        Key::Char('!'),
        Key::Up,
        Key::Enter,
        Key::Backspace,
        Key::Char('工'),
        Key::Enter,
    ]);
    assert_eq!(
        run_on("Choose", &mut Select::new(&items), &mut terminal).unwrap(),
        1
    );
    assert!(
        terminal
            .frames
            .iter()
            .any(|frame| frame.iter().any(|line| line == "No matches"))
    );
}

#[test]
fn cancelling_an_empty_or_filtered_list_uses_the_runner() {
    for items in [vec![], vec!["alpha".into()]] {
        for key in [CTRL_C, Key::Esc.into()] {
            let mut terminal = TestTerminal::keys([Key::Char('!').into(), key]);
            assert_cancelled(run_on("Choose", &mut Select::new(&items), &mut terminal));
            assert!(terminal.closed);
        }
    }
}

#[test]
fn navigation_wraps_and_selected_items_remain_visible_on_small_screens() {
    let items: Vec<_> = (0..30).map(|index| format!("item-{index}")).collect();
    let mut select = Select::new(&items);
    assert_eq!(select.handle(Key::Enter.into()), ControlFlow::Continue(()));
    assert_eq!(select.handle(Key::Up.into()), ControlFlow::Continue(()));
    assert_eq!(select.handle(Key::Enter.into()), ControlFlow::Break(29));
    let mut frame = Frame::new("Choose".into(), (30, 5));
    select.render(&mut frame);
    assert!(frame.lines.iter().any(|line| line.contains("item-29")));
    assert_eq!(select.handle(Key::Down.into()), ControlFlow::Continue(()));
    assert_eq!(select.handle(Key::Enter.into()), ControlFlow::Break(0));
    for key in [
        Key::BackTab.into(),
        KeyEvent::new(Key::Tab, KeyModifiers::SHIFT),
    ] {
        let mut terminal = TestTerminal::keys([key, Key::Enter.into()]);
        assert_eq!(
            run_on("Choose", &mut Select::new(&items), &mut terminal).unwrap(),
            29
        );
    }
}
