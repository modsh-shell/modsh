//! Syntax highlighter — Real-time token coloring

use crossterm::style::{Color, ResetColor, SetForegroundColor};
use modsh_core::lexer::{tokenize, Operator, Redirect, Token};
use std::fmt::Write;

/// Style for a token
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Foreground color
    pub fg: Color,
}

impl Style {
    /// Default command style
    pub const COMMAND: Self = Self { fg: Color::Green };
    /// Unknown command style
    pub const UNKNOWN: Self = Self { fg: Color::Red };
    /// Argument style
    pub const ARG: Self = Self { fg: Color::Reset };
    /// String style
    pub const STRING: Self = Self { fg: Color::Yellow };
    /// Operator style
    pub const OPERATOR: Self = Self { fg: Color::Blue };
    /// Comment style
    pub const COMMENT: Self = Self {
        fg: Color::DarkGrey,
    };
    /// Error style
    pub const ERROR: Self = Self { fg: Color::Red };
}

/// Syntax highlighter
pub struct Highlighter {
    /// Whether to highlight commands based on PATH existence
    pub check_path: bool,
}

impl Highlighter {
    /// Create a new highlighter
    #[must_use]
    pub fn new() -> Self {
        Self { check_path: true }
    }

    /// Highlight a line of input
    #[must_use]
    pub fn highlight(&self, input: &str) -> String {
        let Ok(tokens) = tokenize(input) else {
            return input.to_string();
        };

        let mut result = String::new();
        let mut is_first = true;

        for token in &tokens {
            if matches!(token, Token::Eof) {
                break;
            }

            let style = self.style_for_token(token, is_first);
            let text = token_text(token);

            // Apply style
            let _ = write!(
                result,
                "{}{}{}",
                SetForegroundColor(style.fg),
                text,
                ResetColor
            );

            if matches!(token, Token::Word(_)) {
                is_first = false;
            }
            if matches!(
                token,
                Token::Operator(
                    Operator::Pipe
                        | Operator::Semicolon
                        | Operator::And
                        | Operator::Or
                        | Operator::Background
                )
            ) {
                is_first = true;
            }
        }

        result
    }

    fn style_for_token(&self, token: &Token, is_command: bool) -> Style {
        match token {
            Token::Word(word) => {
                if is_command && self.check_path {
                    if is_valid_command(word) {
                        Style::COMMAND
                    } else {
                        Style::UNKNOWN
                    }
                } else {
                    Style::ARG
                }
            }
            Token::Operator(_) | Token::Redirect(_) => Style::OPERATOR,
            Token::Comment(_) => Style::COMMENT,
            Token::SingleQuoted(_) | Token::DoubleQuoted(_) | Token::Eof => Style::ARG,
        }
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

fn token_text(token: &Token) -> String {
    match token {
        Token::Word(w) => w.clone(),
        Token::Operator(op) => match op {
            Operator::Pipe => "|".to_string(),
            Operator::And => "&&".to_string(),
            Operator::Or => "||".to_string(),
            Operator::Semicolon => ";".to_string(),
            Operator::Background => "&".to_string(),
            Operator::Bang => "!".to_string(),
            Operator::LParen => "(".to_string(),
            Operator::RParen => ")".to_string(),
            Operator::LBrace => "{".to_string(),
            Operator::RBrace => "}".to_string(),
        },
        Token::Redirect(r) => match r {
            Redirect::Input { .. } => "<".to_string(),
            Redirect::Output { .. } => ">".to_string(),
            Redirect::Append { .. } => ">>".to_string(),
            Redirect::Heredoc { .. } => "<<".to_string(),
            Redirect::Herestring { .. } => "<<<".to_string(),
            Redirect::ReadWrite { .. } => "<>".to_string(),
            Redirect::OutputStdoutStderr => "&>".to_string(),
            Redirect::AppendStdoutStderr => "&>>".to_string(),
        },
        Token::SingleQuoted(s) => format!("'{s}'"),
        Token::DoubleQuoted(s) => format!("\"{s}\""),
        Token::Comment(c) => format!("#{c}"),
        Token::Eof => String::new(),
    }
}

fn is_valid_command(cmd: &str) -> bool {
    // Builtins are always valid
    let builtins = [
        "cd", "pwd", "echo", "export", "unset", "env", "exit", "true", "false", "source", ".",
        "alias", "unalias", "read", "test", "[", "trap", "shift", "set", "return",
    ];
    if builtins.contains(&cmd) {
        return true;
    }

    // Check in PATH
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let candidate = std::path::PathBuf::from(dir).join(cmd);
            if candidate.exists() {
                return true;
            }
        }
    }

    false
}
