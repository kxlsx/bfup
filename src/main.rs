/// Parsing args and acting on them accordingly.
mod cli;
/// Packaging & verifying 
/// the preprocessor's configuration.
mod config;
/// Module mainly containing 
/// the [`Lexer`][crate::lex::Lexer] iterator
/// over the tokens recognized by the preprocessor.
mod lex;
/// Module containing the main preprocessor 
/// functions.
mod pre;

use std::process::ExitCode;

use anyhow::Result;
use colored::Colorize;

// TODO: accept multiple files? (chain?)

fn main() -> ExitCode {
    check_and_print_result(cli::process_args())
}

fn check_and_print_result(result: Result<()>) -> ExitCode {
    if let Err(err) = result {
        eprintln!("{} {}\n", "error:".red().bold(), err);
        if let Some(cause) = err.chain().nth(1) {
            eprintln!("{}", cause);
        }
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}
