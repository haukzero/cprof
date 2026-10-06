use std::ops::Range;

use super::TreeStyle;

/// Empty branches are visible but cannot select anything.
pub(crate) enum Item<T> {
    Leaf(String, T),
    Branch(String, Vec<Item<T>>),
}

impl<T> Item<T> {
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Leaf(_, _) => false,
            Self::Branch(_, children) => children.iter().all(Self::is_empty),
        }
    }
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
    pub(super) path: String,
    leaves: Range<usize>,
}

/// Branch states are derived from their leaves, so toggling a child cannot
/// leave an ancestor checked when only part of its subtree is selected.
pub(super) struct Selection<T> {
    pub(super) rows: Vec<Row>,
    checked: Vec<bool>,
    values: Vec<T>,
}

impl<T> Selection<T> {
    pub(super) fn new(root: Item<T>, style: TreeStyle) -> Self {
        let mut selection = Self {
            rows: Vec::new(),
            checked: Vec::new(),
            values: Vec::new(),
        };
        selection.append(root, "", "", "", style);
        selection
    }

    fn append(
        &mut self,
        item: Item<T>,
        parent: &str,
        prefix: &str,
        children_prefix: &str,
        style: TreeStyle,
    ) {
        let start = self.checked.len();
        let row = self.rows.len();
        let (label, children) = match item {
            Item::Leaf(label, value) => {
                self.checked.push(false);
                self.values.push(value);
                (label, Vec::new())
            }
            Item::Branch(label, children) => (label, children),
        };
        let path = if parent.is_empty() {
            label.clone()
        } else {
            format!("{parent}/{label}")
        };
        self.rows.push(Row {
            prefix: prefix.to_string(),
            label,
            path: path.clone(),
            leaves: start..start,
        });
        let count = children.len();
        for (index, child) in children.into_iter().enumerate() {
            let last = index + 1 == count;
            self.append(
                child,
                // The root is a grouping label, not part of descendant paths.
                if row == 0 { "" } else { &path },
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
        self.toggle_rows(&[row]);
    }

    /// Apply one state to every matching subtree, including overlapping rows.
    pub(super) fn toggle_rows(&mut self, rows: &[usize]) {
        let checked = rows
            .iter()
            .any(|&row| matches!(self.check(row), Check::None | Check::Partial));
        for &row in rows {
            self.checked[self.rows[row].leaves.clone()].fill(checked);
        }
    }

    pub(super) fn selected(&self) -> Vec<usize> {
        self.checked
            .iter()
            .enumerate()
            .filter_map(|(index, &checked)| checked.then_some(index))
            .collect()
    }

    pub(super) fn into_selected(self) -> Vec<T> {
        self.values
            .into_iter()
            .zip(self.checked)
            .filter_map(|(value, checked)| checked.then_some(value))
            .collect()
    }
}
