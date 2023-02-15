use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::io::Read;

use ron::error::SpannedError as RonError;
use serde::Deserialize;

/// Error type returned when constructing a [`Config`]
#[derive(thiserror::Error, fmt::Debug)]
pub enum Error {
    #[error("{0} cannot be {1}.")]
    NotUnique(String, String),
    #[error("{0}")]
    FromRon(String),
}

impl From<RonError> for Error {
    fn from(ron_error: RonError) -> Self {
        Error::FromRon(format!(
            "[{}:{}]: {}",
            ron_error.position.line, ron_error.position.col, ron_error.code
        ))
    }
}

pub const DEFAULT_OPERATORS: &str = "+-<>[].,";
pub const DEFAULT_GROUP_START_DELIMITER: char = '(';
pub const DEFAULT_GROUP_END_DELIMITER: char = ')';
pub const DEFAULT_NUMBER_PREFIX: char = '#';
pub const DEFAULT_MACRO_PREFIX: char = '$';
pub const DEFAULT_ESCAPE_PREFIX: char = '\\';

/// The type of a field contained within the [`Config`]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigField {
    Operator,
    GroupStartDelimiter,
    GroupEndDelimiter,
    NumberPrefix,
    MacroPrefix,
    EscapePrefix,
}

impl fmt::Display for ConfigField {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Operator => "operator",
                Self::GroupStartDelimiter => "group start delimiter",
                Self::GroupEndDelimiter => "group end delimiter",
                Self::NumberPrefix => "number prefix",
                Self::MacroPrefix => "macro prefix",
                Self::EscapePrefix => "escape prefix",
            }
        )
    }
}

/// Struct containing config information for the
/// [`Lexer`][crate::lex::Lexer]. The possible
/// fields are defined within the [`ConfigField`] enum.
///
/// Use `get_field()` to check whether a field contains the passed value.
///
/// Use 'get_value()` to get a field's value.
pub struct Config {
    values_to_fields: HashMap<char, ConfigField>,
    fields_to_values: HashMap<ConfigField, char>,
}

impl Default for Config {
    fn default() -> Self {
        Config::new(
            DEFAULT_OPERATORS.chars(),
            DEFAULT_GROUP_START_DELIMITER,
            DEFAULT_GROUP_END_DELIMITER,
            DEFAULT_NUMBER_PREFIX,
            DEFAULT_MACRO_PREFIX,
            DEFAULT_ESCAPE_PREFIX,
        )
        .expect("Default config shouldn't fail.")
    }
}

/// Return error if the char is already assigned to a field.
macro_rules! try_insert_fields {
    { $map:expr => $( ( $ch:expr, $field:expr ) ),+ } => {
        $(
        if let Some(field) = $map.insert($ch, $field) {
            return Err(Error::NotUnique($field.to_string(), field.to_string()));
        }
        )+
    };
}

impl Config {
    /// Initialize a new config,
    /// returns error if the passed values are not unique within the `Config`.
    pub fn new<C: IntoIterator<Item = char>>(
        operators: C,
        group_start_delimiter: char,
        group_end_delimiter: char,
        number_prefix: char,
        macro_prefix: char,
        escape_prefix: char,
    ) -> Result<Self, Error> {
        let mut field_map: HashMap<char, ConfigField> = HashMap::new();

        operators.into_iter().for_each(|ch| {
            field_map.insert(ch, ConfigField::Operator);
        });

        try_insert_fields! {
            field_map =>
                (group_start_delimiter, ConfigField::GroupStartDelimiter),
                (group_end_delimiter, ConfigField::GroupEndDelimiter),
                (number_prefix, ConfigField::NumberPrefix),
                (macro_prefix, ConfigField::MacroPrefix),
                (escape_prefix, ConfigField::EscapePrefix)
        };

        Ok(Config {
            fields_to_values: field_map.iter().map(|(ch, field)| (*field, *ch)).collect(),
            values_to_fields: field_map,
        })
    }

    /// Deserialize a `Config` struct from reader containing ron specification.
    pub fn from_reader_ron<R: Read>(reader: R) -> Result<Config, Error> {
        // TODO: generate from ConfigFields with procmacro?
        #[derive(Deserialize)]
        #[serde(rename = "Config", default)]
        struct ConfigDe {
            operators: String,
            group_start_delimiter: char,
            group_end_delimiter: char,
            number_prefix: char,
            macro_prefix: char,
            escape_prefix: char,
        }

        impl Default for ConfigDe {
            fn default() -> Self {
                ConfigDe {
                    operators: String::from(DEFAULT_OPERATORS),
                    group_start_delimiter: DEFAULT_GROUP_START_DELIMITER,
                    group_end_delimiter: DEFAULT_GROUP_END_DELIMITER,
                    number_prefix: DEFAULT_NUMBER_PREFIX,
                    macro_prefix: DEFAULT_MACRO_PREFIX,
                    escape_prefix: DEFAULT_ESCAPE_PREFIX,
                }
            }
        }

        let de: ConfigDe = ron::de::from_reader(reader)?;

        Config::new(
            de.operators.chars(),
            de.group_start_delimiter,
            de.group_end_delimiter,
            de.number_prefix,
            de.macro_prefix,
            de.escape_prefix,
        )
    }

    /// Get the field associated with the passed value (if there is one).
    pub fn get_field(&self, ch: &char) -> Option<&ConfigField> {
        self.values_to_fields.get(ch)
    }

    /// Get the value associated with the passed field.
    pub fn get_value(&self, field: &ConfigField) -> &char {
        self.fields_to_values
            .get(field)
            .expect("Every field should be set.")
    }
}
