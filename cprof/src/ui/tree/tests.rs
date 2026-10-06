use dialoguer::console::{Key, measure_text_width};

use super::TreeStyle;
use super::screen::search_line;
use super::state::{Check, Item, Selection};
use super::view::View;

fn tree() -> Selection<usize> {
    tree_with_style(TreeStyle::Unicode)
}

fn tree_with_style(style: TreeStyle) -> Selection<usize> {
    Selection::new(
        Item::Branch(
            "All profiles".into(),
            vec![
                Item::Branch(
                    "claude".into(),
                    vec![
                        Item::Leaf("personal".into(), 0),
                        Item::Leaf("work".into(), 1),
                    ],
                ),
                Item::Branch("codex".into(), vec![Item::Leaf("work".into(), 2)]),
                Item::Branch("empty".into(), vec![]),
            ],
        ),
        style,
    )
}

#[test]
fn parent_toggle_selects_and_clears_only_its_descendants() {
    let mut tree = tree();
    tree.toggle(1);
    assert_eq!(tree.selected(), [0, 1]);
    assert_eq!(tree.check(0), Check::Partial);
    assert_eq!(tree.check(1), Check::All);
    assert_eq!(tree.check(2), Check::All);
    assert_eq!(tree.check(3), Check::All);
    assert_eq!(tree.check(4), Check::None);
    tree.toggle(1);
    assert!(tree.selected().is_empty());
    assert_eq!(tree.check(0), Check::None);
}

#[test]
fn excluding_a_leaf_updates_all_ancestors_and_can_be_reversed() {
    let mut tree = tree();
    tree.toggle(0);
    assert_eq!(tree.selected(), [0, 1, 2]);
    assert_eq!(tree.check(0), Check::All);
    tree.toggle(3);
    assert_eq!(tree.selected(), [0, 2]);
    assert_eq!(tree.check(0), Check::Partial);
    assert_eq!(tree.check(1), Check::Partial);
    assert_eq!(tree.check(4), Check::All);
    tree.toggle(1);
    assert_eq!(tree.check(0), Check::All);
    assert_eq!(tree.selected(), [0, 1, 2]);
    tree.toggle(0);
    assert!(tree.selected().is_empty());
}

#[test]
fn selecting_individual_siblings_checks_their_parent() {
    let mut tree = tree();
    tree.toggle(2);
    tree.toggle(3);
    assert_eq!(tree.check(1), Check::All);
    assert_eq!(tree.check(0), Check::Partial);
    tree.toggle(5);
    assert_eq!(tree.check(0), Check::All);
}

#[test]
fn empty_branches_never_become_selected_leaves() {
    let mut tree = tree();
    tree.toggle(6);
    assert_eq!(tree.check(6), Check::Empty);
    assert!(tree.selected().is_empty());
    tree.toggle(0);
    assert_eq!(tree.check(6), Check::Empty);
    assert_eq!(tree.check(0), Check::All);
    assert_eq!(tree.selected(), [0, 1, 2]);
}

#[test]
fn selected_values_keep_their_identity_and_display_order_independent_of_labels() {
    struct Profile(&'static str);

    let mut tree = Selection::new(
        Item::Branch(
            "All profiles".into(),
            vec![
                Item::Branch("empty".into(), vec![]),
                Item::Branch(
                    "claude".into(),
                    vec![
                        Item::Leaf("work (active)".into(), Profile("claude/work")),
                        Item::Branch(
                            "nested".into(),
                            vec![Item::Leaf("other".into(), Profile("claude/other"))],
                        ),
                    ],
                ),
                Item::Branch(
                    "codex".into(),
                    vec![Item::Leaf("work (active)".into(), Profile("codex/work"))],
                ),
            ],
        ),
        TreeStyle::Unicode,
    );
    tree.toggle(6);
    tree.toggle(2);
    tree.toggle(5);

    let selected: Vec<_> = tree
        .into_selected()
        .into_iter()
        .map(|profile| profile.0)
        .collect();
    assert_eq!(selected, ["claude/work", "codex/work"]);
}

#[test]
fn nested_empty_branches_have_no_selectable_values() {
    let empty = Item::<()>::Branch("root".into(), vec![Item::Branch("nested".into(), vec![])]);
    assert!(empty.is_empty());
    assert!(!Item::Branch("root".into(), vec![empty, Item::Leaf("leaf".into(), ())]).is_empty());
}

#[test]
fn tree_connectors_preserve_target_and_profile_hierarchy() {
    for (style, expected) in [
        (
            TreeStyle::Unicode,
            [
                "All profiles",
                "├─ claude",
                "│  ├─ personal",
                "│  └─ work",
                "├─ codex",
                "│  └─ work",
                "└─ empty",
            ],
        ),
        (
            TreeStyle::Ascii,
            [
                "All profiles",
                "+- claude",
                "|  +- personal",
                "|  \\- work",
                "+- codex",
                "|  \\- work",
                "\\- empty",
            ],
        ),
    ] {
        let tree = tree_with_style(style);
        let labels: Vec<_> = tree
            .rows
            .iter()
            .map(|row| format!("{}{}", row.prefix, row.label))
            .collect();
        assert_eq!(labels, expected);
    }
}

fn search<T>(view: &mut View, tree: &Selection<T>, query: &str) {
    assert_eq!(view.handle(Key::Char('/'), &tree.rows), None);
    for ch in query.chars() {
        assert_eq!(view.handle(Key::Char(ch), &tree.rows), None);
    }
}

#[test]
fn search_paths_include_ancestors_but_not_the_grouping_root() {
    let tree = Selection::new(
        Item::Branch(
            "All profiles".into(),
            vec![Item::Branch(
                "target".into(),
                vec![Item::Branch(
                    "nested".into(),
                    vec![Item::Leaf("work (active)".into(), ())],
                )],
            )],
        ),
        TreeStyle::Unicode,
    );
    let paths: Vec<_> = tree.rows.iter().map(|row| row.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "All profiles",
            "target",
            "target/nested",
            "target/nested/work (active)",
        ]
    );
}

#[test]
fn search_matches_targets_names_and_paths_without_case_sensitivity() {
    for style in [TreeStyle::Unicode, TreeStyle::Ascii] {
        let tree = tree_with_style(style);
        for (query, expected) in [
            ("CLAUDE", vec![1, 2, 3]),
            ("work", vec![3, 5]),
            ("codex/WORK", vec![5]),
            ("empty", vec![6]),
        ] {
            let mut view = View::new(&tree.rows);
            search(&mut view, &tree, query);
            assert_eq!(view.rows, expected, "{query}");
        }
    }
}

#[test]
fn matching_a_target_allows_selecting_its_entire_subtree() {
    let mut tree = tree();
    let mut view = View::new(&tree.rows);
    search(&mut view, &tree, "claude");
    assert_eq!(view.handle(Key::Enter, &tree.rows), None);
    assert!(!view.editing());
    assert_eq!(
        view.handle(Key::Char(' '), &tree.rows),
        Some(Key::Char(' '))
    );
    tree.toggle(view.current().unwrap());
    assert_eq!(tree.selected(), [0, 1]);
    assert_eq!(tree.check(1), Check::All);
    assert_eq!(tree.check(4), Check::None);
    assert_eq!(view.handle(Key::Enter, &tree.rows), Some(Key::Enter));
}

#[test]
fn searching_and_clearing_preserve_hidden_selections_and_value_order() {
    let mut tree = tree();
    let mut view = View::new(&tree.rows);
    search(&mut view, &tree, "codex/work");
    tree.toggle(view.current().unwrap());
    view.handle(Key::Enter, &tree.rows);
    search(&mut view, &tree, "claude/personal");
    tree.toggle(view.current().unwrap());
    assert_eq!(view.handle(Key::Escape, &tree.rows), None);
    assert_eq!(view.query(), None);
    assert!(!view.editing());
    assert_eq!(view.rows, (0..tree.rows.len()).collect::<Vec<_>>());
    assert_eq!(view.current(), Some(2));
    assert_eq!(tree.into_selected(), [0, 2]);
}

#[test]
fn toggling_all_matches_preserves_hidden_leaves_and_handles_overlapping_branches() {
    let mut tree = tree();
    tree.toggle(5);
    let mut view = View::new(&tree.rows);
    search(&mut view, &tree, "claude");
    tree.toggle_rows(&view.rows);
    assert_eq!(tree.selected(), [0, 1, 2]);
    tree.toggle_rows(&view.rows);
    assert_eq!(tree.selected(), [2]);

    view.handle(Key::Enter, &tree.rows);
    search(&mut view, &tree, "work");
    tree.toggle_rows(&view.rows);
    assert_eq!(tree.selected(), [1, 2]);
    tree.toggle_rows(&view.rows);
    assert!(tree.selected().is_empty());
}

#[test]
fn empty_branches_do_not_prevent_clearing_all_visible_selections() {
    let mut tree = tree();
    let view = View::new(&tree.rows);
    tree.toggle_rows(&view.rows);
    assert_eq!(tree.selected(), [0, 1, 2]);
    tree.toggle_rows(&view.rows);
    assert!(tree.selected().is_empty());
}

#[test]
fn search_treats_shortcuts_spaces_and_unicode_as_literal_text() {
    let name = "qjak 工作";
    let mut tree = Selection::new(
        Item::Branch(
            "All profiles".into(),
            vec![Item::Branch(
                "target".into(),
                vec![Item::Leaf(name.into(), ())],
            )],
        ),
        TreeStyle::Ascii,
    );
    let mut view = View::new(&tree.rows);
    search(&mut view, &tree, &format!("target/{name}"));
    assert_eq!(view.rows, [2]);
    assert!(view.editing());
    assert!(tree.selected().is_empty());
    view.handle(Key::Backspace, &tree.rows);
    assert_eq!(view.query(), Some("target/qjak 工"));
    assert_eq!(view.rows, [2]);
    assert_eq!(view.handle(Key::CtrlC, &tree.rows), Some(Key::CtrlC));
    view.handle(Key::Enter, &tree.rows);
    assert_eq!(
        view.handle(Key::Char('q'), &tree.rows),
        Some(Key::Char('q'))
    );
    tree.toggle(view.current().unwrap());
    assert_eq!(tree.into_selected(), [()]);
}

#[test]
fn no_matches_allow_navigation_clearing_and_recovery_without_changing_selection() {
    let mut tree = tree();
    tree.toggle(5);
    let mut view = View::new(&tree.rows);
    search(&mut view, &tree, "missing");
    assert!(view.rows.is_empty());
    for key in [
        Key::ArrowDown,
        Key::ArrowUp,
        Key::Tab,
        Key::BackTab,
        Key::Home,
        Key::End,
    ] {
        view.handle(key, &tree.rows);
        assert_eq!(view.current(), None);
    }
    tree.toggle_rows(&view.rows);
    assert_eq!(tree.selected(), [2]);
    for _ in 0..10 {
        view.handle(Key::Backspace, &tree.rows);
    }
    assert_eq!(view.query(), Some(""));
    assert_eq!(view.rows.len(), tree.rows.len());
    view.handle(Key::Char('!'), &tree.rows);
    view.handle(Key::Enter, &tree.rows);
    for key in [Key::Char('j'), Key::Char('k'), Key::Home, Key::End] {
        view.handle(key, &tree.rows);
        assert_eq!(view.current(), None);
    }
    assert_eq!(view.handle(Key::Escape, &tree.rows), None);
    assert_eq!(view.current(), Some(0));
    assert_eq!(view.handle(Key::Escape, &tree.rows), Some(Key::Escape));
    assert_eq!(tree.selected(), [2]);
}

#[test]
fn navigation_uses_filtered_indices_and_wraps_within_results() {
    let tree = tree();
    let mut view = View::new(&tree.rows);
    view.handle(Key::End, &tree.rows);
    search(&mut view, &tree, "work");
    view.handle(Key::Enter, &tree.rows);
    assert_eq!(view.current(), Some(3));
    for (key, expected) in [
        (Key::Char('k'), 5),
        (Key::Char('j'), 3),
        (Key::ArrowDown, 5),
        (Key::Tab, 3),
        (Key::BackTab, 5),
        (Key::ArrowUp, 3),
        (Key::End, 5),
        (Key::Home, 3),
    ] {
        assert_eq!(view.handle(key, &tree.rows), None);
        assert_eq!(view.current(), Some(expected));
    }
    view.handle(Key::Escape, &tree.rows);
    assert_eq!(view.current(), Some(3));
}

#[test]
fn long_search_input_keeps_its_unicode_tail_visible_on_narrow_terminals() {
    let query = "target/personal工作";
    for width in 0..30 {
        let line = search_line(query, width);
        assert!(measure_text_width(&line) <= width);
        if width > 0 {
            assert!(line.starts_with('/'));
            assert!(query.ends_with(&line[1..]));
        }
    }
    assert_eq!(search_line(query, 5), "/工作");
    assert_eq!(search_line("", 1), "/");
}
