//! Consistent human output and structured output with no panic paths.
use crate::cli::OutputFormat;
use mammoth_core::{Error, Result};
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
            let rows = match &value {
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
                    headers.iter().map(|k| text(if r.is_object() { &r[k] } else { r })).collect()
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
                let mut table = comfy_table::Table::new();
                table.load_preset(comfy_table::presets::UTF8_FULL).set_header(headers);
                for row in values {
                    table.add_row(row);
                }
                writeln!(out, "{table}")?;
            }
        }
        OutputFormat::Auto => unreachable!("format resolved"),
    }
    Ok(())
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
        eprintln!("error[{}]: {}", e.code(), e);
        for hint in e.hints() {
            eprintln!("  · {hint}");
        }
        eprintln!("  docs: {}", e.docs_url());
    }
}
