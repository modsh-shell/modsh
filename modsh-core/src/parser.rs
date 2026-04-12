//! Parser — Builds AST from token stream

use crate::lexer::{Operator, Token};
use thiserror::Error;

/// Errors that can occur during parsing
#[derive(Error, Debug)]
pub enum ParseError {
    /// Unexpected token encountered
    #[error("unexpected token: {0:?}")]
    Unexpected(Token),
    /// Unexpected end of input
    #[error("unexpected end of input")]
    UnexpectedEof,
    /// Expected a different token
    #[error("expected {expected}, got {got:?}")]
    Expected {
        /// Expected token description
        expected: String,
        /// Actual token received
        got: Token,
    },
}

/// AST node types for POSIX shell commands
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Simple command: cmd arg1 arg2
    Simple(SimpleCommand),
    /// Pipeline: cmd1 | cmd2 | cmd3
    Pipeline(Vec<Command>),
    /// AND list: cmd1 && cmd2
    And(Box<Command>, Box<Command>),
    /// OR list: cmd1 || cmd2
    Or(Box<Command>, Box<Command>),
    /// List with separator: cmd1 ; cmd2
    List(Box<Command>, Box<Command>),
    /// Background command: cmd &
    Background(Box<Command>),
    /// Subshell: ( commands )
    Subshell(Box<Command>),
    /// Group: { commands; }
    Group(Box<Command>),
}

/// A simple command with words and redirects
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SimpleCommand {
    /// Words (command name and arguments)
    pub words: Vec<String>,
    /// Redirections
    pub redirects: Vec<Redirect>,
}

/// Redirection specification
#[derive(Debug, Clone, PartialEq)]
pub struct Redirect {
    /// File descriptor (None means default)
    pub fd: Option<u32>,
    /// Redirection type
    pub kind: RedirectKind,
    /// Target (file or variable)
    pub target: String,
}

/// Redirection kinds
#[derive(Debug, Clone, PartialEq)]
pub enum RedirectKind {
    /// Input: <
    Input,
    /// Output: >
    Output,
    /// Append: >>
    Append,
    /// Here-document: <<
    Heredoc,
    /// Here-string: <<<
    Herestring,
    /// Read-write: <>
    ReadWrite,
}

/// Parser for POSIX shell syntax
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    /// Create a new parser from a token stream
    #[must_use]
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parse the entire input into a command AST
    ///
    /// # Errors
    /// Returns an error if the token stream contains unexpected tokens
    pub fn parse(&mut self) -> Result<Command, ParseError> {
        self.parse_list()
    }

    fn parse_list(&mut self) -> Result<Command, ParseError> {
        let left = self.parse_pipeline()?;

        match self.peek() {
            Token::Operator(Operator::Semicolon) => {
                self.advance();
                let right = self.parse_list()?;
                Ok(Command::List(Box::new(left), Box::new(right)))
            }
            Token::Operator(Operator::Background) => {
                self.advance();
                Ok(Command::Background(Box::new(left)))
            }
            Token::Operator(Operator::And) => {
                self.advance();
                let right = self.parse_list()?;
                Ok(Command::And(Box::new(left), Box::new(right)))
            }
            Token::Operator(Operator::Or) => {
                self.advance();
                let right = self.parse_list()?;
                Ok(Command::Or(Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
        }
    }

    fn parse_pipeline(&mut self) -> Result<Command, ParseError> {
        let mut commands = vec![self.parse_command()?];

        while matches!(self.peek(), Token::Operator(Operator::Pipe)) {
            self.advance();
            commands.push(self.parse_command()?);
        }

        if commands.len() == 1 {
            Ok(commands.into_iter().next().unwrap())
        } else {
            Ok(Command::Pipeline(commands))
        }
    }

    fn parse_command(&mut self) -> Result<Command, ParseError> {
        match self.peek() {
            Token::Operator(Operator::LParen) => {
                self.advance();
                let cmd = self.parse_list()?;
                self.expect_operator(Operator::RParen)?;
                Ok(Command::Subshell(Box::new(cmd)))
            }
            Token::Operator(Operator::LBrace) => {
                self.advance();
                let cmd = self.parse_list()?;
                self.expect_operator(Operator::RBrace)?;
                Ok(Command::Group(Box::new(cmd)))
            }
            _ => self.parse_simple_command(),
        }
    }

    fn parse_simple_command(&mut self) -> Result<Command, ParseError> {
        let mut cmd = SimpleCommand::default();

        loop {
            match self.peek() {
                Token::Word(w) => {
                    cmd.words.push(w.clone());
                    self.advance();
                }
                Token::Redirect(r) => {
                    let redirect = self.convert_redirect(r.clone())?;
                    cmd.redirects.push(redirect);
                    self.advance();
                }
                _ => break,
            }
        }

        if cmd.words.is_empty() && cmd.redirects.is_empty() {
            return Err(ParseError::Unexpected(self.peek().clone()));
        }

        Ok(Command::Simple(cmd))
    }

    fn convert_redirect(&self, r: crate::lexer::Redirect) -> Result<Redirect, ParseError> {
        use crate::lexer::Redirect as LRedirect;

        // Check if we need to read a target word before moving `r`
        let needs_target = !matches!(r, LRedirect::Heredoc { .. });

        let (fd, kind, target) = match r {
            LRedirect::Input { fd } => (fd, RedirectKind::Input, String::new()),
            LRedirect::Output { fd } => (fd, RedirectKind::Output, String::new()),
            LRedirect::Append { fd } => (fd, RedirectKind::Append, String::new()),
            LRedirect::Heredoc { delimiter } => (None, RedirectKind::Heredoc, delimiter),
            LRedirect::Herestring => (None, RedirectKind::Herestring, String::new()),
            LRedirect::ReadWrite { fd } => (fd, RedirectKind::ReadWrite, String::new()),
        };

        if needs_target {
            match self.peek_next() {
                Token::Word(t) => Ok(Redirect {
                    fd,
                    kind,
                    target: t.clone(),
                }),
                _ => Err(ParseError::Expected {
                    expected: "redirect target".to_string(),
                    got: self.peek_next().clone(),
                }),
            }
        } else {
            Ok(Redirect { fd, kind, target })
        }
    }

    fn expect_operator(&mut self, op: Operator) -> Result<(), ParseError> {
        match self.peek() {
            Token::Operator(o) if *o == op => {
                self.advance();
                Ok(())
            }
            token => Err(ParseError::Expected {
                expected: format!("{op:?}"),
                got: token.clone(),
            }),
        }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn peek_next(&self) -> &Token {
        self.tokens.get(self.pos + 1).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }
}

/// Parse an input string into a command AST
///
/// # Errors
/// Returns an error if the input contains invalid syntax
pub fn parse(input: &str) -> Result<Command, ParseError> {
    let tokens = crate::lexer::tokenize(input).map_err(|_e| ParseError::Expected {
        expected: "valid token".to_string(),
        got: Token::Eof,
    })?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        let cmd = parse("echo hello").unwrap();
        match cmd {
            Command::Simple(s) => {
                assert_eq!(s.words, vec!["echo", "hello"]);
            }
            _ => panic!("Expected simple command"),
        }
    }

    #[test]
    fn test_pipeline() {
        let cmd = parse("ls | wc -l").unwrap();
        match cmd {
            Command::Pipeline(commands) => {
                assert_eq!(commands.len(), 2);
            }
            _ => panic!("Expected pipeline"),
        }
    }
}
