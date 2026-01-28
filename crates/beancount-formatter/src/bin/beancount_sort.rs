use clap::Parser;
use std::io::{self, Read};

/// Sort beancount directives by date
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Edit file in place
    #[arg(short, long)]
    in_place: bool,

    /// Input file (reads from stdin if not provided)
    file: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let content = if let Some(ref path) = args.file {
        // Read from file path argument
        std::fs::read_to_string(path)?
    } else {
        // Read from stdin
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    };

    let fmt = beancount_formatter::Formatter::new();
    let sorted = fmt.sort(&content);

    if args.in_place {
        // Write back to the same file
        if let Some(ref path) = args.file {
            std::fs::write(path, sorted)?;
        } else {
            eprintln!("Error: -i/--in-place requires a file argument");
            std::process::exit(1);
        }
    } else {
        // Write to stdout
        print!("{}", sorted);
    }

    Ok(())
}
