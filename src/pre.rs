use std::error::Error as ErrorTrait;
use std::io::Write;
use std::marker::{Send, Sync};

use anyhow::Result;

use crate::config::Config;
use crate::lex::{Lexer, Token};

/// Shorthand for a loop that runs $times times.
macro_rules! repeat {
    ($body:expr, $times:expr) => {
        for _ in 0..$times {
            $body;
        }
    };
}

/// Define a write_token_iter function with optional, additional arguments
/// and an statement to run after an operator has been written.
macro_rules! define_write_token_iter {
    {($output_ident:ident : $output_type:ty $(, $arg_ident:ident : $arg_type:ty)* ) $after: stmt} => {
        fn write_token_iter<'a, T, W>(token_iter: T, $output_ident: $output_type, $($arg_ident: $arg_type),*) -> Result<()>
        where
            W: Write,
            T: Iterator<Item = &'a Token>
        {
            let mut multiplier: usize = 1;
            for token in token_iter {
                match token {
                    Token::Group(group) => {
                        repeat!(write_token_iter(group.iter(), $output_ident, $($arg_ident),*)?, multiplier);
                        multiplier = 1;
                    },
                    Token::Operator(operator) => {
                        repeat!({
                            write!($output_ident, "{operator}")?;
                            $after
                        }, multiplier);
                        multiplier = 1;
                    },
                    Token::Number(number) => multiplier = *number,
                }
            }

            Ok(())
        }
    };
}

/// Run the preprocessor with the passed `config` on `input`, writing the result
/// to `output`.
///
/// ## Preprocessing behaviour
///
/// The following rules are applied when generating the output
/// *(in order, from most important, to least)*
/// 1. Macros are expanded
/// 2. The escape prefix skips the next `char`.
/// 3. A number prefix followed by a number **n**
/// multiply the next token **n** times.
/// 4. A macro prefix followed by any `char`, followed by a token,
/// defines the `char` as a macro evaluating to said token.
/// 5. Groups enclosed in group delimiters are treated as
/// a single token.
/// 6. Operators are copied to output.
/// 7. Every other `char` is skipped.
///
/// See [`Lexer`] for details about how tokens are recognized.
pub fn preprocess<I, W, E>(input: I, output: &mut W, config: &Config) -> Result<()>
where
    I: Iterator<Item = Result<char, E>>,
    W: Write,
    E: ErrorTrait + Sync + Send + 'static,
{
    define_write_token_iter!((output: &mut W) {});

    let tokens = Lexer::new(input, config).read_all_tokens()?;
    write_token_iter(tokens.iter(), output)?;

    Ok(())
}

/// Same as [`preprocess`], but aligns the output
/// in a rectangle of width `line_width`
pub fn preprocess_and_align<I, W, E>(
    input: I,
    output: &mut W,
    config: &Config,
    line_width: usize,
) -> Result<()>
where
    I: Iterator<Item = Result<char, E>>,
    W: Write,
    E: ErrorTrait + Sync + Send + 'static,
{
    define_write_token_iter!((output: &mut W, line_len: &mut usize, line_max_len: usize) {
        *line_len += 1;
        if *line_len == line_max_len {
            writeln!(output)?;
            *line_len = 0;
        }
    });

    let tokens = Lexer::new(input, config).read_all_tokens()?;
    write_token_iter(tokens.iter(), output, &mut 0, line_width)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use anyhow::Result;

    use super::*;
    use crate::config::Config;
    use bfup_derive::{as_char_results, as_char_results_and_input};

    macro_rules! preprocess_str_into_string {
        (let $input_ident:ident = $input:expr => $output:ident) => {
            let mut out = Cursor::new($output.into_bytes());
            let input_chars;
            (input_chars, $input_ident) = as_char_results_and_input!($input);

            preprocess(input_chars.into_iter(), &mut out, &Config::default())?;

            $output = String::from_utf8(out.into_inner())?;
        };
        (let $input_ident:ident = $input:expr => $output:ident with line_width = $line_width:expr) => {
            let mut out = Cursor::new($output.into_bytes());
            let input_chars;
            (input_chars, $input_ident) = as_char_results_and_input!($input);

            preprocess_and_align(
                input_chars.into_iter(),
                &mut out,
                &Config::default(),
                $line_width,
            )?;

            $output = String::from_utf8(out.into_inner())?;
        };
    }

    #[test]
    fn preprocess_copy_input() -> Result<()> {
        let mut output = String::new();

        let input: &str;
        preprocess_str_into_string!(
            let input = "++++[][]---<><><><>" => output
        );

        assert!(
            output == "++++[][]---<><><><>",
            "input (\"{input}\") and output (\"{output}\") should be equal.",
        );

        Ok(())
    }

    #[test]
    fn preprocess_multiplier() -> Result<()> {
        let mut output = String::new();

        let input: &str;
        preprocess_str_into_string!(
            let input = "#5+-#2(>#2(--#0(+++)))" => output
        );

        assert!(
            output == "+++++->---->----",
            "\"{input}\" preprocessed to \"{output}\" should be equal to \"+++++->---->----\".",
        );

        Ok(())
    }

    #[test]
    fn preprocess_macros() -> Result<()> {
        let mut output = String::new();

        let input: &str;
        preprocess_str_into_string!(
            let input = "$m/thistextwillbeskipped/+$g$\n (#2([-]))(--)mg\n" => output
        );

        assert!(
            output == "+--[-][-]",
            "\"{input}\" preprocessed to \"{output}\" should be equal to \"+--[-][-]\".",
        );

        Ok(())
    }

    #[test]
    fn preprocess_just_comments() -> Result<()> {
        let mut output = String::new();

        let input: &str;
        preprocess_str_into_string!(
            let input = "thiswillnotbecopied\\+\\#\\(\\)" => output
        );

        assert!(
            output == "",
            "\"{input}\" preprocessed to \"{output}\" should be \"\"."
        );

        Ok(())
    }

    #[test]
    fn preprocess_nothing() -> Result<()> {
        let mut output = Cursor::new(String::new().into_bytes());
        let input_chars: [Result<char, std::convert::Infallible>; 0] = as_char_results!("");

        preprocess(input_chars.into_iter(), &mut output, &Config::default())?;

        let output = String::from_utf8(output.into_inner())?;

        assert!(output == "", "output should be empty.");

        Ok(())
    }

    #[test]
    fn preprocess_with_alignment() -> Result<()> {
        let mut output = String::new();

        let input: &str;
        preprocess_str_into_string!(
            let input = "#6(#6(+))" => output with line_width = 6
        );

        assert!(
            output == "++++++\n++++++\n++++++\n++++++\n++++++\n++++++\n",
            "\"{input}\" preprocessed to \"{output}\" should be \"++++++\n++++++\n++++++\n++++++\n++++++\n++++++\n\".",
        );

        Ok(())
    }
}
