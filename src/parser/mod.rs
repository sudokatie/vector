//! Parser module - parses tokens into AST

pub mod ast;
pub mod expr;

pub use ast::{Expr, Stmt, BinaryOp, UnaryOp, FunctionDef};

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

    pub fn statement(&mut self) -> Result<Stmt, ParseError> {
        // Skip any trailing semicolons
        while self.match_token(TokenKind::Semicolon)? {}

        if self.is_at_end() {
            return Err(ParseError::UnexpectedEof);
        }

        // Check for statement-starting keywords
        if let Some(token) = &self.current {
            match &token.kind {
                TokenKind::Let => return self.let_statement(),
                TokenKind::Fn => return self.function_statement(),
                TokenKind::If => return self.if_statement(),
                TokenKind::While => return self.while_statement(),
                TokenKind::For => return self.for_statement(),
                TokenKind::Return => return self.return_statement(),
                _ => {}
            }
        }

        // Expression statement or assignment
        let expr = self.parse_expr()?;

        // Check for assignment
        if self.match_token(TokenKind::Equal)? {
            let value = self.parse_expr()?;
            return Ok(Stmt::Assign(expr, value));
        }

        Ok(Stmt::Expr(expr))
    }

    fn let_statement(&mut self) -> Result<Stmt, ParseError> {
        self.advance()?; // consume 'let'

        let is_mut = self.match_token(TokenKind::Mut)?;
        let name = self.expect_identifier()?;

        let initializer = if self.match_token(TokenKind::Equal)? {
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(Stmt::Let(name, is_mut, initializer))
    }

    fn function_statement(&mut self) -> Result<Stmt, ParseError> {
        self.advance()?; // consume 'fn'

        let name = self.expect_identifier()?;
        self.expect(TokenKind::LeftParen)?;
        let params = self.parse_param_list()?;

        self.expect(TokenKind::LeftBrace)?;
        let body = self.parse_block_stmts()?;
        self.expect(TokenKind::RightBrace)?;

        Ok(Stmt::Function(FunctionDef {
            name: Some(name),
            params,
            body,
        }))
    }

    fn if_statement(&mut self) -> Result<Stmt, ParseError> {
        self.advance()?; // consume 'if'

        let condition = self.parse_expr()?;
        self.expect(TokenKind::LeftBrace)?;
        let then_branch = self.parse_block_stmts()?;
        self.expect(TokenKind::RightBrace)?;

        let else_branch = if self.match_token(TokenKind::Else)? {
            if self.check(&TokenKind::If) {
                // else if
                Some(vec![self.if_statement()?])
            } else {
                self.expect(TokenKind::LeftBrace)?;
                let stmts = self.parse_block_stmts()?;
                self.expect(TokenKind::RightBrace)?;
                Some(stmts)
            }
        } else {
            None
        };

        Ok(Stmt::If(condition, then_branch, else_branch))
    }

    fn while_statement(&mut self) -> Result<Stmt, ParseError> {
        self.advance()?; // consume 'while'

        let condition = self.parse_expr()?;
        self.expect(TokenKind::LeftBrace)?;
        let body = self.parse_block_stmts()?;
        self.expect(TokenKind::RightBrace)?;

        Ok(Stmt::While(condition, body))
    }

    fn for_statement(&mut self) -> Result<Stmt, ParseError> {
        self.advance()?; // consume 'for'

        let name = self.expect_identifier()?;
        self.expect(TokenKind::In)?;
        let iterable = self.parse_expr()?;

        self.expect(TokenKind::LeftBrace)?;
        let body = self.parse_block_stmts()?;
        self.expect(TokenKind::RightBrace)?;

        Ok(Stmt::For(name, iterable, body))
    }

    fn return_statement(&mut self) -> Result<Stmt, ParseError> {
        self.advance()?; // consume 'return'

        // Check if there's an expression or just bare return
        let value = if !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            if !self.check(&TokenKind::Semicolon) {
                Some(self.parse_expr()?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(Stmt::Return(value))
    }

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
    fn test_parse_if() {
        let stmts = parse("if true { 1 }").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::If(cond, then_branch, else_branch) => {
                assert_eq!(*cond, Expr::Bool(true));
                assert_eq!(then_branch.len(), 1);
                assert!(else_branch.is_none());
            }
            _ => panic!("Expected If"),
        }
    }

    #[test]
    fn test_parse_if_else() {
        let stmts = parse("if false { 1 } else { 2 }").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::If(_, _, else_branch) => {
                assert!(else_branch.is_some());
            }
            _ => panic!("Expected If"),
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
