//! Consistent human output and structured output with no panic paths.
use crate::cli::OutputFormat;
use mammoth_core::{Error, Result};
use mammoth_viz::style::{self, cell, paint_error, Tone};
use serde::Serialize;
use serde_json::Value;
use std::io::{IsTerminal, Write};
impl OutputFormat {
    pub fn resolve(self) -> Self {
        match self {
            Self::Auto if std::io::stdout().is_terminal() => Self::Table,
            Self::Auto => Self::Json,
            other => other,
        }
    }
}
pub fn emit(value: &impl Serialize, fmt: OutputFormat) -> Result<()> {
    let value = serde_json::to_value(value).map_err(|e| Error::InvalidInput(e.to_string()))?;
    let out = std::io::stdout();
    let mut out = out.lock();
    match fmt.resolve() {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).map_err(|e| Error::InvalidInput(e.to_string()))?
        )?,
        OutputFormat::Yaml => writeln!(
            out,
            "{}",
            serde_yaml::to_string(&value)
                .map_err(|e| Error::InvalidInput(e.to_string()))?
                .trim_end()
        )?,
        OutputFormat::Table | OutputFormat::Csv => {
            let human = fmt.resolve() == OutputFormat::Table;
            // A nested report belongs in a field/value view. Serializing it
            // into one cell made skew and cluster tables hundreds of columns wide.
            let rows = match &value {
                Value::Object(_) if human => {
                    fn flatten(value: &Value, prefix: &str, rows: &mut Vec<Value>) {
                        match value {
                            Value::Object(object) => {
                                for (key, value) in object {
                                    flatten(
                                        value,
                                        &if prefix.is_empty() {
                                            key.clone()
                                        } else {
                                            format!("{prefix}.{key}")
                                        },
                                        rows,
                                    );
                                }
                            }
                            Value::Array(array) if !array.is_empty() => {
                                for (index, value) in array.iter().enumerate() {
                                    flatten(value, &format!("{prefix}[{index}]"), rows);
                                }
                            }
                            _ => rows.push(serde_json::json!({"field":prefix,"value":text(value)})),
                        }
                    }
                    let mut rows = vec![];
                    flatten(&value, "", &mut rows);
                    rows
                }
                Value::Array(v) => v.clone(),
                v => vec![v.clone()],
            };
            let mut headers = std::collections::BTreeSet::new();
            for row in &rows {
                if let Some(o) = row.as_object() {
                    headers.extend(o.keys().cloned());
                }
            }
            let headers: Vec<_> = if headers.is_empty() {
                vec!["value".into()]
            } else {
                headers.into_iter().collect()
            };
            let values: Vec<Vec<String>> = rows
                .iter()
                .map(|r| {
                    headers
                        .iter()
                        .map(|k| {
                            let value = text(if r.is_object() { &r[k] } else { r });
                            if human {
                                style::clean(&value)
                            } else {
                                value
                            }
                        })
                        .collect()
                })
                .collect();
            if fmt.resolve() == OutputFormat::Csv {
                let mut w = csv::Writer::from_writer(&mut out);
                w.write_record(&headers).map_err(csv_error)?;
                for row in values {
                    w.write_record(row).map_err(csv_error)?;
                }
                w.flush()?;
            } else {
                let mut table = style::table();
                table.set_header(headers.iter().map(|header| cell(header, Tone::Heading)));
                for row in values {
                    table.add_row(
                        row.iter()
                            .enumerate()
                            .map(|(index, value)| cell(value, value_tone(value, index))),
                    );
                }
                writeln!(out, "{table}")?;
            }
        }
        OutputFormat::Auto => unreachable!("format resolved"),
    }
    Ok(())
}
fn value_tone(value: &str, column: usize) -> Tone {
    match value.to_ascii_lowercase().as_str() {
        "healthy" | "running" | "succeeded" | "true" => Tone::Ok,
        "dead" | "corrupt" | "failed" | "missing" | "critical" => Tone::Critical,
        "warn" | "stopped" | "starting" | "false" => Tone::Warn,
        "—" | "null" => Tone::Muted,
        _ if column == 0 => Tone::Accent,
        _ => Tone::Heading,
    }
}
pub fn chart(text: &str) -> Result<()> {
    std::io::stdout().lock().write_all(text.as_bytes())?;
    Ok(())
}
pub fn report(report: &mammoth_core::ClusterReport, fmt: OutputFormat) -> Result<()> {
    if fmt.resolve() == OutputFormat::Table {
        chart(&format!("{}\n{}", mammoth_viz::cluster(report), mammoth_viz::health(&report.health)))
    } else {
        emit(report, fmt)
    }
}
pub fn files(files: &[mammoth_core::FileStatus], fmt: OutputFormat) -> Result<()> {
    if fmt.resolve() != OutputFormat::Table {
        return emit(&files, fmt);
    }
    if files.is_empty() {
        return chart("This directory is empty.\n");
    }
    let mut table = style::table();
    table.set_header(["path", "size", "storage"].map(|s| cell(s, Tone::Heading)));
    for file in files {
        table.add_row([
            cell(
                &style::clean(&format!(
                    "{}{}",
                    file.path.display(),
                    if file.is_dir { "/" } else { "" }
                )),
                if file.is_dir { Tone::Accent } else { Tone::Heading },
            ),
            comfy_table::Cell::new(if file.is_dir {
                "—".into()
            } else {
                mammoth_viz::size(file.len)
            }),
            cell(
                &if file.is_dir {
                    "directory".into()
                } else if file.inlined {
                    "inline".into()
                } else {
                    format!("{} blocks × {}", file.blocks, file.replication.unwrap_or(0))
                },
                Tone::Muted,
            ),
        ]);
    }
    chart(&format!("{table}\n"))
}
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "—".into(),
        _ => v.to_string(),
    }
}
fn csv_error(e: csv::Error) -> Error {
    Error::Io(std::io::Error::other(e))
}
pub fn print_error(e: &Error, fmt: OutputFormat) {
    if matches!(fmt.resolve(), OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Csv) {
        eprintln!(
            "{}",
            serde_json::json!({"code":e.code(),"message":e.to_string(),"hints":e.hints(),"docs":e.docs_url()})
        );
    } else {
        eprintln!(
            "{}: {}",
            paint_error(&format!("error[{}]", e.code()), Tone::Critical),
            style::clean(&e.to_string())
        );
        for hint in e.hints() {
            eprintln!("  {}", paint_error(&format!("· {hint}"), Tone::Warn));
        }
        eprintln!("  docs: {}", e.docs_url());
    }
}
