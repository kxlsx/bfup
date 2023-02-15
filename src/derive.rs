use std::collections::HashSet;

use function_name::named;
use proc_macro as proc;
use proc_macro2::TokenStream;
use proc_macro_error::{abort, proc_macro_error};
use quote::{quote, ToTokens};
use syn::{parse::*, punctuated::*, *};

/// Abort, but with the function's name appended at the front.
macro_rules! abort_named_fn {
    ($span:expr, $fmt:expr $(, $arg:expr)*) => {
        abort!($span, concat!("{}: ", $fmt), function_name!(), $($arg),*)
    };
}

/// Converts the passed `char` or `str` literal
/// into an array of chars wrapped in
/// `Result::<char, std::convert::Infallible>::Ok`.
///
/// # Example
/// ```
/// use bfup_derive::as_char_results;
///
/// let wrapped = as_char_results!("abc");
///
/// assert!(wrapped[0] == Ok('a'));
/// assert!(wrapped[1] == Ok('b'));
/// assert!(wrapped[2] == Ok('c'));
/// ```
#[proc_macro]
#[proc_macro_error]
#[named]
pub fn as_char_results(input: proc::TokenStream) -> proc::TokenStream {
    let input_literal = parse_macro_input!(input as ExprLit);

    match input_literal.lit {
        Lit::Str(str_literal) => {
            let mut ok_wrapped_chars: Punctuated<Expr, Token![,]> = Punctuated::new();
            for char in str_literal.value().chars() {
                ok_wrapped_chars.push(
                    parse_quote!(std::result::Result::<char, std::convert::Infallible>::Ok(#char)),
                )
            }

            proc::TokenStream::from(quote!([ #ok_wrapped_chars ]))
        }
        Lit::Char(char_literal) => {
            let char = char_literal.value();

            proc::TokenStream::from(
                quote!([ std::result::Result::<char, std::convert::Infallible>::Ok(#char) ]),
            )
        }
        _ => abort_named_fn!(input_literal, "Input must be a string or char literal."),
    }
}

/// The same as [`as_char_results()`], but evaluates to
/// a tuple containing the char_results and the input literal.
///
/// # Example
/// ```
/// use bfup_derive::as_char_results_and_input;
///
/// let (wrapped, input) = as_char_results_and_input!("abc");
///     
/// assert!(input == "abc");
/// assert!(wrapped[0] == Ok('a'));
/// assert!(wrapped[1] == Ok('b'));
/// assert!(wrapped[2] == Ok('c'));
/// ```
#[proc_macro]
pub fn as_char_results_and_input(input: proc::TokenStream) -> proc::TokenStream {
    let input_literal = TokenStream::from(input.clone());
    let ok_wrapped_chars = TokenStream::from(as_char_results(input));

    proc::TokenStream::from(quote!(
        (#ok_wrapped_chars , #input_literal)
    ))
}

/// A shorthand for setting repeating named fields
/// in an enum's variants.
///
/// Fields passed into this macro are set into every
/// variant in the enum, but an optional "skip list" can
/// be defined, by listing the variants to be skipped at
/// the beginning, enclosed in "![]".
///
/// # Example
/// ```
/// use bfup_derive::enum_fields;
///
/// #[enum_fields(![Three] foo: i32, bar: u32)]
/// enum Numbers {
///     One,
///     Two{ skrzat: u8 },
///     Three,
/// }
///
/// let one = Numbers::One { foo: 21, bar: 37 };
/// let two = Numbers::Two { foo: 5, bar: 5, skrzat: 42 };
/// let three = Numbers::Three;
/// ```
#[proc_macro_attribute]
#[proc_macro_error]
#[named]
pub fn enum_fields(args: proc::TokenStream, input: proc::TokenStream) -> proc::TokenStream {
    let mut enum_definition = parse_macro_input!(input as ItemEnum);

    let (skip_list, field_list) = parse_macro_input!(args with parse_enum_fields_args);
    let fields: FieldsNamed = parse_quote!({ #field_list });

    for enum_variant in &mut enum_definition.variants {
        if skip_list.contains(&enum_variant.ident) {
            continue;
        }
        match &mut enum_variant.fields {
            Fields::Unit => enum_variant.fields = Fields::Named(fields.clone()),
            Fields::Named(existing_fields) => existing_fields.named.extend(fields.named.clone()),
            Fields::Unnamed(_) => abort_named_fn!(
                enum_variant,
                "Cannot add a named field to a tuple-like enum variant."
            ),
        }
    }

    proc::TokenStream::from(enum_definition.to_token_stream())
}

/// A set of identifiers to skip in [`enum_fields`].
///
/// Parsed from the following syntax:
///
/// `![VARIANT1, VARIANT2, ...]`
struct SkipList {
    to_skip: HashSet<Ident>,
}

impl Parse for SkipList {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut skip_list = SkipList::new();

        input.parse::<Token![!]>()?;
        let bracket_content;
        bracketed!(bracket_content in input);

        loop {
            skip_list.insert(bracket_content.parse()?);
            if bracket_content.is_empty() {
                break;
            }

            bracket_content.parse::<Token![,]>()?;

            if bracket_content.is_empty() {
                break;
            }
        }

        Ok(skip_list)
    }
}

impl SkipList {
    pub fn new() -> Self {
        SkipList {
            to_skip: HashSet::new(),
        }
    }

    pub fn insert(&mut self, ident: Ident) -> bool {
        self.to_skip.insert(ident)
    }

    pub fn contains(&self, ident: &Ident) -> bool {
        self.to_skip.contains(ident)
    }
}

/// A punctuated list of named field definitions.
struct FieldList {
    fields: Punctuated<Field, Token![,]>,
}

impl Parse for FieldList {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut fields: Punctuated<Field, Token![,]> = Punctuated::new();

        loop {
            fields.push_value(Field::parse_named(input)?);
            if input.is_empty() {
                break;
            }

            fields.push_punct(input.parse()?);
            if input.is_empty() {
                break;
            }
        }

        Ok(FieldList { fields })
    }
}

impl ToTokens for FieldList {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.fields.to_tokens(tokens);
    }
}

/// Parse the arguments passed into [`enum_fields`] into a [`SkipList`] and [`FieldList`].
fn parse_enum_fields_args(input: ParseStream) -> Result<(SkipList, FieldList)> {
    let skip_list = if input.peek(Token![!]) {
        match SkipList::parse(input) {
            Ok(list) => list,
            Err(error) => return Err(error),
        }
    } else {
        SkipList::new()
    };
    let field_list = match FieldList::parse(input) {
        Ok(list) => list,
        Err(error) => return Err(error),
    };

    Ok((skip_list, field_list))
}
