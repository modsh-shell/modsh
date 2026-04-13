//! Lexer module for POSIX shell syntax tokenization
//!
//! This module provides functionality to tokenize shell input into a stream of tokens.

mod token;
mod core;

// Re-export public types from submodules
pub use token::{LexError, Operator, Redirect, Token};
pub use core::{Lexer, tokenize};

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
        let has_comment = tokens.iter().any(|t| matches!(t, Token::Comment(_)));
        assert!(has_comment, "Expected a comment token");
    }

    #[test]
    fn test_escaped_characters() {
        let tokens = tokenize(r#"echo hello\ world"#).unwrap();
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
        assert_eq!(tokens.len(), 9);
        assert!(matches!(tokens[2], Token::Operator(Operator::Pipe)));
        assert!(matches!(tokens[5], Token::Operator(Operator::Pipe)));
    }

    #[test]
    fn test_hash_in_word_not_comment() {
        let tokens = tokenize("echo foo#bar").unwrap();
        assert!(matches!(tokens[1], Token::Word(ref w) if w == "foo#bar"));
    }

    #[test]
    fn test_dollar_variables() {
        let tokens = tokenize("echo $HOME ${USER}").unwrap();
        assert!(matches!(tokens[1], Token::Word(ref w) if w.contains("HOME")));
    }

    #[test]
    fn test_quoted_newline() {
        let tokens = tokenize("'line1\nline2'").unwrap();
        assert!(matches!(tokens[0], Token::Word(ref w) if w.contains('\n')));
    }

    #[test]
    fn test_brace_expansion_not_special() {
        let tokens = tokenize("{a,b,c}").unwrap();
        assert!(matches!(tokens[0], Token::Operator(Operator::LBrace)));
    }
}
