//! Lexer module - tokenizes source code

mod token;

pub use token::{Token, TokenKind, Span, StringPart};

use thiserror::Error;

/// Lexer error type
#[derive(Error, Debug, Clone, PartialEq)]
pub enum LexError {
    #[error("Unexpected character '{0}' at line {1}")]
    UnexpectedChar(char, u32),

    #[error("Unterminated string at line {0}")]
    UnterminatedString(u32),

    #[error("Invalid number format at line {0}")]
    InvalidNumber(u32),

    #[error("Unterminated comment at line {0}")]
    UnterminatedComment(u32),
}

/// Lexer for Vector source code
#[derive(Clone)]
pub struct Lexer<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    line: u32,
    start: usize,
    current: usize,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given source
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
            line: 1,
            start: 0,
            current: 0,
        }
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace();

        if self.is_at_end() {
            return Ok(Token::new(TokenKind::Eof, Span::new(self.current, self.current), self.line));
        }

        self.start = self.current;
        let c = self.advance().unwrap();

        match c {
            // Single character tokens
            '(' => Ok(self.make_token(TokenKind::LeftParen)),
            ')' => Ok(self.make_token(TokenKind::RightParen)),
            '{' => Ok(self.make_token(TokenKind::LeftBrace)),
            '}' => Ok(self.make_token(TokenKind::RightBrace)),
            '[' => Ok(self.make_token(TokenKind::LeftBracket)),
            ']' => Ok(self.make_token(TokenKind::RightBracket)),
            ',' => Ok(self.make_token(TokenKind::Comma)),
            ';' => Ok(self.make_token(TokenKind::Semicolon)),
            ':' => Ok(self.make_token(TokenKind::Colon)),

            // Operators that might be doubled or combined
            '+' => {
                if self.match_char('+') {
                    Ok(self.make_token(TokenKind::PlusPlus))
                } else {
                    Ok(self.make_token(TokenKind::Plus))
                }
            }
            '-' => Ok(self.make_token(TokenKind::Minus)),
            '*' => {
                if self.match_char('*') {
                    Ok(self.make_token(TokenKind::StarStar))
                } else {
                    Ok(self.make_token(TokenKind::Star))
                }
            }
            '/' => {
                if self.match_char('/') {
                    self.skip_line_comment();
                    self.next_token()
                } else if self.match_char('*') {
                    self.skip_block_comment()?;
                    self.next_token()
                } else {
                    Ok(self.make_token(TokenKind::Slash))
                }
            }
            '%' => Ok(self.make_token(TokenKind::Percent)),

            '=' => {
                if self.match_char('=') {
                    Ok(self.make_token(TokenKind::EqualEqual))
                } else if self.match_char('>') {
                    Ok(self.make_token(TokenKind::FatArrow))
                } else {
                    Ok(self.make_token(TokenKind::Equal))
                }
            }
            '!' => {
                if self.match_char('=') {
                    Ok(self.make_token(TokenKind::BangEqual))
                } else {
                    Ok(self.make_token(TokenKind::Bang))
                }
            }
            '<' => {
                if self.match_char('=') {
                    Ok(self.make_token(TokenKind::LessEqual))
                } else if self.match_char('<') {
                    Ok(self.make_token(TokenKind::LessLess))
                } else {
                    Ok(self.make_token(TokenKind::Less))
                }
            }
            '>' => {
                if self.match_char('=') {
                    Ok(self.make_token(TokenKind::GreaterEqual))
                } else if self.match_char('>') {
                    Ok(self.make_token(TokenKind::GreaterGreater))
                } else {
                    Ok(self.make_token(TokenKind::Greater))
                }
            }
            '&' => Ok(self.make_token(TokenKind::Ampersand)),
            '|' => Ok(self.make_token(TokenKind::Pipe)),
            '^' => Ok(self.make_token(TokenKind::Caret)),
            '~' => Ok(self.make_token(TokenKind::Tilde)),

            '.' => {
                if self.match_char('.') {
                    if self.match_char('=') {
                        Ok(self.make_token(TokenKind::DotDotEqual))
                    } else {
                        Ok(self.make_token(TokenKind::DotDot))
                    }
                } else {
                    Ok(self.make_token(TokenKind::Dot))
                }
            }

            '"' => self.string(),

            c if c.is_ascii_digit() => self.number(),
            c if c.is_alphabetic() || c == '_' => self.identifier(),

            _ => Err(LexError::UnexpectedChar(c, self.line)),
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(&(_, c)) = self.chars.peek() {
            match c {
                ' ' | '\r' | '\t' => {
                    self.advance();
                }
                '\n' => {
                    self.line += 1;
                    self.advance();
                }
                _ => break,
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(&(_, c)) = self.chars.peek() {
            if c == '\n' {
                break;
            }
            self.advance();
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), LexError> {
        let start_line = self.line;
        let mut depth = 1;

        while depth > 0 {
            if self.is_at_end() {
                return Err(LexError::UnterminatedComment(start_line));
            }

            let c = self.advance().unwrap();
            if c == '\n' {
                self.line += 1;
            } else if c == '/' && self.match_char('*') {
                depth += 1;
            } else if c == '*' && self.match_char('/') {
                depth -= 1;
            }
        }

        Ok(())
    }

    fn string(&mut self) -> Result<Token, LexError> {
        let start_line = self.line;
        let mut parts: Vec<StringPart> = Vec::new();
        let mut current_literal = String::new();
        let mut has_interpolation = false;

        while let Some(&(_, c)) = self.chars.peek() {
            if c == '"' {
                break;
            }
            if c == '\n' {
                self.line += 1;
                current_literal.push(c);
                self.advance();
            } else if c == '\\' {
                self.advance();
                // Handle escape sequences
                if let Some(&(_, escaped)) = self.chars.peek() {
                    self.advance();
                    match escaped {
                        'n' => current_literal.push('\n'),
                        't' => current_literal.push('\t'),
                        'r' => current_literal.push('\r'),
                        '\\' => current_literal.push('\\'),
                        '"' => current_literal.push('"'),
                        '{' => current_literal.push('{'),
                        '}' => current_literal.push('}'),
                        _ => {
                            current_literal.push('\\');
                            current_literal.push(escaped);
                        }
                    }
                }
            } else if c == '{' {
                self.advance(); // consume '{'
                has_interpolation = true;
                
                // Save current literal if non-empty
                if !current_literal.is_empty() {
                    parts.push(StringPart::Literal(std::mem::take(&mut current_literal)));
                }
                
                // Parse interpolation expression (until matching '}')
                let mut expr = String::new();
                let mut brace_depth = 1;
                
                while brace_depth > 0 {
                    if self.is_at_end() {
                        return Err(LexError::UnterminatedString(start_line));
                    }
                    if let Some(&(_, c)) = self.chars.peek() {
                        self.advance();
                        if c == '{' {
                            brace_depth += 1;
                            expr.push(c);
                        } else if c == '}' {
                            brace_depth -= 1;
                            if brace_depth > 0 {
                                expr.push(c);
                            }
                        } else {
                            if c == '\n' {
                                self.line += 1;
                            }
                            expr.push(c);
                        }
                    }
                }
                
                parts.push(StringPart::Interpolation(expr.trim().to_string()));
            } else {
                current_literal.push(c);
                self.advance();
            }
        }

        if self.is_at_end() {
            return Err(LexError::UnterminatedString(start_line));
        }

        self.advance(); // Closing quote

        // If no interpolation, return simple string
        if !has_interpolation {
            return Ok(Token::new(
                TokenKind::String(current_literal),
                Span::new(self.start, self.current),
                self.line,
            ));
        }

        // Save final literal if non-empty
        if !current_literal.is_empty() {
            parts.push(StringPart::Literal(current_literal));
        }

        Ok(Token::new(
            TokenKind::InterpolatedString(parts),
            Span::new(self.start, self.current),
            self.line,
        ))
    }

    fn number(&mut self) -> Result<Token, LexError> {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }

        let mut is_float = false;
        if self.peek() == Some('.') && self.peek_next().is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            self.advance(); // Consume '.'
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }

        // Scientific notation
        if self.peek().is_some_and(|c| c == 'e' || c == 'E') {
            is_float = true;
            self.advance();
            if self.peek().is_some_and(|c| c == '+' || c == '-') {
                self.advance();
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }

        let text = &self.source[self.start..self.current];
        if is_float {
            match text.parse::<f64>() {
                Ok(value) => Ok(Token::new(
                    TokenKind::Float(value),
                    Span::new(self.start, self.current),
                    self.line,
                )),
                Err(_) => Err(LexError::InvalidNumber(self.line)),
            }
        } else {
            match text.parse::<i64>() {
                Ok(value) => Ok(Token::new(
                    TokenKind::Int(value),
                    Span::new(self.start, self.current),
                    self.line,
                )),
                Err(_) => Err(LexError::InvalidNumber(self.line)),
            }
        }
    }

    fn identifier(&mut self) -> Result<Token, LexError> {
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            self.advance();
        }

        let text = &self.source[self.start..self.current];
        let kind = match text {
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "match" => TokenKind::Match,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "nil" => TokenKind::Nil,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            "try" => TokenKind::Try,
            _ => TokenKind::Identifier(text.to_string()),
        };

        Ok(Token::new(kind, Span::new(self.start, self.current), self.line))
    }

    fn advance(&mut self) -> Option<char> {
        let (i, c) = self.chars.next()?;
        self.current = i + c.len_utf8();
        Some(c)
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, c)| c)
    }

    fn peek_next(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.next().map(|(_, c)| c)
    }

    fn is_at_end(&mut self) -> bool {
        self.chars.peek().is_none()
    }

    fn make_token(&self, kind: TokenKind) -> Token {
        Token::new(kind, Span::new(self.start, self.current), self.line)
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.next_token();
        match &token {
            Ok(t) if t.kind == TokenKind::Eof => None,
            _ => Some(token),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_char_tokens() {
        let mut lexer = Lexer::new("(){}[]");
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::LeftParen);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::RightParen);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::LeftBrace);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::RightBrace);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::LeftBracket);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::RightBracket);
    }

    #[test]
    fn test_operators() {
        let mut lexer = Lexer::new("+ - * / == != < <= > >=");
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Plus);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Minus);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Star);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Slash);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::EqualEqual);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::BangEqual);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Less);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::LessEqual);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Greater);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::GreaterEqual);
    }

    #[test]
    fn test_keywords() {
        let mut lexer = Lexer::new("let mut fn if else while for return true false nil");
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Let);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Mut);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Fn);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::If);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Else);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::While);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::For);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Return);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::True);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::False);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Nil);
    }

    #[test]
    fn test_numbers() {
        let mut lexer = Lexer::new("42 3.14 1e10 2.5e-3");
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Int(42));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Float(3.14));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Float(1e10));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Float(2.5e-3));
    }

    #[test]
    fn test_strings() {
        let mut lexer = Lexer::new(r#""hello" "world""#);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::String("hello".to_string()));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::String("world".to_string()));
    }

    #[test]
    fn test_identifiers() {
        let mut lexer = Lexer::new("foo bar_baz _private");
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Identifier("foo".to_string()));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Identifier("bar_baz".to_string()));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Identifier("_private".to_string()));
    }

    #[test]
    fn test_comments() {
        let mut lexer = Lexer::new("foo // comment\nbar /* block */ baz");
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Identifier("foo".to_string()));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Identifier("bar".to_string()));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Identifier("baz".to_string()));
    }
}
