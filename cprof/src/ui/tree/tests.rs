use super::TreeStyle;
use super::state::{Check, Item, Selection};

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
