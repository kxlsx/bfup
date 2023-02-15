use std::collections::HashMap;
use std::error::Error as ErrorTrait;
use std::fmt;
use std::iter::Peekable;
use std::result::Result as StdResult;

use crate::config::{Config, ConfigField::*};
use bfup_derive::enum_fields;

/// Result type used within the [`Lexer`].
pub type Result<T, E> = std::result::Result<T, Error<E>>;

/// Struct representing a group of [`Errors`][Error].
/// When displayed, every error is printed sequentially, followed by a newline.
#[derive(fmt::Debug)]
pub struct ErrorGroup<E: ErrorTrait>(Vec<Error<E>>);

impl<E: ErrorTrait> fmt::Display for ErrorGroup<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        assert!(!self.0.is_empty(), "ErrorGroup shouldn't be empty.");

        let mut error_iter = self.0.iter().peekable();

        while let Some(error) = error_iter.next() {
            write!(f, "{error}")?;

            if error_iter.peek().is_some() {
                writeln!(f)?;
            } else {
                break;
            }
        }

        Ok(())
    }
}

/// Error type returned by the [`Lexer`].
/// Every error variant (except `Input`) contains the line and column
/// numbers specifying where in the input it occured.
#[enum_fields(![Input, Group]
    lineno: usize,
    colno: usize
)]
#[enum_fields(![Input, NumberMissing, MacroMissing, Group]
    group_start_delimiter: char,
    group_end_delimiter: char
)]
#[derive(thiserror::Error, fmt::Debug)]
pub enum Error<E: ErrorTrait> {
    #[error("{0}.")]
    Input(#[from] E),
    #[error("[{lineno}:{colno}]: '{group_end_delimiter}' must have a preceding '{group_start_delimiter}'.")]
    DelimiterUnopened,
    #[error("[{lineno}:{colno}]: expected '{group_end_delimiter}'.")]
    DelimiterUnclosed,
    #[error("[{lineno}:{colno}]: number prefix '{number_prefix}' must be followed by number.")]
    NumberMissing { number_prefix: char },
    #[error("[{lineno}:{colno}]: macro_prefix '{macro_prefix}' must be followed by a character and a token.")]
    MacroMissing { macro_prefix: char },
    #[error(
        "[{lineno}:{colno}]: group is empty ('{group_start_delimiter}{group_end_delimiter}')."
    )]
    GroupEmpty,
    #[error("{0}")]
    Group(ErrorGroup<E>),
}

/// A group of [Tokens][Token].
pub type Group = Vec<Token>;

/// A token enum returned by the [Lexer].
#[derive(Clone, fmt::Debug)]
pub enum Token {
    /// Decimal number preceded by a prefix specified
    /// in the [Config].
    Number(usize),
    /// Operator specified in the [Config].
    Operator(char),
    /// A group of Tokens.
    Group(Group),
}

/// Iterator over the [`Tokens`][Token]
/// read from an input: [`Iterator<Item = Result<char, E>>`][std::iter::Iterator].
///
/// The `Lexer` recognizes the following structures:
/// * Operators
/// * Numbers *(preceded by a number prefix)*
/// * Groups *(enclosed in group delimiters)*
/// * Macro definitions *(preceded by a macro prefix)*
/// * Macro occurences
///
///
/// Every `char` not defined as an operator, prefix, group delimiter or macro
/// is completely skipped
/// *(operators, prefixes and group delimiters are specified in the [`Config`]
/// passed to the `Lexer` when initializing)*.
/// In addition, specific characters can be escaped *(skipped by the `Lexer`)* when
/// preceded by an escape prefix.
///
/// ## Operators
///
/// Every `char` specified as an operator is yielded verbatim as a [`Token`].
///
/// ## Numbers
///
/// When a number prefix is encountered, the `Lexer` will try to
/// read the next chars as a base-10 number, yielding it as a [`Token`].
/// If the prefix is not followed by at least one decimal digit,
/// an [`Error::NumberMissing`] will be yielded.
///
/// ## Groups
///
/// Groups are a collection of [`Tokens`][Token] enclosed in group delimiters.
/// The `Lexer` will try to yield the group as a whole, returning an [`Error::Group`]
/// if any tokens in it were erroneous.
///
/// ## Macros
///
/// Macros are defined with a macro prefix followed by a `char`, followed by a valid token.
/// *(macro occurences are also tokens, but macro definitions are not)*
/// After a macro has been defined, every occurence of said char is replaced by the
/// specified token.
///
/// Be wary, that ***every*** `char` can be defined as a macro, even
/// operators, prefixes and group delimiters.
#[cfg_attr(feature = "integration-tests", visibility::make(pub))]
pub struct Lexer<'a, I, E>
where
    E: ErrorTrait,
    I: Iterator<Item = StdResult<char, E>>,
{
    config: &'a Config,
    char_iter: Peekable<I>,

    macro_symbol_table: HashMap<char, Token>,

    lineno: usize,
    colno: usize,
}

impl<'a, I, E> Lexer<'a, I, E>
where
    E: ErrorTrait,
    I: Iterator<Item = StdResult<char, E>>,
{
    /// Create a new `Lexer` with the given input and [`Config`].
    pub fn new(input: I, config: &'a Config) -> Self {
        Lexer {
            config,
            char_iter: input.peekable(),
            macro_symbol_table: HashMap::new(),
            lineno: 1,
            colno: 0,
        }
    }

    /// Try to read every token in the `Lexer`'s input into a [`Vec<Token>`].
    pub fn read_all_tokens(&mut self) -> Result<Vec<Token>, E> {
        const TOKEN_STOR_INIT_SIZE: usize = 32;

        let mut tokens: Vec<Token> = Vec::with_capacity(TOKEN_STOR_INIT_SIZE);
        let mut errors: Vec<Error<E>> = Vec::new();
        loop {
            match self.read_token() {
                Some(Err(Error::Input(error))) => return Err(Error::Input(error)),
                Some(Ok(token)) => tokens.push(token),
                Some(Err(error)) => errors.push(error),
                None => break,
            }
        }

        if !errors.is_empty() {
            return Err(Error::Group(ErrorGroup(errors)));
        }

        Ok(tokens)
    }

    /// Try to read a [`Token`].
    pub fn read_token(&mut self) -> Option<Result<Token, E>> {
        loop {
            let ch = match self.next_char() {
                Some(Ok(ch)) => ch,
                Some(Err(error)) => return Some(Err(error)),
                None => return None,
            };

            if let Some(macro_token) = self.macro_symbol_table.get(&ch) {
                return Some(Ok(macro_token.clone()));
            }

            match self.config.get_field(&ch) {
                Some(EscapePrefix) => {
                    // skip the next character
                    self.next_char();
                    continue;
                }
                Some(NumberPrefix) => match self.read_number() {
                    Ok(number) => return Some(Ok(Token::Number(number))),
                    Err(error) => return Some(Err(error)),
                },
                Some(MacroPrefix) => match self.read_macro_definition() {
                    Ok(_) => continue,
                    Err(error) => return Some(Err(error)),
                },
                Some(GroupStartDelimiter) => match self.read_group() {
                    Ok(group) => return Some(Ok(Token::Group(group))),
                    Err(error) => return Some(Err(error)),
                },
                Some(GroupEndDelimiter) => {
                    return Some(Err(Error::DelimiterUnopened {
                        lineno: self.lineno,
                        colno: self.colno,
                        group_start_delimiter: *self.config.get_value(&GroupStartDelimiter),
                        group_end_delimiter: *self.config.get_value(&GroupEndDelimiter),
                    }));
                }
                Some(Operator) => {
                    return Some(Ok(Token::Operator(ch)));
                }
                None => (),
            }
        }
    }

    /// Try to read a base 10 number from input.
    fn read_number(&mut self) -> Result<usize, E> {
        const NUMBER_STOR_INIT_SIZE: usize = 8;

        let mut number_string = String::with_capacity(NUMBER_STOR_INIT_SIZE);

        loop {
            if let Some(Ok(next_ch)) = self.char_iter.peek() {
                if !next_ch.is_ascii_digit() {
                    break;
                }
            }

            match self.next_char() {
                Some(Ok(ch)) => number_string.push(ch),
                None => break,
                Some(Err(error)) => return Err(error),
            }
        }

        if let Ok(number) = number_string.parse::<usize>() {
            Ok(number)
        } else {
            Err(Error::NumberMissing {
                lineno: self.lineno,
                colno: self.colno,
                number_prefix: *self.config.get_value(&NumberPrefix),
            })
        }
    }

    /// Try to read a macro definition and set it into the symbol table.
    fn read_macro_definition(&mut self) -> Result<(), E> {
        let macro_symbol = match self.next_char() {
            Some(Ok(ch)) => ch,
            Some(Err(error)) => return Err(error),
            None => {
                return Err(Error::MacroMissing {
                    lineno: self.lineno,
                    colno: self.colno,
                    macro_prefix: *self.config.get_value(&MacroPrefix),
                })
            }
        };

        let macro_token = match self.read_token() {
            Some(Ok(token)) => token,
            Some(Err(error)) => return Err(error),
            None => {
                return Err(Error::MacroMissing {
                    lineno: self.lineno,
                    colno: self.colno,
                    macro_prefix: *self.config.get_value(&MacroPrefix),
                })
            }
        };

        self.macro_symbol_table.insert(macro_symbol, macro_token);

        Ok(())
    }

    /// Try to read a group, yields [`Error::Group`] on error.
    fn read_group(&mut self) -> Result<Group, E> {
        const GROUP_STOR_INIT_SIZE: usize = 16;

        let mut group_tokens: Vec<Token> = Vec::with_capacity(GROUP_STOR_INIT_SIZE);
        let mut errors: Vec<Error<E>> = Vec::new();
        loop {
            match self.read_token() {
                Some(Ok(token)) => group_tokens.push(token),
                Some(Err(Error::DelimiterUnopened { .. })) => break,
                Some(Err(error)) => errors.push(error),
                None => {
                    errors.push(Error::DelimiterUnclosed {
                        lineno: self.lineno,
                        colno: self.colno,
                        group_start_delimiter: *self.config.get_value(&GroupStartDelimiter),
                        group_end_delimiter: *self.config.get_value(&GroupEndDelimiter),
                    });
                    break;
                }
            }
        }

        if !errors.is_empty() {
            return Err(Error::Group(ErrorGroup(errors)));
        }

        if !group_tokens.is_empty() {
            Ok(group_tokens)
        } else {
            Err(Error::GroupEmpty {
                lineno: self.lineno,
                colno: self.colno,
                group_start_delimiter: *self.config.get_value(&GroupStartDelimiter),
                group_end_delimiter: *self.config.get_value(&GroupEndDelimiter),
            })
        }
    }

    /// Advance the input iterator.
    fn next_char(&mut self) -> Option<Result<char, E>> {
        let next_char = self.char_iter.next();

        self.colno += 1;

        match next_char {
            Some(Ok('\n')) => {
                self.lineno += 1;
                self.colno = 0;
                Some(Ok('\n'))
            }
            Some(Ok(ch)) => Some(Ok(ch)),
            Some(Err(error)) => Some(Err(Error::Input(error))),
            None => None,
        }
    }
}

impl<'a, I, E> Iterator for Lexer<'a, I, E>
where
    E: ErrorTrait,
    I: Iterator<Item = StdResult<char, E>>,
{
    type Item = Result<Token, E>;

    fn next(&mut self) -> Option<Self::Item> {
        self.read_token()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;
    use crate::config::Config;
    use bfup_derive::as_char_results;

    #[test]
    fn lex_operator() -> Result<()> {
        let input = as_char_results!('+');
        let token = Lexer::new(input.into_iter(), &Config::default())
            .next()
            .expect("The lexer should not be empty.")?;

        assert!(
            if let Token::Operator('+') = token {
                true
            } else {
                false
            },
            "Operators don't match."
        );

        Ok(())
    }

    #[test]
    fn lex_number() -> Result<()> {
        let input = as_char_results!("#2137");
        let token = Lexer::new(input.into_iter(), &Config::default())
            .next()
            .expect("The lexer should not be empty.")?;

        assert!(
            if let Token::Number(2137) = token {
                true
            } else {
                false
            },
            "Numbers don't match."
        );

        Ok(())
    }

    #[test]
    fn lex_group() -> Result<()> {
        let input = as_char_results!("(#42-)");
        let token = Lexer::new(input.into_iter(), &Config::default())
            .next()
            .expect("The lexer should not be empty.")?;

        if let Token::Group(group) = token {
            match group.get(0) {
                Some(Token::Number(42)) => (),
                _ => panic!("Numbers don't match."),
            }
            match group.get(1) {
                Some(Token::Operator('-')) => (),
                _ => panic!("Operators don't match."),
            }
        } else {
            panic!("The token should be Token::Group.")
        }

        Ok(())
    }

    #[test]
    fn lex_macro() -> Result<()> {
        let input = as_char_results!("$m+m");
        let token = Lexer::new(input.into_iter(), &Config::default())
            .next()
            .expect("The lexer should not be empty.")?;

        assert!(
            if let Token::Operator('+') = token {
                true
            } else {
                false
            },
            "Operators don't match."
        );

        Ok(())
    }

    #[test]
    fn lex_escape() -> Result<()> {
        let input = as_char_results!("thiswillnotbelexed\\+\\#\\(\\)");
        let token = Lexer::new(input.into_iter(), &Config::default()).next();

        assert!(token.is_none(), "The lexer should be empty");

        Ok(())
    }

    #[test]
    fn lex_nothing() -> Result<()> {
        let input: [Result<char, std::convert::Infallible>; 0] = as_char_results!("");
        let token = Lexer::new(input.into_iter(), &Config::default()).next();

        assert!(token.is_none(), "The lexer should be empty");

        Ok(())
    }
}
