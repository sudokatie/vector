//! Expression parser using Pratt parsing (precedence climbing)

use super::ast::{BinaryOp, Expr, FunctionDef, UnaryOp, InterpolationPart};
use super::{ParseError, Parser};
use crate::lexer::{TokenKind, StringPart};

/// Operator precedence levels (higher = tighter binding)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    None = 0,
    Assignment = 1,  // =
    Or = 2,          // or
    And = 3,         // and
    Equality = 4,    // == !=
    Comparison = 5,  // < <= > >=
    BitOr = 6,       // |
    BitXor = 7,      // ^
    BitAnd = 8,      // &
    Shift = 9,       // << >>
    Range = 10,      // .. ..=
    Term = 11,       // + - ++
    Factor = 12,     // * / %
    Power = 13,      // ** (right associative)
    Unary = 14,      // - not ~
    Call = 15,       // () [] .
}

impl<'a> Parser<'a> {
    /// Parse an expression with the given minimum precedence
    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_expr_precedence(Precedence::None)
    }

    /// Pratt parser: parse expression with precedence climbing
    pub fn parse_expr_precedence(&mut self, min_prec: Precedence) -> Result<Expr, ParseError> {
        let mut left = self.parse_prefix()?;

        while let Some(op_prec) = self.current_precedence() {
            if op_prec <= min_prec {
                break;
            }

            left = self.parse_infix(left, op_prec)?;
        }

        Ok(left)
    }

    /// Parse prefix expressions (literals, unary ops, grouping)
    fn parse_prefix(&mut self) -> Result<Expr, ParseError> {
        let token = self.current.clone().ok_or(ParseError::UnexpectedEof)?;

        match &token.kind {
            // Literals
            TokenKind::Int(n) => {
                let n = *n;
                self.advance()?;
                Ok(Expr::Int(n))
            }
            TokenKind::Float(n) => {
                let n = *n;
                self.advance()?;
                Ok(Expr::Float(n))
            }
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance()?;
                Ok(Expr::String(s))
            }
            TokenKind::InterpolatedString(parts) => {
                let parts = parts.clone();
                self.advance()?;
                self.parse_interpolated_string(parts)
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

            // Grouping: (expr)
            TokenKind::LeftParen => {
                self.advance()?; // consume (
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RightParen)?;
                Ok(expr)
            }

            // Array literal: [a, b, c]
            TokenKind::LeftBracket => {
                self.advance()?; // consume [
                let elements = self.parse_list(TokenKind::RightBracket)?;
                Ok(Expr::Array(elements))
            }

            // Table literal: { key: value, ... }
            TokenKind::LeftBrace => {
                self.advance()?; // consume {
                let pairs = self.parse_table_pairs()?;
                self.expect(TokenKind::RightBrace)?;
                Ok(Expr::Table(pairs))
            }

            // Unary minus: -expr
            TokenKind::Minus => {
                self.advance()?;
                let operand = self.parse_expr_precedence(Precedence::Unary)?;
                Ok(Expr::Unary(UnaryOp::Neg, Box::new(operand)))
            }

            // Logical not: not expr
            TokenKind::Not => {
                self.advance()?;
                let operand = self.parse_expr_precedence(Precedence::Unary)?;
                Ok(Expr::Unary(UnaryOp::Not, Box::new(operand)))
            }

            // Bitwise not: ~expr
            TokenKind::Tilde => {
                self.advance()?;
                let operand = self.parse_expr_precedence(Precedence::Unary)?;
                Ok(Expr::Unary(UnaryOp::BitNot, Box::new(operand)))
            }

            // Anonymous function: fn(params) { body }
            TokenKind::Fn => {
                self.parse_anonymous_function()
            }

            // If expression: if cond { then_expr } else { else_expr }
            TokenKind::If => {
                self.parse_if_expression()
            }

            // Try expression: try expr
            TokenKind::Try => {
                self.advance()?;
                let expr = self.parse_expr_precedence(Precedence::Unary)?;
                Ok(Expr::Try(Box::new(expr)))
            }

            _ => Err(ParseError::UnexpectedToken {
                found: format!("{}", token.kind),
                expected: "expression".to_string(),
                line: token.line,
            }),
        }
    }

    /// Parse infix expressions (binary ops, calls, indexing)
    fn parse_infix(&mut self, left: Expr, prec: Precedence) -> Result<Expr, ParseError> {
        let token = self.current.clone().ok_or(ParseError::UnexpectedEof)?;

        match &token.kind {
            // Binary operators
            TokenKind::Plus => self.parse_binary(left, BinaryOp::Add, prec),
            TokenKind::Minus => self.parse_binary(left, BinaryOp::Sub, prec),
            TokenKind::Star => self.parse_binary(left, BinaryOp::Mul, prec),
            TokenKind::Slash => self.parse_binary(left, BinaryOp::Div, prec),
            TokenKind::Percent => self.parse_binary(left, BinaryOp::Mod, prec),
            TokenKind::StarStar => self.parse_binary_right(left, BinaryOp::Pow, prec),
            TokenKind::PlusPlus => self.parse_binary(left, BinaryOp::Concat, prec),

            TokenKind::EqualEqual => self.parse_binary(left, BinaryOp::Eq, prec),
            TokenKind::BangEqual => self.parse_binary(left, BinaryOp::Ne, prec),
            TokenKind::Less => self.parse_binary(left, BinaryOp::Lt, prec),
            TokenKind::LessEqual => self.parse_binary(left, BinaryOp::Le, prec),
            TokenKind::Greater => self.parse_binary(left, BinaryOp::Gt, prec),
            TokenKind::GreaterEqual => self.parse_binary(left, BinaryOp::Ge, prec),

            TokenKind::And => self.parse_binary(left, BinaryOp::And, prec),
            TokenKind::Or => self.parse_binary(left, BinaryOp::Or, prec),

            TokenKind::Ampersand => self.parse_binary(left, BinaryOp::BitAnd, prec),
            TokenKind::Pipe => self.parse_binary(left, BinaryOp::BitOr, prec),
            TokenKind::Caret => self.parse_binary(left, BinaryOp::BitXor, prec),
            TokenKind::LessLess => self.parse_binary(left, BinaryOp::Shl, prec),
            TokenKind::GreaterGreater => self.parse_binary(left, BinaryOp::Shr, prec),

            TokenKind::DotDot => self.parse_binary(left, BinaryOp::Range, prec),
            TokenKind::DotDotEqual => self.parse_binary(left, BinaryOp::RangeInclusive, prec),

            // Call: expr(args)
            TokenKind::LeftParen => {
                self.advance()?; // consume (
                let args = self.parse_list(TokenKind::RightParen)?;
                Ok(Expr::Call(Box::new(left), args))
            }

            // Index: expr[index]
            TokenKind::LeftBracket => {
                self.advance()?; // consume [
                let index = self.parse_expr()?;
                self.expect(TokenKind::RightBracket)?;
                Ok(Expr::Index(Box::new(left), Box::new(index)))
            }

            // Field access: expr.field
            TokenKind::Dot => {
                self.advance()?; // consume .
                let field = self.expect_identifier()?;
                Ok(Expr::Field(Box::new(left), field))
            }

            _ => Ok(left),
        }
    }

    /// Parse left-associative binary operator
    fn parse_binary(&mut self, left: Expr, op: BinaryOp, prec: Precedence) -> Result<Expr, ParseError> {
        self.advance()?; // consume operator
        let right = self.parse_expr_precedence(prec)?;
        Ok(Expr::Binary(Box::new(left), op, Box::new(right)))
    }

    /// Parse right-associative binary operator (e.g., **)
    fn parse_binary_right(&mut self, left: Expr, op: BinaryOp, prec: Precedence) -> Result<Expr, ParseError> {
        self.advance()?; // consume operator
        // Use prec - 1 for right associativity
        let right_prec = match prec {
            Precedence::Power => Precedence::Factor, // One level lower
            p => p,
        };
        let right = self.parse_expr_precedence(right_prec)?;
        Ok(Expr::Binary(Box::new(left), op, Box::new(right)))
    }

    /// Get precedence of current token (if it's an infix operator)
    fn current_precedence(&self) -> Option<Precedence> {
        let token = self.current.as_ref()?;
        Some(match &token.kind {
            TokenKind::Or => Precedence::Or,
            TokenKind::And => Precedence::And,
            TokenKind::EqualEqual | TokenKind::BangEqual => Precedence::Equality,
            TokenKind::Less | TokenKind::LessEqual |
            TokenKind::Greater | TokenKind::GreaterEqual => Precedence::Comparison,
            TokenKind::Pipe => Precedence::BitOr,
            TokenKind::Caret => Precedence::BitXor,
            TokenKind::Ampersand => Precedence::BitAnd,
            TokenKind::LessLess | TokenKind::GreaterGreater => Precedence::Shift,
            TokenKind::DotDot | TokenKind::DotDotEqual => Precedence::Range,
            TokenKind::Plus | TokenKind::Minus | TokenKind::PlusPlus => Precedence::Term,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Precedence::Factor,
            TokenKind::StarStar => Precedence::Power,
            TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::Dot => Precedence::Call,
            _ => return None,
        })
    }

    /// Parse comma-separated list until closing token
    fn parse_list(&mut self, closing: TokenKind) -> Result<Vec<Expr>, ParseError> {
        let mut items = Vec::new();

        if !self.check(&closing) {
            items.push(self.parse_expr()?);

            while self.match_token(TokenKind::Comma)? {
                if self.check(&closing) {
                    break; // trailing comma
                }
                items.push(self.parse_expr()?);
            }
        }

        self.expect(closing)?;
        Ok(items)
    }

    /// Parse table key-value pairs
    fn parse_table_pairs(&mut self) -> Result<Vec<(Expr, Expr)>, ParseError> {
        let mut pairs = Vec::new();

        if !self.check(&TokenKind::RightBrace) {
            pairs.push(self.parse_table_pair()?);

            while self.match_token(TokenKind::Comma)? {
                if self.check(&TokenKind::RightBrace) {
                    break; // trailing comma
                }
                pairs.push(self.parse_table_pair()?);
            }
        }

        Ok(pairs)
    }

    /// Parse single table pair (key: value or just value for implicit index)
    fn parse_table_pair(&mut self) -> Result<(Expr, Expr), ParseError> {
        // Check for identifier: value syntax
        if let Some(token) = &self.current {
            if let TokenKind::Identifier(name) = &token.kind {
                let name = name.clone();
                let saved_current = self.current.clone();

                self.advance()?;

                if self.match_token(TokenKind::Colon)? {
                    // identifier: value
                    let value = self.parse_expr()?;
                    return Ok((Expr::String(name), value));
                } else {
                    // Just an expression, restore and parse normally
                    self.current = saved_current;
                }
            }
        }

        // General case: [key] = value or just value
        let key = self.parse_expr()?;

        if self.match_token(TokenKind::Colon)? {
            let value = self.parse_expr()?;
            Ok((key, value))
        } else {
            // Implicit numeric index (will be assigned at runtime)
            Ok((Expr::Nil, key))
        }
    }

    /// Parse interpolated string into expression
    fn parse_interpolated_string(&mut self, parts: Vec<StringPart>) -> Result<Expr, ParseError> {
        use crate::lexer::Lexer;
        
        let mut result_parts = Vec::new();
        
        for part in parts {
            match part {
                StringPart::Literal(s) => {
                    result_parts.push(InterpolationPart::Literal(s));
                }
                StringPart::Interpolation(expr_src) => {
                    // Parse the expression source
                    let lexer = Lexer::new(&expr_src);
                    let mut parser = Parser::new(lexer);
                    let expr = parser.parse_expr()?;
                    result_parts.push(InterpolationPart::Expr(Box::new(expr)));
                }
            }
        }
        
        Ok(Expr::Interpolation(result_parts))
    }

    /// Parse anonymous function: fn(params) { body }
    fn parse_anonymous_function(&mut self) -> Result<Expr, ParseError> {
        self.advance()?; // consume 'fn'
        self.expect(TokenKind::LeftParen)?;

        let params = self.parse_param_list()?;

        self.expect(TokenKind::LeftBrace)?;
        let body = self.parse_block_stmts()?;
        self.expect(TokenKind::RightBrace)?;

        Ok(Expr::Function(FunctionDef {
            name: None,
            params,
            body,
        }))
    }

    /// Parse if expression: if cond { expr } else { expr }
    pub fn parse_if_expression(&mut self) -> Result<Expr, ParseError> {
        self.advance()?; // consume 'if'
        
        let condition = self.parse_expr()?;
        
        self.expect(TokenKind::LeftBrace)?;
        let then_expr = self.parse_block_expr()?;
        self.expect(TokenKind::RightBrace)?;
        
        let else_expr = if self.match_token(TokenKind::Else)? {
            if self.check(&TokenKind::If) {
                // else if - recursively parse
                Some(Box::new(self.parse_if_expression()?))
            } else {
                self.expect(TokenKind::LeftBrace)?;
                let expr = self.parse_block_expr()?;
                self.expect(TokenKind::RightBrace)?;
                Some(Box::new(expr))
            }
        } else {
            None
        };
        
        Ok(Expr::If(Box::new(condition), Box::new(then_expr), else_expr))
    }
    
    /// Parse a block as an expression (returns the last expression value)
    fn parse_block_expr(&mut self) -> Result<Expr, ParseError> {
        let mut stmts = Vec::new();
        
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            stmts.push(self.statement()?);
        }
        
        if stmts.is_empty() {
            return Ok(Expr::Nil);
        }
        
        // Check if last statement is an expression (for implicit return)
        // Use match to avoid consuming the value on failed pattern match
        match stmts.last() {
            Some(super::ast::Stmt::Expr(_)) => {
                // Last is expression - if it's the only statement, unwrap it
                if stmts.len() == 1 {
                    if let Some(super::ast::Stmt::Expr(expr)) = stmts.pop() {
                        return Ok(expr);
                    }
                }
                // Otherwise keep as block
                Ok(Expr::Block(stmts))
            }
            Some(_) => {
                // Last statement is not an expression (e.g., Return, Let, etc.)
                // Keep all statements in the block
                Ok(Expr::Block(stmts))
            }
            None => Ok(Expr::Nil),
        }
    }

    /// Parse parameter list
    pub fn parse_param_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut params = Vec::new();

        if !self.check(&TokenKind::RightParen) {
            params.push(self.expect_identifier()?);

            while self.match_token(TokenKind::Comma)? {
                if self.check(&TokenKind::RightParen) {
                    break;
                }
                params.push(self.expect_identifier()?);
            }
        }

        self.expect(TokenKind::RightParen)?;
        Ok(params)
    }

    // Helper methods (check, match_token, expect, expect_identifier, parse_block_stmts)
    // are defined in mod.rs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_expr(source: &str) -> Result<Expr, ParseError> {
        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer);
        parser.parse_expr()
    }

    #[test]
    fn test_precedence_add_mul() {
        // 1 + 2 * 3 = 1 + (2 * 3)
        let expr = parse_expr("1 + 2 * 3").unwrap();
        match expr {
            Expr::Binary(left, BinaryOp::Add, right) => {
                assert_eq!(*left, Expr::Int(1));
                match *right {
                    Expr::Binary(l, BinaryOp::Mul, r) => {
                        assert_eq!(*l, Expr::Int(2));
                        assert_eq!(*r, Expr::Int(3));
                    }
                    _ => panic!("Expected Mul"),
                }
            }
            _ => panic!("Expected Add"),
        }
    }

    #[test]
    fn test_precedence_mul_add() {
        // 1 * 2 + 3 = (1 * 2) + 3
        let expr = parse_expr("1 * 2 + 3").unwrap();
        match expr {
            Expr::Binary(left, BinaryOp::Add, right) => {
                match *left {
                    Expr::Binary(l, BinaryOp::Mul, r) => {
                        assert_eq!(*l, Expr::Int(1));
                        assert_eq!(*r, Expr::Int(2));
                    }
                    _ => panic!("Expected Mul"),
                }
                assert_eq!(*right, Expr::Int(3));
            }
            _ => panic!("Expected Add"),
        }
    }

    #[test]
    fn test_associativity_left() {
        // 1 - 2 - 3 = (1 - 2) - 3
        let expr = parse_expr("1 - 2 - 3").unwrap();
        match expr {
            Expr::Binary(left, BinaryOp::Sub, right) => {
                match *left {
                    Expr::Binary(l, BinaryOp::Sub, r) => {
                        assert_eq!(*l, Expr::Int(1));
                        assert_eq!(*r, Expr::Int(2));
                    }
                    _ => panic!("Expected inner Sub"),
                }
                assert_eq!(*right, Expr::Int(3));
            }
            _ => panic!("Expected outer Sub"),
        }
    }

    #[test]
    fn test_associativity_right_power() {
        // 2 ** 3 ** 4 = 2 ** (3 ** 4)
        let expr = parse_expr("2 ** 3 ** 4").unwrap();
        match expr {
            Expr::Binary(left, BinaryOp::Pow, right) => {
                assert_eq!(*left, Expr::Int(2));
                match *right {
                    Expr::Binary(l, BinaryOp::Pow, r) => {
                        assert_eq!(*l, Expr::Int(3));
                        assert_eq!(*r, Expr::Int(4));
                    }
                    _ => panic!("Expected inner Pow"),
                }
            }
            _ => panic!("Expected outer Pow"),
        }
    }

    #[test]
    fn test_unary_minus() {
        let expr = parse_expr("-42").unwrap();
        match expr {
            Expr::Unary(UnaryOp::Neg, operand) => {
                assert_eq!(*operand, Expr::Int(42));
            }
            _ => panic!("Expected Neg"),
        }
    }

    #[test]
    fn test_unary_not() {
        let expr = parse_expr("not true").unwrap();
        match expr {
            Expr::Unary(UnaryOp::Not, operand) => {
                assert_eq!(*operand, Expr::Bool(true));
            }
            _ => panic!("Expected Not"),
        }
    }

    #[test]
    fn test_grouping() {
        // (1 + 2) * 3
        let expr = parse_expr("(1 + 2) * 3").unwrap();
        match expr {
            Expr::Binary(left, BinaryOp::Mul, right) => {
                match *left {
                    Expr::Binary(l, BinaryOp::Add, r) => {
                        assert_eq!(*l, Expr::Int(1));
                        assert_eq!(*r, Expr::Int(2));
                    }
                    _ => panic!("Expected Add"),
                }
                assert_eq!(*right, Expr::Int(3));
            }
            _ => panic!("Expected Mul"),
        }
    }

    #[test]
    fn test_call() {
        let expr = parse_expr("foo(1, 2)").unwrap();
        match expr {
            Expr::Call(callee, args) => {
                assert_eq!(*callee, Expr::Identifier("foo".to_string()));
                assert_eq!(args.len(), 2);
                assert_eq!(args[0], Expr::Int(1));
                assert_eq!(args[1], Expr::Int(2));
            }
            _ => panic!("Expected Call"),
        }
    }

    #[test]
    fn test_index() {
        let expr = parse_expr("arr[0]").unwrap();
        match expr {
            Expr::Index(arr, idx) => {
                assert_eq!(*arr, Expr::Identifier("arr".to_string()));
                assert_eq!(*idx, Expr::Int(0));
            }
            _ => panic!("Expected Index"),
        }
    }

    #[test]
    fn test_field() {
        let expr = parse_expr("obj.field").unwrap();
        match expr {
            Expr::Field(obj, field) => {
                assert_eq!(*obj, Expr::Identifier("obj".to_string()));
                assert_eq!(field, "field");
            }
            _ => panic!("Expected Field"),
        }
    }

    #[test]
    fn test_array_literal() {
        let expr = parse_expr("[1, 2, 3]").unwrap();
        match expr {
            Expr::Array(elements) => {
                assert_eq!(elements.len(), 3);
                assert_eq!(elements[0], Expr::Int(1));
                assert_eq!(elements[1], Expr::Int(2));
                assert_eq!(elements[2], Expr::Int(3));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_table_literal() {
        let expr = parse_expr("{ name: \"Alice\", age: 30 }").unwrap();
        match expr {
            Expr::Table(pairs) => {
                assert_eq!(pairs.len(), 2);
            }
            _ => panic!("Expected Table"),
        }
    }

    #[test]
    fn test_chained_calls() {
        let expr = parse_expr("a.b().c[0]").unwrap();
        // Should parse as ((a.b()).c)[0]
        match expr {
            Expr::Index(inner, _) => {
                match *inner {
                    Expr::Field(call, _) => {
                        match *call {
                            Expr::Call(field, _) => {
                                match *field {
                                    Expr::Field(base, _) => {
                                        assert_eq!(*base, Expr::Identifier("a".to_string()));
                                    }
                                    _ => panic!("Expected inner Field"),
                                }
                            }
                            _ => panic!("Expected Call"),
                        }
                    }
                    _ => panic!("Expected outer Field"),
                }
            }
            _ => panic!("Expected Index"),
        }
    }

    #[test]
    fn test_comparison_chain() {
        // 1 < 2 and 2 < 3
        let expr = parse_expr("1 < 2 and 2 < 3").unwrap();
        match expr {
            Expr::Binary(_, BinaryOp::And, _) => {}
            _ => panic!("Expected And"),
        }
    }

    #[test]
    fn test_logical_precedence() {
        // a or b and c = a or (b and c)
        let expr = parse_expr("a or b and c").unwrap();
        match expr {
            Expr::Binary(left, BinaryOp::Or, right) => {
                assert_eq!(*left, Expr::Identifier("a".to_string()));
                match *right {
                    Expr::Binary(l, BinaryOp::And, r) => {
                        assert_eq!(*l, Expr::Identifier("b".to_string()));
                        assert_eq!(*r, Expr::Identifier("c".to_string()));
                    }
                    _ => panic!("Expected And"),
                }
            }
            _ => panic!("Expected Or"),
        }
    }
}
