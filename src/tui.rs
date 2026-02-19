use owo_colors::OwoColorize;
use unicode_width::UnicodeWidthStr;

// ── Box drawing ──

const TL: &str = "╭";
const TR: &str = "╮";
const BL: &str = "╰";
const BR: &str = "╯";
const H: &str = "─";
const V: &str = "│";
/// Render a boxed section with a title and body lines.
pub fn section(title: &str, rows: &[String]) -> String {
    let content_width = compute_width(title, rows);
    let mut out = String::new();

    // Top border with title
    let title_display = format!(" {} ", title);
    let title_vis_width = visible_width(&title_display);
    let remaining = content_width.saturating_sub(title_vis_width);
    out.push_str(&format!(
        "{}{}{}{}\n",
        TL.dimmed(),
        H.repeat(1).dimmed(),
        title_display.bold(),
        format!("{}{}", H.repeat(remaining + 1), TR).dimmed(),
    ));

    // Body
    for row in rows {
        let vis_w = visible_width(row);
        let pad = content_width.saturating_sub(vis_w);
        out.push_str(&format!(
            "{}  {}{}{}\n",
            V.dimmed(),
            row,
            " ".repeat(pad),
            V.dimmed(),
        ));
    }

    // Bottom border
    out.push_str(&format!(
        "{}{}{}\n",
        BL.dimmed(),
        H.repeat(content_width + 2).dimmed(),
        BR.dimmed(),
    ));

    out
}

// ── Labels ──

pub fn label_collected(text: &str) -> String {
    format!("{}", text.cyan())
}

pub fn label_symlinked(text: &str) -> String {
    format!("{}", text.green())
}

pub fn label_native(text: &str) -> String {
    format!("{}", text.dimmed())
}

pub fn label_removed(text: &str) -> String {
    format!("{}", text.red())
}

pub fn label_warning(text: &str) -> String {
    format!("{}", text.yellow())
}

// ── Status badges ──

pub fn badge_ok(text: &str) -> String {
    format!("{} {}", "✔".green(), text)
}

pub fn badge_warn(text: &str) -> String {
    format!("{} {}", "⚠".yellow(), text)
}

pub fn badge_err(text: &str) -> String {
    format!("{} {}", "✘".red(), text)
}

pub fn badge_info(text: &str) -> String {
    format!("{} {}", "ℹ".dimmed(), text)
}

pub fn badge_skip(text: &str) -> String {
    format!("{} {}", "⏭".dimmed(), text)
}

pub fn badge_broken(text: &str) -> String {
    format!("{} {}", "💔".red(), text)
}

// ── Table formatting ──

/// Format rows as an aligned table. Each row is a vec of columns.
pub fn table(rows: &[Vec<String>]) -> Vec<String> {
    if rows.is_empty() {
        return vec![];
    }

    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut col_widths = vec![0usize; col_count];

    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            col_widths[i] = col_widths[i].max(visible_width(cell));
        }
    }

    rows.iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, cell)| {
                    let vis_w = visible_width(cell);
                    let pad = col_widths[i].saturating_sub(vis_w);
                    if i == row.len() - 1 {
                        cell.to_string()
                    } else {
                        format!("{}{}  ", cell, " ".repeat(pad))
                    }
                })
                .collect::<String>()
        })
        .collect()
}

// ── Header ──

pub fn header(command: &str, dry_run: bool) -> String {
    let label = if dry_run {
        format!("🌸 hana {} {}", command, "(dry-run)".dimmed())
    } else {
        format!("🌸 hana {}", command)
    };
    format!("{}\n", label.bold())
}

pub fn footer_done() -> String {
    format!("{}\n", "Done.".dimmed())
}

pub fn footer_no_changes() -> String {
    format!(
        "{}\n{}",
        "Everything is in sync. No changes needed.".dimmed(),
        footer_done()
    )
}

// ── Internal ──

fn compute_width(title: &str, rows: &[String]) -> usize {
    let title_w = visible_width(title) + 2; // padding around title
    let max_row = rows.iter().map(|r| visible_width(r)).max().unwrap_or(0);
    title_w.max(max_row).max(20)
}

/// Calculate visible width of a string, stripping ANSI escape codes.
fn visible_width(s: &str) -> usize {
    let stripped = strip_ansi(s);
    UnicodeWidthStr::width(stripped.as_str())
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until 'm' (end of SGR sequence)
            for c2 in chars.by_ref() {
                if c2 == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_width_plain() {
        assert_eq!(visible_width("hello"), 5);
    }

    #[test]
    fn test_visible_width_with_ansi() {
        let colored = format!("{}", "hello".green());
        assert_eq!(visible_width(&colored), 5);
    }

    #[test]
    fn test_strip_ansi() {
        let colored = format!("{}", "test".red().bold());
        assert_eq!(strip_ansi(&colored), "test");
    }

    #[test]
    fn test_table_alignment() {
        let rows = vec![
            vec!["short".to_string(), "a".to_string()],
            vec!["much longer".to_string(), "b".to_string()],
        ];
        let result = table(&rows);
        assert_eq!(result.len(), 2);
        // Both rows should have same visual alignment
        assert!(result[0].contains("short"));
        assert!(result[1].contains("much longer"));
    }

    #[test]
    fn test_section_output() {
        let out = section("Test", &["line 1".to_string(), "line 2".to_string()]);
        let stripped = strip_ansi(&out);
        assert!(stripped.contains("Test"));
        assert!(stripped.contains("line 1"));
        assert!(stripped.contains("line 2"));
    }
}
