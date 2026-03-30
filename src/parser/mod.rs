//! Parser module - parses tokens into AST

pub mod ast;
pub mod expr;
pub mod stmt;

pub use ast::{Expr, Stmt, BinaryOp, UnaryOp, FunctionDef, Pattern, MatchArm, InterpolationPart};

use crate::lexer::{Lexer, LexError, Token, TokenKind};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Lexer error: {0}")]
    Lexer(#[from] LexError),

    #[error("Unexpected token '{found}' at line {line}, expected {expected}")]
    UnexpectedToken {
        found: String,
        expected: String,
        line: u32,
    },

    #[error("Unexpected end of input")]
    UnexpectedEof,
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    pub(crate) current: Option<Token>,
    previous: Option<Token>,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next_token().ok();
        Self {
            lexer,
            current,
            previous: None,
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            statements.push(self.statement()?);
        }

        Ok(statements)
    }

    // Statement parsing is in stmt.rs

    pub(crate) fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        // Wildcard pattern: _
        if let Some(token) = &self.current {
            if let TokenKind::Identifier(name) = &token.kind {
                if name == "_" {
                    self.advance()?;
                    return Ok(Pattern::Wildcard);
                }
            }
        }
        
        // Try to parse a literal or identifier
        let expr = self.parse_expr()?;
        
        // Check if expression is a range - convert to Pattern::Range
        match &expr {
            Expr::Binary(left, BinaryOp::Range, right) => {
                return Ok(Pattern::Range(left.clone(), right.clone(), false));
            }
            Expr::Binary(left, BinaryOp::RangeInclusive, right) => {
                return Ok(Pattern::Range(left.clone(), right.clone(), true));
            }
            _ => {}
        }
        
        // Check for range pattern: expr..expr or expr..=expr (fallback)
        if self.check(&TokenKind::DotDot) {
            self.advance()?;
            let end = self.parse_expr()?;
            return Ok(Pattern::Range(Box::new(expr), Box::new(end), false));
        }
        
        if self.check(&TokenKind::DotDotEqual) {
            self.advance()?;
            let end = self.parse_expr()?;
            return Ok(Pattern::Range(Box::new(expr), Box::new(end), true));
        }
        
        // Check if it's a binding (identifier) or literal
        match expr {
            Expr::Identifier(name) => Ok(Pattern::Binding(name)),
            _ => Ok(Pattern::Literal(expr)),
        }
    }

    // === Helper methods ===

    pub(crate) fn advance(&mut self) -> Result<(), ParseError> {
        self.previous = self.current.take();
        self.current = match self.lexer.next_token() {
            Ok(token) if token.kind == TokenKind::Eof => None,
            Ok(token) => Some(token),
            Err(e) => return Err(ParseError::Lexer(e)),
        };
        Ok(())
    }

    pub(crate) fn is_at_end(&self) -> bool {
        self.current.is_none()
    }

    pub(crate) fn check(&self, kind: &TokenKind) -> bool {
        self.current.as_ref().map_or(false, |t| &t.kind == kind)
    }

    pub(crate) fn match_token(&mut self, kind: TokenKind) -> Result<bool, ParseError> {
        if self.check(&kind) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(crate) fn expect(&mut self, kind: TokenKind) -> Result<(), ParseError> {
        if self.check(&kind) {
            self.advance()?;
            Ok(())
        } else {
            let found = self.current.as_ref()
                .map(|t| format!("{}", t.kind))
                .unwrap_or_else(|| "EOF".to_string());
            let line = self.current.as_ref().map(|t| t.line).unwrap_or(0);
            Err(ParseError::UnexpectedToken {
                found,
                expected: format!("{}", kind),
                line,
            })
        }
    }

    pub(crate) fn expect_identifier(&mut self) -> Result<String, ParseError> {
        if let Some(token) = &self.current {
            if let TokenKind::Identifier(name) = &token.kind {
                let name = name.clone();
                self.advance()?;
                return Ok(name);
            }
        }
        let found = self.current.as_ref()
            .map(|t| format!("{}", t.kind))
            .unwrap_or_else(|| "EOF".to_string());
        let line = self.current.as_ref().map(|t| t.line).unwrap_or(0);
        Err(ParseError::UnexpectedToken {
            found,
            expected: "identifier".to_string(),
            line,
        })
    }

    /// Peek at the next token after current to check if it's an identifier
    /// Used to distinguish `fn name(...)` from `fn(...)`
    pub(crate) fn peek_is_identifier(&self) -> bool {
        // Clone lexer state to peek
        let mut peek_lexer = self.lexer.clone();
        if let Ok(next_token) = peek_lexer.next_token() {
            matches!(next_token.kind, TokenKind::Identifier(_))
        } else {
            false
        }
    }

    /// Parse block statements (without braces)
    pub fn parse_block_stmts(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            stmts.push(self.statement()?);
        }

        Ok(stmts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Result<Vec<Stmt>, ParseError> {
        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer);
        parser.parse()
    }

    #[test]
    fn test_parse_int() {
        let stmts = parse("42").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Expr(Expr::Int(42)) => {}
            _ => panic!("Expected Int(42)"),
        }
    }

    #[test]
    fn test_parse_let() {
        let stmts = parse("let x = 42").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Let(name, is_mut, Some(Expr::Int(42))) => {
                assert_eq!(name, "x");
                assert!(!is_mut);
            }
            _ => panic!("Expected Let"),
        }
    }

    #[test]
    fn test_parse_let_mut() {
        let stmts = parse("let mut y = 0").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Let(name, is_mut, _) => {
                assert_eq!(name, "y");
                assert!(is_mut);
            }
            _ => panic!("Expected Let mut"),
        }
    }

    #[test]
    fn test_parse_function() {
        let stmts = parse("fn add(a, b) { return a + b }").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Function(def) => {
                assert_eq!(def.name, Some("add".to_string()));
                assert_eq!(def.params, vec!["a".to_string(), "b".to_string()]);
                assert_eq!(def.body.len(), 1);
            }
            _ => panic!("Expected Function"),
        }
    }

    #[test]
    fn test_parse_if_expr() {
        // If is parsed as an expression, wrapped in Stmt::Expr
        let stmts = parse("if true { 1 }").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Expr(Expr::If(cond, _, _)) => {
                assert_eq!(**cond, Expr::Bool(true));
            }
            _ => panic!("Expected Stmt::Expr(Expr::If(...))"),
        }
    }

    #[test]
    fn test_parse_if_else_expr() {
        let stmts = parse("if false { 1 } else { 2 }").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Expr(Expr::If(_, _, else_branch)) => {
                assert!(else_branch.is_some());
            }
            _ => panic!("Expected Stmt::Expr(Expr::If(...))"),
        }
    }

    #[test]
    fn test_parse_while() {
        let stmts = parse("while x > 0 { x = x - 1 }").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::While(_, body) => {
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected While"),
        }
    }

    #[test]
    fn test_parse_for() {
        let stmts = parse("for i in 0..10 { print(i) }").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::For(name, _, body) => {
                assert_eq!(name, "i");
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected For"),
        }
    }

    #[test]
    fn test_parse_assignment() {
        let stmts = parse("x = 42").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Assign(target, value) => {
                assert_eq!(*target, Expr::Identifier("x".to_string()));
                assert_eq!(*value, Expr::Int(42));
            }
            _ => panic!("Expected Assign"),
        }
    }

    #[test]
    fn test_parse_multiple_statements() {
        let stmts = parse("let x = 1\nlet y = 2\nx + y").unwrap();
        assert_eq!(stmts.len(), 3);
    }
}
