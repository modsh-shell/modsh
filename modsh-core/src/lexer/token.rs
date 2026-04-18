//! Token types for the POSIX shell lexer

use thiserror::Error;

/// Errors that can occur during lexing
#[derive(Error, Debug)]
pub enum LexError {
    /// Unexpected character encountered
    #[error("unexpected character: {0}")]
    Unexpected(char),
    /// Unterminated quote string
    #[error("unterminated quote")]
    UnterminatedQuote,
    /// Unterminated heredoc delimiter
    #[error("unterminated heredoc")]
    UnterminatedHeredoc,
}

/// Token types for POSIX shell syntax
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// An unquoted word (command name, argument, variable name)
    Word(String),
    /// A single-quoted string ('literal content')
    SingleQuoted(String),
    /// A double-quoted string ("allows $expansion")
    DoubleQuoted(String),
    /// An operator (|, &&, ||, ;, etc.)
    Operator(Operator),
    /// A redirection (<, >, >>, 2>, etc.)
    Redirect(Redirect),
    /// A comment (content after #)
    Comment(String),
    /// End of input
    Eof,
}

/// Shell operators
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operator {
    /// Pipe: |
    Pipe,
    /// AND list: &&
    And,
    /// OR list: ||
    Or,
    /// Semicolon: ;
    Semicolon,
    /// Background: &
    Background,
    /// Logical NOT: !
    Bang,
    /// Left parenthesis: (
    LParen,
    /// Right parenthesis: )
    RParen,
    /// Left brace: {
    LBrace,
    /// Right brace: }
    RBrace,
}

/// Redirection operators
#[derive(Debug, Clone, PartialEq)]
pub enum Redirect {
    /// Input redirection: <
    Input {
        /// File descriptor (None means stdin)
        fd: Option<u32>,
    },
    /// Output redirection: >
    Output {
        /// File descriptor (None means stdout)
        fd: Option<u32>,
    },
    /// Append output: >>
    Append {
        /// File descriptor (None means stdout)
        fd: Option<u32>,
    },
    /// Here-document: <<
    Heredoc {
        /// Heredoc delimiter string
        delimiter: String,
        /// Whether delimiter was quoted (suppresses expansion in body)
        quoted: bool,
        /// Heredoc body content (lines between delimiters)
        body: String,
    },
    /// Here-string: <<<
    Herestring {
        /// File descriptor (None means stdin)
        fd: Option<u32>,
        /// Content word for the here-string
        word: String,
    },
    /// Input/Output redirection: <>
    ReadWrite {
        /// File descriptor
        fd: Option<u32>,
    },
    /// Output to both stdout and stderr: &>
    OutputStdoutStderr,
    /// Append to both stdout and stderr: &>>
    AppendStdoutStderr,
}
