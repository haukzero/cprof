use std::ops::Range;

use super::TreeStyle;

/// Empty branches are visible but cannot select anything.
pub(crate) enum Item {
    Leaf(String),
    Branch(String, Vec<Item>),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Check {
    Empty,
    None,
    Partial,
    All,
}

pub(super) struct Row {
    pub(super) prefix: String,
    pub(super) label: String,
    leaves: Range<usize>,
}

/// Branch states are derived from their leaves, so toggling a child cannot
/// leave an ancestor checked when only part of its subtree is selected.
pub(super) struct Selection {
    pub(super) rows: Vec<Row>,
    checked: Vec<bool>,
}

impl Selection {
    pub(super) fn new(root: Item, style: TreeStyle) -> Self {
        let mut selection = Self {
            rows: Vec::new(),
            checked: Vec::new(),
        };
        selection.append(root, "", "", style);
        selection
    }

    fn append(&mut self, item: Item, prefix: &str, children_prefix: &str, style: TreeStyle) {
        let start = self.checked.len();
        let row = self.rows.len();
        let (label, children) = match item {
            Item::Leaf(label) => {
                self.checked.push(false);
                (label, Vec::new())
            }
            Item::Branch(label, children) => (label, children),
        };
        self.rows.push(Row {
            prefix: prefix.to_string(),
            label,
            leaves: start..start,
        });
        let count = children.len();
        for (index, child) in children.into_iter().enumerate() {
            let last = index + 1 == count;
            self.append(
                child,
                &format!("{children_prefix}{}", style.branch(last)),
                &format!("{children_prefix}{}", style.continuation(last)),
                style,
            );
        }
        self.rows[row].leaves = start..self.checked.len();
    }

    pub(super) fn check(&self, row: usize) -> Check {
        let leaves = &self.checked[self.rows[row].leaves.clone()];
        let count = leaves.iter().filter(|&&checked| checked).count();
        match count {
            _ if leaves.is_empty() => Check::Empty,
            0 => Check::None,
            _ if count == leaves.len() => Check::All,
            _ => Check::Partial,
        }
    }

    pub(super) fn toggle(&mut self, row: usize) {
        let checked = self.check(row) != Check::All;
        self.checked[self.rows[row].leaves.clone()].fill(checked);
    }

    pub(super) fn selected(&self) -> Vec<usize> {
        self.checked
            .iter()
            .enumerate()
            .filter_map(|(index, &checked)| checked.then_some(index))
            .collect()
    }
}
