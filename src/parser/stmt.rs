//! Statement parsing

use super::{Parser, ParseError};
use super::ast::{Stmt, Expr, FunctionDef, MatchArm};
use crate::lexer::TokenKind;

impl<'a> Parser<'a> {
    /// Parse a statement
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
                TokenKind::Fn => {
                    // Peek ahead: fn followed by identifier is a statement,
                    // fn followed by ( is an expression
                    if self.peek_is_identifier() {
                        return self.function_statement();
                    }
                    // Otherwise fall through to expression parsing
                }
                TokenKind::If => {
                    // Parse if as an expression and wrap in Stmt::Expr
                    let if_expr = self.parse_if_expression()?;
                    return Ok(Stmt::Expr(if_expr));
                }
                TokenKind::While => return self.while_statement(),
                TokenKind::For => return self.for_statement(),
                TokenKind::Return => return self.return_statement(),
                TokenKind::Match => return self.match_statement(),
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

    /// Parse let statement
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

    /// Parse function statement (named function)
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

    /// Parse if statement (as Stmt::If, not expression)
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

    /// Parse while statement
    fn while_statement(&mut self) -> Result<Stmt, ParseError> {
        self.advance()?; // consume 'while'

        let condition = self.parse_expr()?;
        self.expect(TokenKind::LeftBrace)?;
        let body = self.parse_block_stmts()?;
        self.expect(TokenKind::RightBrace)?;

        Ok(Stmt::While(condition, body))
    }

    /// Parse for statement
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

    /// Parse return statement
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

    /// Parse match statement
    fn match_statement(&mut self) -> Result<Stmt, ParseError> {
        self.advance()?; // consume 'match'
        
        let value = self.parse_expr()?;
        self.expect(TokenKind::LeftBrace)?;
        
        let mut arms = Vec::new();
        
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            arms.push(self.parse_match_arm()?);
            // Optional comma between arms
            self.match_token(TokenKind::Comma)?;
        }
        
        self.expect(TokenKind::RightBrace)?;
        
        Ok(Stmt::Expr(Expr::Match(Box::new(value), arms)))
    }

    /// Parse a single match arm
    pub(crate) fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let pattern = self.parse_pattern()?;
        
        // Optional guard: if condition
        let guard = if self.match_token(TokenKind::If)? {
            Some(self.parse_expr()?)
        } else {
            None
        };
        
        self.expect(TokenKind::FatArrow)?;
        
        // Body can be a block or single expression
        let body = if self.check(&TokenKind::LeftBrace) {
            self.advance()?;
            let stmts = self.parse_block_stmts()?;
            self.expect(TokenKind::RightBrace)?;
            Expr::Block(stmts)
        } else {
            self.parse_expr()?
        };
        
        Ok(MatchArm { pattern, guard, body })
    }

    // parse_block_stmts is in expr.rs
}
