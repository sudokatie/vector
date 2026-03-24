//! Parser module - parses tokens into AST

pub mod ast;

pub use ast::{Expr, Stmt};

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
    current: Option<Token>,
    previous: Option<Token>,
}

impl<'a> Parser<'a> {
    pub fn new(lexer: Lexer<'a>) -> Self {
        let mut parser = Self {
            lexer,
            current: None,
            previous: None,
        };
        parser.advance().ok();
        parser
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            statements.push(self.statement()?);
        }

        Ok(statements)
    }

    fn statement(&mut self) -> Result<Stmt, ParseError> {
        // For now, parse everything as expression statement
        let expr = self.expression()?;
        Ok(Stmt::Expr(expr))
    }

    fn expression(&mut self) -> Result<Expr, ParseError> {
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.current.clone().ok_or(ParseError::UnexpectedEof)?;

        match &token.kind {
            TokenKind::Int(n) => {
                self.advance()?;
                Ok(Expr::Int(*n))
            }
            TokenKind::Float(n) => {
                self.advance()?;
                Ok(Expr::Float(*n))
            }
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance()?;
                Ok(Expr::String(s))
            }
            TokenKind::True => {
                self.advance()?;
                Ok(Expr::Bool(true))
            }
            TokenKind::False => {
                self.advance()?;
                Ok(Expr::Bool(false))
            }
            TokenKind::Nil => {
                self.advance()?;
                Ok(Expr::Nil)
            }
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance()?;
                Ok(Expr::Identifier(name))
            }
            _ => Err(ParseError::UnexpectedToken {
                found: format!("{}", token.kind),
                expected: "expression".to_string(),
                line: token.line,
            }),
        }
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        self.previous = self.current.take();
        self.current = match self.lexer.next_token() {
            Ok(token) if token.kind == TokenKind::Eof => None,
            Ok(token) => Some(token),
            Err(e) => return Err(ParseError::Lexer(e)),
        };
        Ok(())
    }

    fn is_at_end(&self) -> bool {
        self.current.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_int() {
        let lexer = Lexer::new("42");
        let mut parser = Parser::new(lexer);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Expr(Expr::Int(42)) => {}
            _ => panic!("Expected Int(42)"),
        }
    }

    #[test]
    fn test_parse_bool() {
        let lexer = Lexer::new("true");
        let mut parser = Parser::new(lexer);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Expr(Expr::Bool(true)) => {}
            _ => panic!("Expected Bool(true)"),
        }
    }
}
