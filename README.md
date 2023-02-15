# **bfup**

[![Crate][crate_img]][crate]

Preprocessor for brainfuck-like languages.

It allows the user, like every decent preprocessor should,
to obfuscate their code with weird macros and obtuse syntax,
even being able to arrange the output code in a rectangle of set width.
 
**bfup** is meant to be used primarily on top of [brainfuck][bf] or
languages with brainfuck-like syntaxes, as it operates mainly on
single, utf-8 encoded characters. It is possible to wholly configure
the recognized operators *(characters)* and the defined preprocessor
directives' prefixes.

## Usage

### Basic usage:

```text
bfup [OPTIONS]... [FILE]
```

The list of all available flags can be seen by
using the `--help` flag.

### Behavior

The program tries to preprocess the provided file,
outputting only characters *(to stdin by default)*
specified as operators *([brainfuck operators][bf_ops] by default)*.
By default, the output is also arranged in a rectangle.

### Configuring

Recognized operators, as well as every directive character
used by the parser can be configured with corresponding 
command-line options or read from a [RON][ron] config file.

## Syntax

The preprocessor recognizes 5 basic types of '*tokens*':

| Token                                                  | Preprocessed as                                                                          |
|--------------------------------------------------------|------------------------------------------------------------------------------------------|
| Operators                                              | copied directly to the output                                                            |
| *Tokens* enclosed by `(` `)`                           | groups the *tokens* and treats them as a single token.                                   |
| `#` followed by a *number*                             | multiplies the next token *number* times                                                 |
| `$` followed by any *character*, followed by a *token* | defines a macro that substitutes every subsequent occurrence of *character* with *token* |
| `\`                                                    | skips the next character                                                                 |

## Example

Code evaluating to a [brainfuck][bf] program
that prints `Hello World!` followed by a newline:
```
$z([-]) $p(.z)

$H(#72+p)
$e(#101+p)
$l(#108+p)
$o(#111+p)
$,(#44+p)
$ (#32+p)
$W(#87+p)
$r(#114+p)
$d(#100+p)
$!(#33+p)
$/(#10+p)

Hello, World!/
```

## Caveats

Be wary, that ***every character*** can be defined as a macro; you can overwrite operators, preprocessor directive prefixes and even 
whitespace characters. For example:
```
$+(-) $
< $$([[[-]]])
+++$
```
evaluates to:
```
<---([[[-]]])
```
In addition, macro definitions take the very first valid token they encounter, this means that:
```
$m($z$a+([-])aa)
mz
```
evaluates to:
```
++[-]
```

## Contributing

I appreciate any bug report/contribution/criticism.

[bf]:https://en.wikipedia.org/wiki/Brainfuck
[bf_ops]:https://en.wikipedia.org/wiki/Brainfuck#Commands
[ron]:https://docs.rs/ron/latest/ron/

[crate]:https://crates.io/crates/bfup
[crate_img]:https://img.shields.io/crates/v/bfup.svg?logo=rust
