//! Lexer — Tokenizes POSIX shell syntax

use thiserror::Error;

/// Errors that can occur during lexing
#[derive(Error, Debug)]
pub enum LexError {
    #[error("unexpected character: {0}")]
    Unexpected(char),
    #[error("unterminated quote")]
    UnterminatedQuote,
    #[error("unterminated heredoc")]
    UnterminatedHeredoc,
}

/// Token types for POSIX shell syntax
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A word (command name, argument, variable name)
    Word(String),
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
    Input { fd: Option<u32> },
    /// Output redirection: >
    Output { fd: Option<u32> },
    /// Append output: >>
    Append { fd: Option<u32> },
    /// Here-document: <<
    Heredoc { delimiter: String },
    /// Here-string: <<<
    Herestring,
    /// Input/Output redirection: <>
    ReadWrite { fd: Option<u32> },
}

/// Lexer for POSIX shell syntax
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given input
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Get the next token from the input
    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace();

        if self.is_at_end() {
            return Ok(Token::Eof);
        }

        let ch = self.peek();

        // Handle comments
        if ch == '#' {
            return self.read_comment();
        }

        // Handle operators and redirections
        match ch {
            '|' => {
                self.advance();
                if self.peek() == '|' {
                    self.advance();
                    Ok(Token::Operator(Operator::Or))
                } else {
                    Ok(Token::Operator(Operator::Pipe))
                }
            }
            '&' => {
                self.advance();
                if self.peek() == '&' {
                    self.advance();
                    Ok(Token::Operator(Operator::And))
                } else {
                    Ok(Token::Operator(Operator::Background))
                }
            }
            ';' => {
                self.advance();
                Ok(Token::Operator(Operator::Semicolon))
            }
            '(' => {
                self.advance();
                Ok(Token::Operator(Operator::LParen))
            }
            ')' => {
                self.advance();
                Ok(Token::Operator(Operator::RParen))
            }
            '{' => {
                self.advance();
                Ok(Token::Operator(Operator::LBrace))
            }
            '}' => {
                self.advance();
                Ok(Token::Operator(Operator::RBrace))
            }
            '!' => {
                self.advance();
                Ok(Token::Operator(Operator::Bang))
            }
            '<' | '>' => self.read_redirect(),
            '\'' | '"' => self.read_quoted_word(),
            _ => self.read_word(),
        }
    }

    fn skip_whitespace(&mut self) {
        while !self.is_at_end() && self.peek().is_ascii_whitespace() {
            self.advance();
        }
    }

    fn read_comment(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        while !self.is_at_end() && self.peek() != '\n' {
            self.advance();
        }
        Ok(Token::Comment(self.input[start..self.pos].to_string()))
    }

    fn read_redirect(&mut self) -> Result<Token, LexError> {
        let ch = self.peek();
        self.advance();

        match ch {
            '<' => {
                if self.peek() == '<' {
                    self.advance();
                    if self.peek() == '<' {
                        self.advance();
                        Ok(Token::Redirect(Redirect::Herestring))
                    } else {
                        let delimiter = self.read_delimiter();
                        Ok(Token::Redirect(Redirect::Heredoc { delimiter }))
                    }
                } else if self.peek() == '>' {
                    self.advance();
                    Ok(Token::Redirect(Redirect::ReadWrite { fd: None }))
                } else {
                    Ok(Token::Redirect(Redirect::Input { fd: None }))
                }
            }
            '>' => {
                if self.peek() == '>' {
                    self.advance();
                    Ok(Token::Redirect(Redirect::Append { fd: None }))
                } else {
                    Ok(Token::Redirect(Redirect::Output { fd: None }))
                }
            }
            _ => unreachable!(),
        }
    }

    fn read_delimiter(&mut self) -> String {
        self.skip_whitespace();
        let start = self.pos;
        while !self.is_at_end() && !self.peek().is_ascii_whitespace() {
            self.advance();
        }
        self.input[start..self.pos].to_string()
    }

    fn read_quoted_word(&mut self) -> Result<Token, LexError> {
        let quote = self.peek();
        self.advance();

        let start = self.pos;
        while !self.is_at_end() && self.peek() != quote {
            if self.peek() == '\\' && quote == '"' {
                self.advance(); // skip backslash in double quotes
                if !self.is_at_end() {
                    self.advance();
                }
            } else {
                self.advance();
            }
        }

        if self.is_at_end() {
            return Err(LexError::UnterminatedQuote);
        }

        let word = self.input[start..self.pos].to_string();
        self.advance(); // consume closing quote

        Ok(Token::Word(word))
    }

    fn read_word(&mut self) -> Result<Token, LexError> {
        let start = self.pos;

        while !self.is_at_end() {
            let ch = self.peek();
            if ch.is_ascii_whitespace()
                || matches!(ch, '|' | '&' | ';' | '(' | ')' | '<' | '>' | '{' | '}' | '#')
            {
                break;
            }

            if ch == '\\' {
                self.advance();
                if !self.is_at_end() {
                    self.advance();
                }
            } else if ch == '\'' || ch == '"' {
                // Embedded quotes - read them as part of the word
                let quote = ch;
                self.advance();
                while !self.is_at_end() && self.peek() != quote {
                    self.advance();
                }
                if !self.is_at_end() {
                    self.advance(); // closing quote
                }
            } else {
                self.advance();
            }
        }

        if start == self.pos {
            return Err(LexError::Unexpected(self.peek()));
        }

        Ok(Token::Word(self.input[start..self.pos].to_string()))
    }

    fn peek(&self) -> char {
        self.input[self.pos..].chars().next().unwrap_or('\0')
    }

    fn advance(&mut self) {
        let ch = self.peek();
        if ch != '\0' {
            self.pos += ch.len_utf8();
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.input.len()
    }
}

/// Tokenize the entire input into a vector of tokens
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let mut lexer = Lexer::new(input);
    let mut tokens = Vec::new();

    loop {
        let token = lexer.next_token()?;
        if token == Token::Eof {
            tokens.push(token);
            break;
        }
        tokens.push(token);
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_word() {
        let tokens = tokenize("echo hello").unwrap();
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0], Token::Word(ref w) if w == "echo"));
        assert!(matches!(tokens[1], Token::Word(ref w) if w == "hello"));
    }

    #[test]
    fn test_pipe() {
        let tokens = tokenize("ls | wc -l").unwrap();
        assert!(matches!(tokens[1], Token::Operator(Operator::Pipe)));
    }

    #[test]
    fn test_single_quotes() {
        let tokens = tokenize("echo 'hello world'").unwrap();
        assert!(matches!(tokens[1], Token::Word(ref w) if w == "hello world"));
    }

    #[test]
    fn test_double_quotes() {
        let tokens = tokenize(r#"echo "hello world""#).unwrap();
        assert!(matches!(tokens[1], Token::Word(ref w) if w == "hello world"));
    }
}
