use crate::activation;
use crate::config;
use crate::error::Result;
use crate::profile;
use crate::targets;
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

#[derive(Serialize)]
struct TargetRow {
    name: String,
    #[serde(rename = "type")]
    kind: &'static str,
    profiles: profile::ProfileCounts,
    active: Option<String>,
    store_dir: String,
    #[serde(skip)]
    profiles_display: String,
}

struct TableColumn<T> {
    header: &'static str,
    value: fn(&T) -> &str,
}

impl TargetRow {
    fn new(target: &targets::TargetSpec) -> Result<Self> {
        let kind = if targets::is_builtin(target) {
            "built-in"
        } else {
            "extra"
        };
        let profiles = profile::counts(target)?;
        let store_dir = config::profiles_dir(target)?.display().to_string();
        let active = activation::active_name(target)?;
        Ok(TargetRow {
            name: target.id.to_string(),
            kind,
            profiles_display: profiles.to_string(),
            profiles,
            active,
            store_dir,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> &str {
        self.kind
    }

    fn store_dir(&self) -> &str {
        &self.store_dir
    }

    fn profiles(&self) -> &str {
        &self.profiles_display
    }

    fn active(&self) -> &str {
        self.active.as_deref().unwrap_or("(none)")
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
        header: "profile count",
        value: TargetRow::profiles,
    },
    TableColumn {
        header: "active",
        value: TargetRow::active,
    },
    TableColumn {
        header: "store dir",
        value: TargetRow::store_dir,
    },
];

pub fn run(targets: &targets::TargetRepository, json: bool) -> Result<()> {
    let rows = targets
        .all()
        .iter()
        .map(|target| TargetRow::new(target))
        .collect::<Result<Vec<_>>>()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        print!("{}", render_table(&rows, TARGET_COLUMNS));
    }
    Ok(())
}

fn render_table<T>(rows: &[T], columns: &[TableColumn<T>]) -> String {
    let widths = columns
        .iter()
        .map(|column| {
            rows.iter()
                .map(|row| display_width((column.value)(row)))
                .max()
                .unwrap_or(0)
                .max(display_width(column.header))
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
            output.extend(std::iter::repeat_n(' ', width - display_width(value)));
        }
    }
    output.push('\n');
}

fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
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
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn renders_target_columns() {
        let output = render_table(
            &[TargetRow {
                name: "codex".to_string(),
                kind: "built-in",
                profiles: crate::profile::ProfileCounts {
                    total: 3,
                    incomplete: 1,
                },
                profiles_display: "3 (1 incomplete)".to_string(),
                store_dir: "/tmp/profiles/codex".to_string(),
                active: None,
            }],
            TARGET_COLUMNS,
        );

        let mut lines = output.lines();
        assert_eq!(
            lines
                .next()
                .expect("table should contain a header")
                .split_whitespace()
                .collect::<Vec<_>>(),
            ["name", "type", "profile", "count", "active", "store", "dir"]
        );
        let separator = lines.next().expect("table should contain a separator");
        assert!(separator.contains('-'));
        assert!(
            separator
                .chars()
                .all(|character| character == '-' || character == ' ')
        );
        assert_eq!(
            lines
                .next()
                .expect("table should contain a target row")
                .split_whitespace()
                .collect::<Vec<_>>(),
            [
                "codex",
                "built-in",
                "3",
                "(1",
                "incomplete)",
                "(none)",
                "/tmp/profiles/codex",
            ]
        );
        assert!(
            lines.next().is_none(),
            "table should contain one target row"
        );
    }

    #[test]
    fn aligns_columns_when_values_contain_wide_characters() {
        let output = render_table(
            &[
                TargetRow {
                    name: "中文".to_string(),
                    kind: "extra",
                    profiles: crate::profile::ProfileCounts {
                        total: 1,
                        incomplete: 0,
                    },
                    profiles_display: "1 (0 incomplete)".to_string(),
                    active: Some("配置".to_string()),
                    store_dir: "/tmp/profiles/chinese".to_string(),
                },
                TargetRow {
                    name: "ascii".to_string(),
                    kind: "extra",
                    profiles: crate::profile::ProfileCounts {
                        total: 1,
                        incomplete: 0,
                    },
                    profiles_display: "1 (0 incomplete)".to_string(),
                    active: Some("active".to_string()),
                    store_dir: "/tmp/profiles/ascii".to_string(),
                },
            ],
            TARGET_COLUMNS,
        );

        let store_dir_columns = output
            .lines()
            .skip(2)
            .map(|line| {
                let byte_index = line
                    .find("/tmp/profiles/")
                    .expect("row should contain path");
                UnicodeWidthStr::width(&line[..byte_index])
            })
            .collect::<Vec<_>>();
        assert_eq!(store_dir_columns.len(), 2);
        assert_eq!(store_dir_columns[0], store_dir_columns[1]);
    }
}
