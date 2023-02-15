use std::fs::File;
use std::io::{stdin, stdout, BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use utf8_chars::BufReadCharsExt;

use crate::config::{self, Config};
use crate::pre::{preprocess, preprocess_and_align};

const DEFAULT_LINE_WIDTH: usize = 32;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(help_template(
    "\
{name} {version}
{author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}
"
))]
struct Cli {
    /// File to preprocess [default: stdin]
    #[arg(value_name = "FILE")]
    input: Option<PathBuf>,

    /// Specify output filename
    #[arg(short = 'o', long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Read preprocessor config from a ron file.
    #[arg(short = 'C', long, value_name = "FILE")]
    config_file: Option<PathBuf>,

    /// Specify recognized operators
    #[arg(short = '+', long,
        conflicts_with = "config_file",
        default_value_t = String::from(config::DEFAULT_OPERATORS),
    )]
    operators: String,

    /// Specify number prefix
    #[arg(short = '#', long,
        conflicts_with = "config_file",
        default_value_t = config::DEFAULT_NUMBER_PREFIX,
        value_name = "CHAR",
    )]
    number_prefix: char,

    /// Specify macro prefix
    #[arg(short = 'm', long,
        conflicts_with = "config_file",
        default_value_t = config::DEFAULT_MACRO_PREFIX,
        value_name = "CHAR",
    )]
    macro_prefix: char,

    /// Specify escape prefix
    #[arg(short = 'e', long,
        conflicts_with = "config_file",
        default_value_t = config::DEFAULT_ESCAPE_PREFIX,
        value_name = "CHAR",
    )]
    escape_prefix: char,

    /// Specify group start delimiter
    #[arg(long,
        conflicts_with = "config_file",
        default_value_t = config::DEFAULT_GROUP_START_DELIMITER,
        value_name = "CHAR",
    )]
    group_start_delimiter: char,

    /// Specify group end delimiter
    #[arg(long,
        conflicts_with = "config_file",
        default_value_t = config::DEFAULT_GROUP_END_DELIMITER,
        value_name = "CHAR",
    )]
    group_end_delimiter: char,

    /// Do not align output in a rectangle
    #[arg(short = 'n', long)]
    no_align: bool,

    /// Do not append a newline character at the end
    #[arg(short = 'b', long)]
    no_newline: bool,

    /// Specify max line width
    #[arg(short = 'l', long,
        conflicts_with = "no_align",
        default_value_t = DEFAULT_LINE_WIDTH,
        value_name = "WIDTH",
    )]
    line_width: usize,

    /// Print license
    #[arg(short = 'L', long)]
    license: bool,
}

/// Read args from env and act on them accordingly.
pub fn process_args() -> Result<()> {
    let cli = Cli::parse();

    if cli.license {
        print_license();
        return Ok(());
    }

    let mut input: Box<dyn BufRead> = if let Some(path) = &cli.input {
        Box::new(BufReader::new(File::open(path).with_context(|| {
            format!("failed to open '{}'", path.display())
        })?))
    } else {
        Box::new(stdin().lock())
    };

    let mut output: Box<dyn Write> = if let Some(path) = &cli.output {
        Box::new(BufWriter::new(File::create(path).with_context(|| {
            format!("failed to open '{}'", path.display())
        })?))
    } else {
        Box::new(stdout().lock())
    };

    let config = if let Some(path) = &cli.config_file {
        let config_reader = BufReader::new(
            File::open(path)
                .with_context(|| format!("failed to open config '{}'", path.display()))?,
        );

        Config::from_reader_ron(config_reader)
            .with_context(|| format!("failed to parse config '{}'", path.display()))?
    } else {
        Config::new(
            cli.operators.chars(),
            cli.group_start_delimiter,
            cli.group_end_delimiter,
            cli.number_prefix,
            cli.macro_prefix,
            cli.escape_prefix,
        )
        .with_context(|| "invalid configuration")?
    };

    if cli.no_align {
        preprocess(input.chars_raw(), &mut output, &config)
    } else {
        preprocess_and_align(input.chars_raw(), &mut output, &config, cli.line_width)
    }
    .with_context(|| "failure while preprocessing")?;

    if !cli.no_newline {
        writeln!(output).with_context(|| "write failure")?;
    }

    Ok(())
}

fn print_license() {
    const LICENSE: &str =
        "This is free software. You may redistribute copies of it under the terms of
the GNU General Public License <https://www.gnu.org/licenses/gpl.html>.
There is NO WARRANTY, to the extent permitted by law.";
    // just in case
    debug_assert!(
        env!("CARGO_PKG_LICENSE").starts_with("GPL-3.0"),
        "LICENSE message needs to be updated."
    );

    println!(
        "{} {}\n{}\n\n{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_AUTHORS"),
        LICENSE
    );
}
