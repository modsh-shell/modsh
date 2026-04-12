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
    /// If statement: if cmd; then cmds; elif cmd; then cmds; else cmds; fi
    If(IfClause),
    /// For loop: for var in words; do cmds; done
    For(ForLoop),
    /// While loop: while cmd; do cmds; done
    While(WhileLoop),
    /// Case statement: case word in patterns) cmds;; esac
    Case(CaseStatement),
    /// Function definition: name() { body; } or function name { body; }
    Function(FunctionDefinition),
}

/// Function definition
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDefinition {
    /// Function name
    pub name: String,
    /// Function body (typically a group or compound command)
    pub body: Box<Command>,
}

/// If clause with optional elif/else
#[derive(Debug, Clone, PartialEq)]
pub struct IfClause {
    /// Condition command
    pub condition: Box<Command>,
    /// Commands to execute if condition succeeds
    pub then_branch: Box<Command>,
    /// Optional elif clauses
    pub elif_branches: Vec<(Box<Command>, Box<Command>)>,
    /// Optional else branch
    pub else_branch: Option<Box<Command>>,
}

/// For loop
#[derive(Debug, Clone, PartialEq)]
pub struct ForLoop {
    /// Loop variable name
    pub var: String,
    /// Words to iterate over (empty means "$@")
    pub words: Vec<String>,
    /// Body commands
    pub body: Box<Command>,
}

/// While loop
#[derive(Debug, Clone, PartialEq)]
pub struct WhileLoop {
    /// Condition command
    pub condition: Box<Command>,
    /// Body commands
    pub body: Box<Command>,
}

/// Case statement
#[derive(Debug, Clone, PartialEq)]
pub struct CaseStatement {
    /// Word to match
    pub word: String,
    /// Case clauses (pattern, commands)
    pub clauses: Vec<(Vec<String>, Box<Command>)>,
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
    /// Returns an error if the token stream contains unexpected tokens or if
    /// there are leftover tokens after parsing a valid command
    pub fn parse(&mut self) -> Result<Command, ParseError> {
        // Handle empty or comment-only input
        self.skip_comments();
        if matches!(self.peek(), Token::Eof) {
            return Err(ParseError::UnexpectedEof);
        }

        let cmd = self.parse_list()?;

        // Check for leftover tokens
        self.skip_comments();
        if !matches!(self.peek(), Token::Eof) {
            return Err(ParseError::Unexpected(self.peek().clone()));
        }

        Ok(cmd)
    }

    /// Skip over comment tokens
    fn skip_comments(&mut self) {
        while matches!(self.peek(), Token::Comment(_)) {
            self.advance();
        }
    }

    fn parse_list(&mut self) -> Result<Command, ParseError> {
        self.parse_list_until(&[])
    }

    /// Check if current token is a terminator word
    fn is_terminator_word(&self, terminators: &[&str]) -> bool {
        matches!(self.peek(), Token::Word(w) if terminators.contains(&w.as_str()))
    }

    /// Check if current token is a terminator operator (RBrace, RParen, Semicolon)
    fn is_terminator_op(&self, terminators: &[&str]) -> bool {
        match self.peek() {
            Token::Operator(Operator::RBrace) if terminators.contains(&"}") => true,
            Token::Operator(Operator::RParen) if terminators.contains(&")") => true,
            Token::Operator(Operator::Semicolon) if terminators.contains(&";") => true,
            _ => false,
        }
    }

    /// Parse a list stopping when we hit a terminator word or operator
    fn parse_list_until(&mut self, terminators: &[&str]) -> Result<Command, ParseError> {
        // Check for terminator at start
        if self.is_terminator_word(terminators) || self.is_terminator_op(terminators) {
            return Err(ParseError::Unexpected(self.peek().clone()));
        }

        let left = self.parse_and_or_until(terminators)?;

        match self.peek() {
            // Stop if we hit a terminator word or operator
            Token::Word(w) if terminators.contains(&w.as_str()) => Ok(left),
            Token::Operator(Operator::RBrace) if terminators.contains(&"}") => Ok(left),
            Token::Operator(Operator::RParen) if terminators.contains(&")") => Ok(left),
            Token::Operator(Operator::Semicolon) if terminators.contains(&";") => Ok(left),
            Token::Operator(Operator::Semicolon) => {
                self.advance();
                // After semicolon, check if next token is a terminator
                if self.is_terminator_word(terminators) || self.is_terminator_op(terminators) {
                    return Ok(left);
                }
                let right = self.parse_list_until(terminators)?;
                Ok(Command::List(Box::new(left), Box::new(right)))
            }
            Token::Operator(Operator::Background) => {
                self.advance();
                let bg_cmd = Command::Background(Box::new(left));
                // After &, check if next token is a terminator
                if self.is_terminator_word(terminators) || self.is_terminator_op(terminators) {
                    return Ok(bg_cmd);
                }
                match self.peek() {
                    Token::Operator(Operator::Semicolon) => {
                        self.advance();
                        let right = self.parse_list_until(terminators)?;
                        Ok(Command::List(Box::new(bg_cmd), Box::new(right)))
                    }
                    _ => Ok(bg_cmd),
                }
            }
            _ => Ok(left),
        }
    }

    /// Parse AND/OR lists with left-associativity
    fn parse_and_or(&mut self) -> Result<Command, ParseError> {
        self.parse_and_or_until(&[])
    }

    fn parse_and_or_until(&mut self, terminators: &[&str]) -> Result<Command, ParseError> {
        let mut left = self.parse_pipeline_until(terminators)?;

        loop {
            match self.peek() {
                // Stop at terminator words
                Token::Word(w) if terminators.contains(&w.as_str()) => break,
                Token::Operator(Operator::And) => {
                    self.advance();
                    let right = self.parse_pipeline_until(terminators)?;
                    left = Command::And(Box::new(left), Box::new(right));
                }
                Token::Operator(Operator::Or) => {
                    self.advance();
                    let right = self.parse_pipeline_until(terminators)?;
                    left = Command::Or(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }

        Ok(left)
    }

    fn parse_pipeline(&mut self) -> Result<Command, ParseError> {
        self.parse_pipeline_until(&[])
    }

    fn parse_pipeline_until(&mut self, terminators: &[&str]) -> Result<Command, ParseError> {
        let mut commands = vec![self.parse_command_until(terminators)?];

        while matches!(self.peek(), Token::Operator(Operator::Pipe)) {
            self.advance();
            commands.push(self.parse_command_until(terminators)?);
        }

        if commands.len() == 1 {
            Ok(commands.into_iter().next().unwrap())
        } else {
            Ok(Command::Pipeline(commands))
        }
    }

    fn parse_command(&mut self) -> Result<Command, ParseError> {
        self.parse_command_until(&[])
    }

    fn parse_command_until(&mut self, terminators: &[&str]) -> Result<Command, ParseError> {
        // Check for terminator words first
        if let Token::Word(w) = self.peek() {
            if terminators.contains(&w.as_str()) {
                return Err(ParseError::Unexpected(self.peek().clone()));
            }
        }

        match self.peek() {
            Token::Operator(Operator::LParen) => {
                self.advance();
                // Create new terminators list with ")" added
                let mut paren_terminators = terminators.to_vec();
                paren_terminators.push(")");
                let cmd = self.parse_list_until(&paren_terminators)?;
                self.expect_operator(Operator::RParen)?;
                Ok(Command::Subshell(Box::new(cmd)))
            }
            Token::Operator(Operator::LBrace) => {
                self.advance();
                // Create new terminators list with "}" added
                let mut brace_terminators = terminators.to_vec();
                brace_terminators.push("}");
                let cmd = self.parse_list_until(&brace_terminators)?;
                self.expect_operator(Operator::RBrace)?;
                Ok(Command::Group(Box::new(cmd)))
            }
            Token::Word(w) => {
                let word = w.clone(); // Clone to avoid borrow issues
                // Check for compound command keywords (they take priority over terminators)
                match word.as_str() {
                    "if" => self.parse_if(),
                    "for" => self.parse_for(),
                    "while" => self.parse_while(),
                    "case" => self.parse_case(),
                    "function" => self.parse_function(),
                    _ => {
                        // Check for function definition: name() { ... }
                        if let Some(func) = self.try_parse_function_def(&word, terminators)? {
                            return Ok(func);
                        }
                        if terminators.contains(&word.as_str()) {
                            Err(ParseError::Unexpected(self.peek().clone()))
                        } else {
                            self.parse_simple_command()
                        }
                    }
                }
            }
            _ => self.parse_simple_command(),
        }
    }

    /// Parse if statement: if cmd; then cmds; [elif cmd; then cmds;]... [else cmds;] fi
    fn parse_if(&mut self) -> Result<Command, ParseError> {
        self.expect_word("if")?;
        let condition = Box::new(self.parse_list_until(&["then"])?);
        self.expect_word("then")?;
        let then_branch = Box::new(self.parse_list_until(&["elif", "else", "fi"])?);

        let mut elif_branches = Vec::new();
        let else_branch = loop {
            match self.peek() {
                Token::Word(w) if w == "elif" => {
                    self.advance();
                    let elif_cond = Box::new(self.parse_list_until(&["then"])?);
                    self.expect_word("then")?;
                    let elif_then = Box::new(self.parse_list_until(&["elif", "else", "fi"])?);
                    elif_branches.push((elif_cond, elif_then));
                }
                Token::Word(w) if w == "else" => {
                    self.advance();
                    let else_cmd = Box::new(self.parse_list_until(&["fi"])?);
                    self.expect_word("fi")?;
                    break Some(else_cmd);
                }
                Token::Word(w) if w == "fi" => {
                    self.advance();
                    break None;
                }
                _ => {
                    return Err(ParseError::Expected {
                        expected: "elif, else, or fi".to_string(),
                        got: self.peek().clone(),
                    });
                }
            }
        };

        Ok(Command::If(IfClause {
            condition,
            then_branch,
            elif_branches,
            else_branch,
        }))
    }

    /// Parse for loop: for var in words; do cmds; done
    fn parse_for(&mut self) -> Result<Command, ParseError> {
        self.expect_word("for")?;
        let var = self.expect_word_value()?;

        let words = if matches!(self.peek(), Token::Word(w) if w == "in") {
            self.advance();
            self.parse_for_words()?
        } else {
            Vec::new() // Empty means iterate over "$@"
        };

        self.expect_word("do")?;
        let body = Box::new(self.parse_list_until(&["done"])?);
        self.expect_word("done")?;

        Ok(Command::For(ForLoop { var, words, body }))
    }

    /// Parse words until semicolon or newline (for for-loop)
    fn parse_for_words(&mut self) -> Result<Vec<String>, ParseError> {
        let mut words = Vec::new();
        loop {
            match self.peek() {
                Token::Word(w) if w == "do" || w == ";" => break,
                Token::Operator(Operator::Semicolon) => {
                    self.advance();
                    break;
                }
                Token::Word(w) => {
                    words.push(w.clone());
                    self.advance();
                }
                _ => break,
            }
        }
        Ok(words)
    }

    /// Parse while loop: while cmd; do cmds; done
    fn parse_while(&mut self) -> Result<Command, ParseError> {
        self.expect_word("while")?;
        let condition = Box::new(self.parse_list_until(&["do"])?);
        self.expect_word("do")?;
        let body = Box::new(self.parse_list_until(&["done"])?);
        self.expect_word("done")?;

        Ok(Command::While(WhileLoop { condition, body }))
    }

    /// Parse case statement: case word in patterns) cmds;; esac
    fn parse_case(&mut self) -> Result<Command, ParseError> {
        self.expect_word("case")?;
        let word = self.expect_word_value()?;
        self.expect_word("in")?;

        let mut clauses = Vec::new();
        loop {
            match self.peek() {
                Token::Word(w) if w == "esac" => {
                    self.advance();
                    break;
                }
                _ => {
                    let patterns = self.parse_case_patterns()?;
                    // Parse body until we hit ;; (represented as ; in terminators, we check for double)
                    let body = Box::new(self.parse_list_until(&[";", "esac"])?);
                    // Expect the second ; of ;; or handle single ; before esac
                    self.expect_case_terminator_or_semicolon()?;
                    clauses.push((patterns, body));
                }
            }
        }

        Ok(Command::Case(CaseStatement { word, clauses }))
    }

    /// Parse function definition: function name { body; } or function name() { body; }
    fn parse_function(&mut self) -> Result<Command, ParseError> {
        self.expect_word("function")?;
        let name = self.expect_word_value()?;

        // Optional () after function name (bash style)
        if matches!(self.peek(), Token::Operator(Operator::LParen)) {
            self.advance();
            self.expect_operator(Operator::RParen)?;
        }

        let body = self.parse_function_body()?;
        Ok(Command::Function(FunctionDefinition { name, body }))
    }

    /// Try to parse function definition: name() { body; }
    /// Returns Ok(Some(func)) if it's a function def, Ok(None) if not
    fn try_parse_function_def(
        &mut self,
        name: &str,
        _terminators: &[&str],
    ) -> Result<Option<Command>, ParseError> {
        // Look ahead: next tokens should be () for function definition
        if !matches!(self.peek_next(), Token::Operator(Operator::LParen)) {
            return Ok(None);
        }

        // Check that name is not a reserved word
        let reserved = ["if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "in"];
        if reserved.contains(&name) {
            return Ok(None);
        }

        // It's a function definition!
        self.advance(); // consume name
        self.advance(); // consume (
        self.expect_operator(Operator::RParen)?;

        let body = self.parse_function_body()?;
        Ok(Some(Command::Function(FunctionDefinition {
            name: name.to_string(),
            body,
        })))
    }

    /// Parse function body (group command or any compound command)
    fn parse_function_body(&mut self) -> Result<Box<Command>, ParseError> {
        // Function body must be a compound command
        match self.peek() {
            Token::Operator(Operator::LBrace) => {
                // { commands; } form
                self.advance();
                let body = Box::new(self.parse_list_until(&["}"])?);
                self.expect_operator(Operator::RBrace)?;
                Ok(body)
            }
            Token::Word(w) => {
                match w.as_str() {
                    "if" => Ok(Box::new(self.parse_if()?)),
                    "for" => Ok(Box::new(self.parse_for()?)),
                    "while" => Ok(Box::new(self.parse_while()?)),
                    "case" => Ok(Box::new(self.parse_case()?)),
                    "function" => Ok(Box::new(self.parse_function()?)),
                    _ => Err(ParseError::Expected {
                        expected: "function body".to_string(),
                        got: self.peek().clone(),
                    }),
                }
            }
            _ => Err(ParseError::Expected {
                expected: "function body".to_string(),
                got: self.peek().clone(),
            }),
        }
    }

    /// Expect case clause terminator (;; or just ; before esac)
    fn expect_case_terminator_or_semicolon(&mut self) -> Result<(), ParseError> {
        match self.peek() {
            Token::Operator(Operator::Semicolon) => {
                self.advance();
                // Check for second ; (;; terminator)
                if matches!(self.peek(), Token::Operator(Operator::Semicolon)) {
                    self.advance();
                }
                Ok(())
            }
            Token::Word(w) if w == "esac" => {
                // Allow missing ;; before esac (optional in some shells)
                Ok(())
            }
            _ => Err(ParseError::Expected {
                expected: ";; or esac".to_string(),
                got: self.peek().clone(),
            }),
        }
    }

    /// Parse case patterns (pattern1 | pattern2)
    fn parse_case_patterns(&mut self) -> Result<Vec<String>, ParseError> {
        let mut patterns = Vec::new();
        loop {
            if let Token::Word(w) = self.peek() {
                patterns.push(w.clone());
                self.advance();
                match self.peek() {
                    Token::Operator(Operator::Pipe) => {
                        self.advance();
                        continue;
                    }
                    Token::Operator(Operator::RParen) => {
                        self.advance();
                        break;
                    }
                    _ => {
                        return Err(ParseError::Expected {
                            expected: "| or )".to_string(),
                            got: self.peek().clone(),
                        });
                    }
                }
            } else {
                return Err(ParseError::Expected {
                    expected: "pattern word".to_string(),
                    got: self.peek().clone(),
                });
            }
        }
        Ok(patterns)
    }

    /// Expect a specific word keyword
    fn expect_word(&mut self, expected: &str) -> Result<(), ParseError> {
        match self.peek() {
            Token::Word(w) if w == expected => {
                self.advance();
                Ok(())
            }
            token => Err(ParseError::Expected {
                expected: format!("'{expected}'"),
                got: token.clone(),
            }),
        }
    }

    /// Expect a word and return its value
    fn expect_word_value(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Token::Word(w) => {
                let val = w.clone();
                self.advance();
                Ok(val)
            }
            token => Err(ParseError::Expected {
                expected: "word".to_string(),
                got: token.clone(),
            }),
        }
    }

    /// Expect case clause terminator (;; or ;& or ;;&)
    fn expect_case_terminator(&mut self) -> Result<(), ParseError> {
        match self.peek() {
            Token::Operator(Operator::Semicolon) => {
                self.advance();
                // Check for second ;
                if matches!(self.peek(), Token::Operator(Operator::Semicolon)) {
                    self.advance();
                }
                Ok(())
            }
            _ => Err(ParseError::Expected {
                expected: ";;".to_string(),
                got: self.peek().clone(),
            }),
        }
    }

    fn parse_simple_command(&mut self) -> Result<Command, ParseError> {
        let mut cmd = SimpleCommand::default();

        loop {
            self.skip_comments();
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

    fn convert_redirect(&mut self, r: crate::lexer::Redirect) -> Result<Redirect, ParseError> {
        use crate::lexer::Redirect as LRedirect;

        // Check if we need to read a target word before moving `r`
        // Heredoc delimiter and Herestring word are already embedded in the token
        let needs_target = !matches!(r, LRedirect::Heredoc { .. } | LRedirect::Herestring { .. });

        let (fd, kind, target) = match r {
            LRedirect::Input { fd } => (fd, RedirectKind::Input, String::new()),
            LRedirect::Output { fd } => (fd, RedirectKind::Output, String::new()),
            LRedirect::Append { fd } => (fd, RedirectKind::Append, String::new()),
            LRedirect::Heredoc { delimiter: _, quoted: _, body } => {
                // Heredoc body is stored in target field for now
                // In full implementation, this would be handled specially
                (None, RedirectKind::Heredoc, body)
            }
            LRedirect::Herestring { word } => (None, RedirectKind::Herestring, word),
            LRedirect::ReadWrite { fd } => (fd, RedirectKind::ReadWrite, String::new()),
        };

        if needs_target {
            // Clone the token to avoid borrow issues
            let next_token = self.peek_next().clone();
            match next_token {
                Token::Word(t) => {
                    // Advance past the target word so it's not consumed as argument
                    self.advance();
                    Ok(Redirect {
                        fd,
                        kind,
                        target: t,
                    })
                }
                _ => Err(ParseError::Expected {
                    expected: "redirect target".to_string(),
                    got: next_token,
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

    #[test]
    fn test_if_statement() {
        let cmd = parse("if true; then echo yes; fi").unwrap();
        match cmd {
            Command::If(if_clause) => {
                // Condition should be 'true'
                assert!(matches!(*if_clause.then_branch, Command::Simple(ref s) if s.words == vec!["echo", "yes"]));
                assert!(if_clause.elif_branches.is_empty());
                assert!(if_clause.else_branch.is_none());
            }
            _ => panic!("Expected if command, got {:?}", cmd),
        }
    }

    #[test]
    fn test_if_else_statement() {
        let cmd = parse("if false; then echo yes; else echo no; fi").unwrap();
        match cmd {
            Command::If(if_clause) => {
                assert!(if_clause.else_branch.is_some());
            }
            _ => panic!("Expected if command"),
        }
    }

    #[test]
    fn test_for_loop() {
        let cmd = parse("for i in a b c; do echo $i; done").unwrap();
        match cmd {
            Command::For(for_loop) => {
                assert_eq!(for_loop.var, "i");
                assert_eq!(for_loop.words, vec!["a", "b", "c"]);
            }
            _ => panic!("Expected for command"),
        }
    }

    #[test]
    fn test_while_loop() {
        let cmd = parse("while true; do echo loop; done").unwrap();
        match cmd {
            Command::While(while_loop) => {
                // Condition should be 'true', body should be 'echo loop'
                assert!(matches!(*while_loop.body, Command::Simple(ref s) if s.words == vec!["echo", "loop"]));
            }
            _ => panic!("Expected while command"),
        }
    }

    #[test]
    fn test_case_statement() {
        let cmd = parse("case x in a) echo A;; b) echo B;; esac").unwrap();
        match cmd {
            Command::Case(case_stmt) => {
                assert_eq!(case_stmt.word, "x");
                assert_eq!(case_stmt.clauses.len(), 2);
                assert_eq!(case_stmt.clauses[0].0, vec!["a"]);
                assert_eq!(case_stmt.clauses[1].0, vec!["b"]);
            }
            _ => panic!("Expected case command"),
        }
    }

    #[test]
    fn test_subshell() {
        let cmd = parse("(echo hello)").unwrap();
        match cmd {
            Command::Subshell(inner) => {
                assert!(matches!(*inner, Command::Simple(ref s) if s.words == vec!["echo", "hello"]));
            }
            _ => panic!("Expected subshell"),
        }
    }

    #[test]
    fn test_group() {
        let cmd = parse("{ echo a; echo b; }").unwrap();
        match cmd {
            Command::Group(inner) => {
                // Group contains a list of commands
                assert!(matches!(*inner, Command::List(..)));
            }
            _ => panic!("Expected group"),
        }
    }

    #[test]
    fn test_function_def_posix() {
        let cmd = parse("foo() { echo hello; }").unwrap();
        match cmd {
            Command::Function(func) => {
                assert_eq!(func.name, "foo");
                // Body is the command inside { }, which is a Simple command for "echo hello"
                assert!(matches!(*func.body, Command::Simple(..)));
            }
            _ => panic!("Expected function definition, got {:?}", cmd),
        }
    }

    #[test]
    fn test_function_def_bash() {
        let cmd = parse("function foo { echo hello; }").unwrap();
        match cmd {
            Command::Function(func) => {
                assert_eq!(func.name, "foo");
                assert!(matches!(*func.body, Command::Simple(..)));
            }
            _ => panic!("Expected function definition"),
        }
    }

    #[test]
    fn test_function_def_bash_with_parens() {
        let cmd = parse("function foo() { echo hello; }").unwrap();
        match cmd {
            Command::Function(func) => {
                assert_eq!(func.name, "foo");
                assert!(matches!(*func.body, Command::Simple(..)));
            }
            _ => panic!("Expected function definition"),
        }
    }

    #[test]
    fn test_function_def_compound_body() {
        let cmd = parse("foo() { if true; then echo yes; fi; }").unwrap();
        match cmd {
            Command::Function(func) => {
                assert_eq!(func.name, "foo");
                assert!(matches!(*func.body, Command::If(..)));
            }
            _ => panic!("Expected function with if body"),
        }
    }
}
