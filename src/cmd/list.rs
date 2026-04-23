use anyhow::Result;

use crate::cli::OutputFormat;
use crate::midi::port;

/// Executes the `list` subcommand.
pub fn execute(args: crate::cli::ListArgs) -> Result<()> {
    let ports = port::list_inputs()?;

    match args.format {
        OutputFormat::Human => print_human(&ports),
        OutputFormat::Json => print_json(&ports),
    }

    Ok(())
}

fn print_human(ports: &[(usize, String)]) {
    println!("Index  Name");
    println!("─────  ────────────────────────────────");
    for (index, name) in ports {
        println!("{index:<5}  {name}");
    }
}

fn print_json(ports: &[(usize, String)]) {
    println!("[");
    for (i, (index, name)) in ports.iter().enumerate() {
        let comma = if i + 1 == ports.len() { "" } else { "," };
        println!(
            "  {{\"index\":{},\"name\":\"{}\"}}{}",
            index,
            escape_json(name),
            comma
        );
    }
    println!("]");
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::escape_json;

    #[test]
    /// Verifies JSON-special characters are escaped in list output.
    fn escapes_json_content() {
        assert_eq!(escape_json("a\"b"), "a\\\"b");
    }
}
