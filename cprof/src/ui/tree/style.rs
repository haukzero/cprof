/// Only tree connectors vary; labels, checkboxes and key hints stay unchanged.
#[derive(Clone, Copy)]
pub(crate) enum TreeStyle {
    Unicode,
    Ascii,
}

impl TreeStyle {
    pub(super) fn branch(self, last: bool) -> &'static str {
        match (self, last) {
            (Self::Unicode, false) => "├─ ",
            (Self::Unicode, true) => "└─ ",
            (Self::Ascii, false) => "+- ",
            (Self::Ascii, true) => "\\- ",
        }
    }

    pub(super) fn continuation(self, last: bool) -> &'static str {
        match (self, last) {
            (_, true) => "   ",
            (Self::Unicode, false) => "│  ",
            (Self::Ascii, false) => "|  ",
        }
    }
}
