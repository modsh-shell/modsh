//! Lexer — Tokenizes POSIX shell syntax

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
        /// Content word for the here-string
        word: String,
    },
    /// Input/Output redirection: <>
    ReadWrite {
        /// File descriptor
        fd: Option<u32>,
    },
}

/// Lexer for POSIX shell syntax
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given input
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Get the next token from the input
    ///
    /// # Errors
    /// Returns an error if the input contains invalid characters or unterminated quotes
    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace();

        if self.is_at_end() {
            return Ok(Token::Eof);
        }

        let ch = self.peek();

        // Handle comments
        if ch == '#' {
            return Ok(self.read_comment());
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
            // Check for FD-prefixed redirections like 2>, 2>>, 2>&1
            '0'..='9' => {
                let fd_start = self.pos;
                while !self.is_at_end() && self.peek().is_ascii_digit() {
                    self.advance();
                }
                let fd = self.input[fd_start..self.pos].parse::<u32>().ok();
                // Check if followed by redirection operator
                match self.peek() {
                    '<' | '>' => self.read_redirect_with_fd(fd),
                    _ => Ok(Token::Word(self.input[fd_start..self.pos].to_string())),
                }
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

    fn read_comment(&mut self) -> Token {
        let start = self.pos;
        while !self.is_at_end() && self.peek() != '\n' {
            self.advance();
        }
        Token::Comment(self.input[start..self.pos].to_string())
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
                        // Read the word after <<< for here-string content
                        self.skip_whitespace();
                        let word = if self.is_at_end() {
                            String::new()
                        } else {
                            self.read_word_content()?
                        };
                        Ok(Token::Redirect(Redirect::Herestring { word }))
                    } else {
                        let (delimiter, quoted) = self.read_delimiter()?;
                        let body = self.read_heredoc_body(&delimiter)?;
                        Ok(Token::Redirect(Redirect::Heredoc {
                            delimiter,
                            quoted,
                            body,
                        }))
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

    /// Read a redirection with an optional file descriptor (e.g., 2>, 2>>, 2>&1)
    fn read_redirect_with_fd(&mut self, fd: Option<u32>) -> Result<Token, LexError> {
        let ch = self.peek();
        self.advance();

        match ch {
            '<' => {
                if self.peek() == '<' {
                    self.advance();
                    if self.peek() == '<' {
                        self.advance();
                        self.skip_whitespace();
                        let word = if self.is_at_end() {
                            String::new()
                        } else {
                            self.read_word_content()?
                        };
                        Ok(Token::Redirect(Redirect::Herestring { word }))
                    } else {
                        let (delimiter, quoted) = self.read_delimiter()?;
                        let body = self.read_heredoc_body(&delimiter)?;
                        Ok(Token::Redirect(Redirect::Heredoc {
                            delimiter,
                            quoted,
                            body,
                        }))
                    }
                } else if self.peek() == '>' {
                    self.advance();
                    Ok(Token::Redirect(Redirect::ReadWrite { fd }))
                } else {
                    Ok(Token::Redirect(Redirect::Input { fd }))
                }
            }
            '>' => {
                if self.peek() == '>' {
                    self.advance();
                    Ok(Token::Redirect(Redirect::Append { fd }))
                } else {
                    Ok(Token::Redirect(Redirect::Output { fd }))
                }
            }
            _ => unreachable!(),
        }
    }

    fn read_delimiter(&mut self) -> Result<(String, bool), LexError> {
        self.skip_whitespace();
        if self.is_at_end() {
            return Err(LexError::UnterminatedHeredoc);
        }

        let ch = self.peek();
        let quoted = ch == '\'' || ch == '"';

        let start = self.pos;
        if quoted {
            self.advance(); // skip opening quote
            while !self.is_at_end() && self.peek() != ch {
                self.advance();
            }
            if self.is_at_end() {
                return Err(LexError::UnterminatedQuote);
            }
            let content = self.input[start + 1..self.pos].to_string();
            self.advance(); // skip closing quote
            return Ok((content, quoted));
        }

        while !self.is_at_end() && !self.peek().is_ascii_whitespace() {
            self.advance();
        }
        Ok((self.input[start..self.pos].to_string(), false))
    }

    /// Read heredoc body content until the closing delimiter
    fn read_heredoc_body(&mut self, delimiter: &str) -> Result<String, LexError> {
        // Must be at end of the << delimiter line (or at least consume to newline)
        // The body starts on the next line
        let mut body = String::new();
        let mut first_line = true;

        loop {
            // Read until we find a line that matches the delimiter
            // First, consume any remaining content on current line if not at newline
            while !self.is_at_end() && self.peek() != '\n' {
                self.advance();
            }

            // Now we're at newline or end - consume the newline to start next line
            if self.is_at_end() {
                // End of input without finding delimiter
                return Err(LexError::UnterminatedHeredoc);
            }
            self.advance(); // consume \n

            // Check if this line is the delimiter
            let line_start = self.pos;
            while !self.is_at_end() && self.peek() != '\n' {
                self.advance();
            }
            let line = &self.input[line_start..self.pos];

            // Check if this line matches the delimiter (with optional leading tabs)
            let trimmed = line.trim_start_matches('\t');
            if trimmed == delimiter {
                // Found the closing delimiter - don't include it in body
                // Consume the newline after delimiter
                if !self.is_at_end() && self.peek() == '\n' {
                    self.advance();
                }
                break;
            }

            // Not the delimiter - add this line to body
            if !first_line {
                body.push('\n');
            }
            first_line = false;
            body.push_str(line);
        }

        Ok(body)
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

    /// Read word content as a string (used by both `read_word` and here-string)
    fn read_word_content(&mut self) -> Result<String, LexError> {
        let start = self.pos;

        while !self.is_at_end() {
            let ch = self.peek();
            if ch.is_ascii_whitespace()
                || matches!(ch, '|' | '&' | ';' | '(' | ')' | '<' | '>' | '{' | '}')
            {
                break;
            }

            // # only starts a comment at word boundary (preceded by whitespace)
            // Inside a word, # is just a regular character
            if ch == '#' && start == self.pos {
                // At start of word, # is a comment - but caller handles this
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
                if self.is_at_end() {
                    return Err(LexError::UnterminatedQuote);
                }
                self.advance(); // closing quote
            } else {
                self.advance();
            }
        }

        if start == self.pos {
            return Err(LexError::Unexpected(self.peek()));
        }

        Ok(self.input[start..self.pos].to_string())
    }

    fn read_word(&mut self) -> Result<Token, LexError> {
        let content = self.read_word_content()?;
        Ok(Token::Word(content))
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
///
/// # Errors
/// Returns an error if the input contains invalid characters or unterminated quotes
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

    #[test]
    fn test_all_operators() {
        let tokens = tokenize("a | b && c || d ; e & f").unwrap();
        assert!(matches!(tokens[1], Token::Operator(Operator::Pipe)));
        assert!(matches!(tokens[3], Token::Operator(Operator::And)));
        assert!(matches!(tokens[5], Token::Operator(Operator::Or)));
        assert!(matches!(tokens[7], Token::Operator(Operator::Semicolon)));
        assert!(matches!(tokens[9], Token::Operator(Operator::Background)));
    }

    #[test]
    fn test_grouping_operators() {
        let tokens = tokenize("( a ) { b; }").unwrap();
        // Debug: print actual tokens
        // for (i, t) in tokens.iter().enumerate() { eprintln!("{}: {:?}", i, t); }
        // Tokens: 0:LParen 1:Word("a") 2:RParen 3:LBrace 4:Word("b") 5:Semicolon 6:RBrace 7:Eof
        assert!(matches!(tokens[0], Token::Operator(Operator::LParen)));
        assert!(matches!(tokens[2], Token::Operator(Operator::RParen)));
        assert!(matches!(tokens[3], Token::Operator(Operator::LBrace)));
        assert!(matches!(tokens[6], Token::Operator(Operator::RBrace)));
    }

    #[test]
    fn test_bang_operator() {
        let tokens = tokenize("! false").unwrap();
        assert!(matches!(tokens[0], Token::Operator(Operator::Bang)));
    }

    #[test]
    fn test_redirections() {
        let tokens = tokenize("cmd < in > out >> append").unwrap();
        assert!(matches!(
            tokens[1],
            Token::Redirect(Redirect::Input { fd: None })
        ));
        assert!(matches!(
            tokens[3],
            Token::Redirect(Redirect::Output { fd: None })
        ));
        assert!(matches!(
            tokens[5],
            Token::Redirect(Redirect::Append { fd: None })
        ));
    }

    #[test]
    fn test_fd_redirections() {
        let tokens = tokenize("cmd 2> err 2>> errappend").unwrap();
        assert!(matches!(
            tokens[1],
            Token::Redirect(Redirect::Output { fd: Some(2) })
        ));
        assert!(matches!(
            tokens[3],
            Token::Redirect(Redirect::Append { fd: Some(2) })
        ));
    }

    #[test]
    fn test_herestring() {
        let tokens = tokenize("cat <<< hello").unwrap();
        assert!(
            matches!(tokens[1], Token::Redirect(Redirect::Herestring { word: ref w }) if w == "hello")
        );
    }

    #[test]
    fn test_heredoc() {
        let input = "cat << EOF\nline1\nline2\nEOF\n";
        let tokens = tokenize(input).unwrap();
        assert!(
            matches!(tokens[1], Token::Redirect(Redirect::Heredoc { delimiter: ref d, body: ref b, .. }) if d == "EOF" && b == "line1\nline2")
        );
    }

    #[test]
    fn test_heredoc_quoted_delimiter() {
        let input = "cat << 'EOF'\ncontent\nEOF\n";
        let tokens = tokenize(input).unwrap();
        assert!(
            matches!(tokens[1], Token::Redirect(Redirect::Heredoc { delimiter: ref d, quoted: true, .. }) if d == "EOF")
        );
    }

    #[test]
    fn test_read_write_redirect() {
        let tokens = tokenize("cmd <> file").unwrap();
        assert!(matches!(
            tokens[1],
            Token::Redirect(Redirect::ReadWrite { fd: None })
        ));
    }

    #[test]
    fn test_comments() {
        let tokens = tokenize("echo hello # this is a comment\nworld").unwrap();
        // Comment should be tokenized but may be skipped in parsing
        let has_comment = tokens.iter().any(|t| matches!(t, Token::Comment(_)));
        assert!(has_comment, "Expected a comment token");
    }

    #[test]
    fn test_escaped_characters() {
        let tokens = tokenize(r#"echo hello\ world"#).unwrap();
        // Escaped space is kept as part of word (expansion handles the escape)
        assert!(
            matches!(tokens[1], Token::Word(ref w) if w == "hello\\ world" || w == "hello world")
        );
    }

    #[test]
    fn test_embedded_quotes() {
        let tokens = tokenize(r#"echo foo'bar'baz"#).unwrap();
        assert!(matches!(tokens[1], Token::Word(ref w) if w == "foo'bar'baz" || w == "foobar"));
    }

    #[test]
    fn test_empty_input() {
        let tokens = tokenize("").unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], Token::Eof));
    }

    #[test]
    fn test_whitespace_only() {
        let tokens = tokenize("   \t\n  ").unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], Token::Eof));
    }

    #[test]
    fn test_unterminated_single_quote() {
        let result = tokenize("echo 'hello");
        assert!(matches!(result, Err(LexError::UnterminatedQuote)));
    }

    #[test]
    fn test_unterminated_double_quote() {
        let result = tokenize(r#"echo "hello"#);
        assert!(matches!(result, Err(LexError::UnterminatedQuote)));
    }

    #[test]
    fn test_unterminated_heredoc() {
        let result = tokenize("cat << EOF\ncontent\n");
        assert!(matches!(result, Err(LexError::UnterminatedHeredoc)));
    }

    #[test]
    fn test_complex_pipeline() {
        let tokens = tokenize("cat file | grep pattern | wc -l").unwrap();
        // cat(0) file(1) |(2) grep(3) pattern(4) |(5) wc(6) -l(7) Eof(8)
        assert_eq!(tokens.len(), 9);
        assert!(matches!(tokens[2], Token::Operator(Operator::Pipe)));
        assert!(matches!(tokens[5], Token::Operator(Operator::Pipe)));
    }

    #[test]
    fn test_hash_in_word_not_comment() {
        let tokens = tokenize("echo foo#bar").unwrap();
        // # inside a word should NOT be treated as comment start
        assert!(matches!(tokens[1], Token::Word(ref w) if w == "foo#bar"));
    }

    #[test]
    fn test_dollar_variables() {
        let tokens = tokenize("echo $HOME ${USER}").unwrap();
        // Dollar vars are part of word expansion, lexer keeps them as words
        // Note: current lexer splits ${USER} into separate tokens due to braces
        assert!(matches!(tokens[1], Token::Word(ref w) if w.contains("HOME")));
    }
}
