use crate::config;
use crate::error::Result;
use crate::targets;

struct TargetRow {
    name: String,
    kind: &'static str,
    store_dir: String,
}

struct TableColumn<T> {
    header: &'static str,
    value: fn(&T) -> &str,
}

impl TargetRow {
    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> &str {
        self.kind
    }

    fn store_dir(&self) -> &str {
        &self.store_dir
    }
}

const TARGET_COLUMNS: &[TableColumn<TargetRow>] = &[
    TableColumn {
        header: "name",
        value: TargetRow::name,
    },
    TableColumn {
        header: "type",
        value: TargetRow::kind,
    },
    TableColumn {
        header: "store dir",
        value: TargetRow::store_dir,
    },
];

pub fn run() -> Result<()> {
    let rows = targets::all()?
        .iter()
        .map(|target| {
            Ok(TargetRow {
                name: target.id.to_string(),
                kind: if targets::is_builtin(target) {
                    "built-in"
                } else {
                    "extra"
                },
                store_dir: config::profiles_dir(target)?.display().to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    print!("{}", render_table(&rows, TARGET_COLUMNS));
    Ok(())
}

fn render_table<T>(rows: &[T], columns: &[TableColumn<T>]) -> String {
    let widths = columns
        .iter()
        .map(|column| {
            rows.iter()
                .map(|row| (column.value)(row).len())
                .max()
                .unwrap_or(0)
                .max(column.header.len())
        })
        .collect::<Vec<_>>();
    let row_width = widths.iter().sum::<usize>() + columns.len().saturating_sub(1) * 2 + 1;
    let mut output = String::with_capacity(row_width * (rows.len() + 2));

    render_row(&mut output, columns, &widths, None);
    render_separator(&mut output, &widths);
    for row in rows {
        render_row(&mut output, columns, &widths, Some(row));
    }
    output
}

fn render_row<T>(
    output: &mut String,
    columns: &[TableColumn<T>],
    widths: &[usize],
    row: Option<&T>,
) {
    let last_column = columns.len().saturating_sub(1);
    for (index, (column, width)) in columns.iter().zip(widths).enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        let value = row.map_or(column.header, |row| (column.value)(row));
        output.push_str(value);
        if index < last_column {
            output.extend(std::iter::repeat_n(' ', width - value.len()));
        }
    }
    output.push('\n');
}

fn render_separator(output: &mut String, widths: &[usize]) {
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.extend(std::iter::repeat_n('-', *width));
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::{TARGET_COLUMNS, TargetRow, render_table};

    #[test]
    fn renders_target_columns() {
        let output = render_table(
            &[TargetRow {
                name: "codex".to_string(),
                kind: "built-in",
                store_dir: "/tmp/profiles/codex".to_string(),
            }],
            TARGET_COLUMNS,
        );

        assert!(output.contains("name   type      store dir"));
        assert!(output.contains("codex  built-in  /tmp/profiles/codex"));
    }
}
