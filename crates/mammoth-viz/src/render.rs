//! Compact terminal views. Numeric exports are kept in the CLI, separate from
//! these human layouts so ANSI styling never enters structured data.
use crate::style::{self, clip, paint, Tone};
use mammoth_core::{types::ReplicationHealth, BlockPlacement, ClusterReport, FileStatus};
use std::{collections::BTreeMap, path::Path};

pub fn size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut index = 0;
    while value >= 1024.0 && index < UNITS.len() - 1 {
        value /= 1024.0;
        index += 1;
    }
    if index == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[index])
    }
}

fn heading(title: &str) -> String {
    format!(
        "{}\n{}\n",
        paint(&clip(title, style::terminal_width()), Tone::Heading),
        paint(&"─".repeat(style::terminal_width()), Tone::Muted)
    )
}
pub fn bar(value: u64, total: u64, width: usize, tone: Tone) -> String {
    let fraction = if total == 0 { 0.0 } else { (value as f64 / total as f64).clamp(0.0, 1.0) };
    let cells = (fraction * width as f64).round() as usize;
    // A nonzero sliver remains visible; the accompanying number is exact.
    let cells = if value > 0 && width > 0 { cells.max(1) } else { cells };
    format!("{}{}", paint(&"█".repeat(cells), tone), paint(&"·".repeat(width - cells), Tone::Muted))
}
fn metric(name: &str, value: u64, total: u64, tone: Tone) -> String {
    let width = style::terminal_width();
    let label = clip(name, width.saturating_sub(16));
    format!(
        "{}  {}\n{}\n",
        paint(&label, tone),
        size(value),
        bar(value, total, width.min(48), tone)
    )
}

pub fn cluster(r: &ClusterReport) -> String {
    let mut out = heading(&format!("{} · WORKER CAPACITY", r.name));
    out += &format!("{} stored · {} workers\n\n", size(r.used), r.nodes.len());
    for node in &r.nodes {
        let tone = style::tone_for_node(node.state);
        out += &format!(
            "{}\n",
            paint(
                &clip(
                    &format!(
                        "{} · {} · {:?} · {} replicas",
                        node.id.0, node.rack, node.state, node.blocks
                    ),
                    style::terminal_width()
                ),
                tone
            )
        );
        let fraction =
            if node.capacity == 0 { 0.0 } else { node.used as f64 / node.capacity as f64 };
        out += &format!(
            "{} / {}  ({:.2}%)\n{}\n\n",
            size(node.used),
            size(node.capacity),
            fraction * 100.0,
            bar(
                node.used,
                node.capacity,
                style::terminal_width().min(48),
                style::tone_for_fill(fraction)
            )
        );
    }
    if r.nodes.is_empty() {
        out += "No workers reported.\n";
    }
    out += &paint("Capacity is a model value for each worker directory.\n", Tone::Muted);
    out
}

pub fn blocks(layout: &[BlockPlacement]) -> String {
    let mut out = heading("BLOCK PLACEMENT");
    if layout.is_empty() {
        out += "No block replicas. Empty or inline files live in namespace metadata.\n";
        return out;
    }
    let nodes: Vec<_> = layout
        .iter()
        .flat_map(|b| b.replicas.iter().map(|r| r.node.0.as_str()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let columns = ((style::terminal_width().saturating_sub(9)) / 9).max(1);
    for group in nodes.chunks(columns) {
        out += &paint(
            &format!(
                "{:8} {}\n",
                "block",
                group.iter().map(|n| format!("{:8}", clip(n, 8))).collect::<Vec<_>>().join(" ")
            ),
            Tone::Heading,
        );
        for block in layout {
            out += &format!("{:<8} ", block.index);
            for node in group {
                let tone = block
                    .replicas
                    .iter()
                    .find(|r| r.node.0 == *node)
                    .map(|r| style::tone_for_replica(r.state))
                    .unwrap_or(Tone::Muted);
                out += &paint(&format!("{:<8} ", tone.symbol()), tone);
            }
            out.push('\n');
        }
        out.push('\n');
    }
    for block in layout {
        out += &format!(
            "{}\n",
            clip(
                &format!(
                    "block {} · {} · {} copies · id {}",
                    block.index,
                    size(block.len),
                    block.replicas.len(),
                    block.id.0
                ),
                style::terminal_width()
            )
        );
    }
    out += "● primary · ▸ replica\n✕ damaged/missing · · absent\n";
    out
}

pub fn topology(report: &ClusterReport) -> String {
    let mut racks = BTreeMap::<&str, Vec<_>>::new();
    for node in &report.nodes {
        racks.entry(&node.rack).or_default().push(node);
    }
    let mut out = heading("RACK TOPOLOGY");
    out += &format!("{} workers · {} racks\n", report.nodes.len(), racks.len());
    let count = racks.len();
    for (index, (rack, nodes)) in racks.into_iter().enumerate() {
        let last = index + 1 == count;
        out += &format!(
            "{}{}\n",
            if last { "└─ " } else { "├─ " },
            paint(&clip(rack, style::terminal_width() - 3), Tone::Heading)
        );
        for (i, node) in nodes.iter().enumerate() {
            let prefix = format!(
                "{}{}",
                if last { "   " } else { "│  " },
                if i + 1 == nodes.len() { "└─ " } else { "├─ " }
            );
            let text = format!(
                "{} {:?} · {} · {} replicas",
                node.id.0,
                node.state,
                size(node.used),
                node.blocks
            );
            out += &format!(
                "{prefix}{}\n",
                paint(&clip(&text, style::terminal_width() - 6), style::tone_for_node(node.state))
            );
        }
    }
    out
}

pub fn health(report: &ReplicationHealth) -> String {
    let entries = [
        ("Healthy", report.healthy, Tone::Ok),
        ("Under-replicated", report.under_replicated, Tone::Warn),
        ("Critical", report.critical, Tone::Critical),
        ("Over-replicated", report.over_replicated, Tone::Warn),
        ("Corrupt", report.corrupt, Tone::Critical),
        ("Missing", report.missing, Tone::Critical),
    ];
    let max = entries.iter().map(|(_, value, _)| *value).max().unwrap_or(0);
    let mut out = heading("REPLICA HEALTH");
    for (name, value, tone) in entries {
        out += &format!(
            "{} {name}: {value}\n{}\n",
            paint(&tone.symbol().to_string(), if value > 0 { tone } else { Tone::Muted }),
            bar(value, max, style::terminal_width().min(40), tone)
        );
    }
    if report.under_replicated + report.critical + report.corrupt + report.missing > 0 {
        out += &paint("Inspect damaged copies: mammoth admin repair\n", Tone::Warn);
    }
    out
}

pub fn skew(path: &Path, entries: &[(String, u64)], partitions: bool) -> String {
    let mut rows = entries.to_vec();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let total = rows.iter().map(|(_, value)| value).sum::<u64>();
    let mut values = rows.iter().map(|(_, value)| *value).collect::<Vec<_>>();
    values.sort_unstable();
    let n = values.len();
    let max = values.last().copied().unwrap_or(0);
    let median = values.get(n / 2).copied().unwrap_or(0);
    let p99 = values.get((n * 99).div_ceil(100).saturating_sub(1)).copied().unwrap_or(0);
    let mut out = heading(&format!("SIZE SKEW · {}", path.display()));
    out += &format!(
        "{n} {} · {} total\nMedian {} · p99 {}\nMax {}\n\n",
        if partitions { "partitions" } else { "files" },
        size(total),
        size(median),
        size(p99),
        size(max)
    );
    if rows.is_empty() {
        out += "No files to plot.\n";
    }
    for (name, value) in rows {
        let tone = if median > 0 && value / median >= 4 { Tone::Warn } else { Tone::Accent };
        out += &metric(&name, value, max, tone);
    }
    out += &paint("Bars share a linear scale; largest entry fills the bar.\n", Tone::Muted);
    out
}

pub fn treemap(root: &Path, files: &[FileStatus], depth: u8) -> String {
    let total = files.iter().filter(|f| !f.is_dir).map(|f| f.len).sum::<u64>();
    let sizes: BTreeMap<_, _> = files
        .iter()
        .map(|file| {
            (
                file.path.as_path(),
                if file.is_dir {
                    files
                        .iter()
                        .filter(|f| !f.is_dir && f.path.starts_with(&file.path))
                        .map(|f| f.len)
                        .sum()
                } else {
                    file.len
                },
            )
        })
        .collect();
    let mut out = heading("NAMESPACE SIZE TREE");
    out += &format!(
        "{} · {}\n",
        paint(
            &clip(&root.to_string_lossy(), style::terminal_width().saturating_sub(16)),
            Tone::Heading
        ),
        size(total)
    );
    fn branch(
        out: &mut String,
        root: &Path,
        files: &[FileStatus],
        sizes: &BTreeMap<&Path, u64>,
        depth: u8,
        prefix: &str,
        total: u64,
    ) {
        if depth == 0 {
            return;
        }
        let mut children = files
            .iter()
            .filter(|f| f.path != root && f.path.parent() == Some(root))
            .collect::<Vec<_>>();
        children.sort_by(|a, b| {
            sizes[b.path.as_path()].cmp(&sizes[a.path.as_path()]).then_with(|| a.path.cmp(&b.path))
        });
        for (index, file) in children.iter().enumerate() {
            let last = index + 1 == children.len();
            let stem = format!("{prefix}{}", if last { "└─ " } else { "├─ " });
            let value = sizes[file.path.as_path()];
            let percent = if total == 0 { 0.0 } else { value as f64 / total as f64 * 100.0 };
            let name = format!(
                "{}{}",
                file.path.file_name().unwrap_or_default().to_string_lossy(),
                if file.is_dir { "/" } else { "" }
            );
            let width = style::terminal_width();
            let indentation = prefix.chars().count() + 3;
            let name = clip(&name, width.saturating_sub(indentation));
            *out += &format!(
                "{stem}{}\n",
                paint(&name, if file.is_dir { Tone::Heading } else { Tone::Accent })
            );
            let next = format!("{prefix}{}", if last { "   " } else { "│  " });
            *out += &format!(
                "{next}{}  {} ({percent:.1}%)\n",
                bar(value, total, width.saturating_sub(indentation + 22).min(24), Tone::Accent),
                size(value)
            );
            if file.is_dir {
                branch(out, &file.path, files, sizes, depth - 1, &next, total);
            }
        }
    }
    branch(&mut out, root, files, &sizes, depth, "", total);
    if total == 0 {
        out += "No file bytes stored in this subtree.\n";
    }
    out +=
        &paint("Directory totals include descendants; bars show share of the root.\n", Tone::Muted);
    out
}
