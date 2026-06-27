use colored::Colorize;
use serde::Serialize;

/// Print a success message (green checkmark + text).
pub fn print_success(msg: impl AsRef<str>) {
    println!("{} {}", "✓".green(), msg.as_ref());
}

/// Print a warning message (yellow exclamation + text).
pub fn print_warning(msg: impl AsRef<str>) {
    println!("{} {}", "!".yellow(), msg.as_ref());
}

/// Print an error message (red cross + text) to stderr.
pub fn print_error(msg: impl AsRef<str>) {
    eprintln!("{} {}", "✗".red(), msg.as_ref());
}

/// Print a dimmed "next step" hint with an arrow.
pub fn print_hint(msg: impl AsRef<str>) {
    println!("{} {}", "→".cyan(), msg.as_ref().dimmed());
}

/// Replace the user's home directory prefix with `~` for compact paths.
pub fn home_relative(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = path.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    path.to_string()
}

/// Pretty-print any Serialize value as JSON. Degrades to a stderr error rather
/// than panicking if serialization somehow fails (effectively never for our own
/// derived types, but we don't crash the process over output formatting).
pub fn print_json<T: Serialize>(val: &T) {
    match serde_json::to_string_pretty(val) {
        Ok(json) => println!("{json}"),
        Err(e) => print_error(format!("failed to serialize JSON output: {e}")),
    }
}

/// Print a simple formatted table.
/// Headers are bold. Column widths auto-sized.
/// Has no effect if `rows` is empty.
pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let col_count = headers.len();
    if col_count == 0 || rows.is_empty() {
        return;
    }

    // Compute column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < col_count {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    // Header row. Pad the plain text to width FIRST, then bold — applying a
    // width to an already-colored string counts the invisible ANSI bytes and
    // breaks alignment.
    for (i, h) in headers.iter().enumerate() {
        let padded = format!("{:width$}", h, width = widths[i]);
        print!("  {}  ", padded.bold());
    }
    println!();

    // Separator row
    for &w in &widths {
        print!("  {}  ", "─".repeat(w).dimmed());
    }
    println!();

    // Data rows
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < col_count {
                print!("  {:width$}  ", cell, width = widths[i]);
            }
        }
        println!();
    }
}

/// Print a human-readable diff line for a changed value.
pub fn print_diff(label: &str, old_val: &str, new_val: &str) {
    if old_val != new_val {
        println!("  {}: {}  {}", label, old_val.red(), new_val.green());
    } else {
        println!("  {}: {} (unchanged)", label, old_val);
    }
}
