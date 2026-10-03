use super::TreeStyle;
use super::state::{Check, Item, Selection};

fn tree() -> Selection {
    tree_with_style(TreeStyle::Unicode)
}

fn tree_with_style(style: TreeStyle) -> Selection {
    Selection::new(
        Item::Branch(
            "All profiles".into(),
            vec![
                Item::Branch(
                    "claude".into(),
                    vec![Item::Leaf("personal".into()), Item::Leaf("work".into())],
                ),
                Item::Branch("codex".into(), vec![Item::Leaf("work".into())]),
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
