// Copyright (C) 2019-2022 Aleo Systems Inc.
// This file is part of the Leo library.

// The Leo library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The Leo library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the Leo library. If not, see <https://www.gnu.org/licenses/>.

use super::*;

use leo_errors::{ParserError, Result};
use leo_span::sym;

const INT_TYPES: &[Token] = &[
    Token::I8,
    Token::I16,
    Token::I32,
    Token::I64,
    Token::I128,
    Token::U8,
    Token::U16,
    Token::U32,
    Token::U64,
    Token::U128,
    Token::Field,
    Token::Group,
];

impl ParserContext<'_> {
    ///
    /// Returns an [`Expression`] AST node if the next token is an expression.
    /// Includes circuit init expressions.
    ///
    pub fn parse_expression(&mut self) -> Result<Expression> {
        // Store current parser state.
        let prior_fuzzy_state = self.disallow_circuit_construction;

        // Allow circuit init expressions.
        self.disallow_circuit_construction = false;

        // Parse expression.
        let result = self.parse_conditional_expression();

        // Restore prior parser state.
        self.disallow_circuit_construction = prior_fuzzy_state;

        result
    }

    ///
    /// Returns an [`Expression`] AST node if the next tokens represent
    /// a ternary expression. May or may not include circuit init expressions.
    ///
    /// Otherwise, tries to parse the next token using [`parse_disjunctive_expression`].
    ///
    pub fn parse_conditional_expression(&mut self) -> Result<Expression> {
        // Try to parse the next expression. Try BinaryOperation::Or.
        let mut expr = self.parse_disjunctive_expression()?;

        // Parse the rest of the ternary expression.
        if self.eat(Token::Question).is_some() {
            let if_true = self.parse_expression()?;
            self.expect(Token::Colon)?;
            let if_false = self.parse_conditional_expression()?;
            expr = Expression::Ternary(TernaryExpression {
                span: expr.span() + if_false.span(),
                condition: Box::new(expr),
                if_true: Box::new(if_true),
                if_false: Box::new(if_false),
            });
        }
        Ok(expr)
    }

    /// Constructs a binary expression `left op right`.
    fn bin_expr(left: Expression, right: Expression, op: BinaryOperation) -> Expression {
        Expression::Binary(BinaryExpression {
            span: left.span() + right.span(),
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    /// Parses a left-associative binary expression `<left> token <right>` using `f` for left/right.
    /// The `token` is translated to `op` in the AST.
    fn parse_bin_expr(
        &mut self,
        token: Token,
        op: BinaryOperation,
        mut f: impl FnMut(&mut Self) -> Result<Expression>,
    ) -> Result<Expression> {
        let mut expr = f(self)?;
        while self.eat(token.clone()).is_some() {
            expr = Self::bin_expr(expr, f(self)?, op);
        }
        Ok(expr)
    }

    /// Returns an [`Expression`] AST node if the next tokens represent
    /// a binary or expression.
    ///
    /// Otherwise, tries to parse the next token using [`parse_conjunctive_expression`].
    pub fn parse_disjunctive_expression(&mut self) -> Result<Expression> {
        self.parse_bin_expr(Token::Or, BinaryOperation::Or, Self::parse_conjunctive_expression)
    }

    /// Returns an [`Expression`] AST node if the next tokens represent a
    /// binary and expression.
    ///
    /// Otherwise, tries to parse the next token using [`parse_equality_expression`].
    pub fn parse_conjunctive_expression(&mut self) -> Result<Expression> {
        self.parse_bin_expr(Token::And, BinaryOperation::And, Self::parse_equality_expression)
    }

    /// Returns an [`Expression`] AST node if the next tokens represent a
    /// binary equals or not equals expression.
    ///
    /// Otherwise, tries to parse the next token using [`parse_ordering_expression`].
    pub fn parse_equality_expression(&mut self) -> Result<Expression> {
        let mut expr = self.parse_ordering_expression()?;
        if let Some(SpannedToken { token: op, .. }) = self.eat_any(&[Token::Eq, Token::NotEq]) {
            let right = self.parse_ordering_expression()?;
            let op = match op {
                Token::Eq => BinaryOperation::Eq,
                Token::NotEq => BinaryOperation::Ne,
                _ => unreachable!("parse_equality_expression_ shouldn't produce this"),
            };
            expr = Self::bin_expr(expr, right, op);
        }
        Ok(expr)
    }

    /// Returns an [`Expression`] AST node if the next tokens represent a
    /// binary relational expression: less than, less than or equals, greater than, greater than or equals.
    ///
    /// Otherwise, tries to parse the next token using [`parse_shift_expression`].
    pub fn parse_ordering_expression(&mut self) -> Result<Expression> {
        let mut expr = self.parse_additive_expression()?;
        while let Some(SpannedToken { token: op, .. }) = self.eat_any(&[Token::Lt, Token::LtEq, Token::Gt, Token::GtEq])
        {
            let right = self.parse_additive_expression()?;
            let op = match op {
                Token::Lt => BinaryOperation::Lt,
                Token::LtEq => BinaryOperation::Le,
                Token::Gt => BinaryOperation::Gt,
                Token::GtEq => BinaryOperation::Ge,
                _ => unreachable!("parse_ordering_expression_ shouldn't produce this"),
            };
            expr = Self::bin_expr(expr, right, op);
        }
        Ok(expr)
    }

    /// Returns an [`Expression`] AST node if the next tokens represent a
    /// binary addition or subtraction expression.
    ///
    /// Otherwise, tries to parse the next token using [`parse_mul_div_pow_expression`].
    pub fn parse_additive_expression(&mut self) -> Result<Expression> {
        let mut expr = self.parse_multiplicative_expression()?;
        while let Some(SpannedToken { token: op, .. }) = self.eat_any(&[Token::Add, Token::Minus]) {
            let right = self.parse_multiplicative_expression()?;
            let op = match op {
                Token::Add => BinaryOperation::Add,
                Token::Minus => BinaryOperation::Sub,
                _ => unreachable!("parse_additive_expression_ shouldn't produce this"),
            };
            expr = Self::bin_expr(expr, right, op);
        }
        Ok(expr)
    }

    /// Returns an [`Expression`] AST node if the next tokens represent a
    /// binary multiplication, division, or modulus expression.
    ///
    /// Otherwise, tries to parse the next token using [`parse_exponential_expression`].
    pub fn parse_multiplicative_expression(&mut self) -> Result<Expression> {
        let mut expr = self.parse_exponential_expression()?;
        while let Some(SpannedToken { token: op, .. }) = self.eat_any(&[Token::Mul, Token::Div]) {
            let right = self.parse_exponential_expression()?;
            let op = match op {
                Token::Mul => BinaryOperation::Mul,
                Token::Div => BinaryOperation::Div,
                _ => unreachable!("parse_multiplicative_expression_ shouldn't produce this"),
            };
            expr = Self::bin_expr(expr, right, op);
        }
        Ok(expr)
    }

    /// Returns an [`Expression`] AST node if the next tokens represent a
    /// binary exponentiation expression.
    ///
    /// Otherwise, tries to parse the next token using [`parse_cast_expression`].
    pub fn parse_exponential_expression(&mut self) -> Result<Expression> {
        let mut expr = self.parse_cast_expression()?;

        if self.eat(Token::Exp).is_some() {
            let right = self.parse_exponential_expression()?;
            expr = Self::bin_expr(expr, right, BinaryOperation::Pow);
        }

        Ok(expr)
    }

    ///
    /// Returns an [`Expression`] AST node if the next tokens represent a
    /// type cast expression.
    ///
    /// Otherwise, tries to parse the next token using [`parse_unary_expression`].
    ///
    pub fn parse_cast_expression(&mut self) -> Result<Expression> {
        let mut expr = self.parse_unary_expression()?;
        while self.eat(Token::As).is_some() {
            let (type_, type_span) = self.parse_type()?;
            expr = Expression::Cast(CastExpression {
                span: expr.span() + &type_span,
                inner: Box::new(expr),
                target_type: type_,
            })
        }
        Ok(expr)
    }

    ///
    /// Returns an [`Expression`] AST node if the next tokens represent a
    /// unary not, negate, or bitwise not expression.
    ///
    /// Otherwise, tries to parse the next token using [`parse_postfix_expression`].
    ///
    pub fn parse_unary_expression(&mut self) -> Result<Expression> {
        let mut ops = Vec::new();
        while let Some(token) = self.eat_any(&[Token::Not, Token::Minus]) {
            ops.push(token);
        }
        let mut inner = self.parse_postfix_expression()?;
        for op in ops.into_iter().rev() {
            let operation = match op.token {
                Token::Not => UnaryOperation::Not,
                Token::Minus => UnaryOperation::Negate,
                _ => unreachable!("parse_unary_expression_ shouldn't produce this"),
            };
            inner = Expression::Unary(UnaryExpression {
                span: &op.span + inner.span(),
                op: operation,
                inner: Box::new(inner),
            });
        }
        Ok(inner)
    }

    ///
    /// Returns an [`Expression`] AST node if the next tokens represent an
    /// array access, circuit member access, function call, or static function call expression.
    ///
    /// Otherwise, tries to parse the next token using [`parse_primary_expression`].
    ///
    pub fn parse_postfix_expression(&mut self) -> Result<Expression> {
        // We don't directly parse named-type's and Identifier's here as
        // the ABNF states. Rather the primary expression already
        // handle those. The ABNF is more specific for language reasons.
        let mut expr = self.parse_primary_expression()?;
        while let Some(token) = self.eat_any(&[Token::LeftSquare, Token::Dot, Token::LeftParen, Token::DoubleColon]) {
            match token.token {
                Token::LeftSquare => {
                    if self.eat(Token::DotDot).is_some() {
                        let right = if self.peek_token().as_ref() != &Token::RightSquare {
                            Some(Box::new(self.parse_expression()?))
                        } else {
                            None
                        };

                        let end = self.expect(Token::RightSquare)?;
                        expr = Expression::Access(AccessExpression::ArrayRange(ArrayRangeAccess {
                            span: expr.span() + &end,
                            array: Box::new(expr),
                            left: None,
                            right,
                        }));
                        continue;
                    }

                    let left = self.parse_expression()?;
                    if self.eat(Token::DotDot).is_some() {
                        let right = if self.peek_token().as_ref() != &Token::RightSquare {
                            Some(Box::new(self.parse_expression()?))
                        } else {
                            None
                        };

                        let end = self.expect(Token::RightSquare)?;
                        expr = Expression::Access(AccessExpression::ArrayRange(ArrayRangeAccess {
                            span: expr.span() + &end,
                            array: Box::new(expr),
                            left: Some(Box::new(left)),
                            right,
                        }));
                    } else {
                        let end = self.expect(Token::RightSquare)?;
                        expr = Expression::Access(AccessExpression::Array(ArrayAccess {
                            span: expr.span() + &end,
                            array: Box::new(expr),
                            index: Box::new(left),
                        }));
                    }
                }
                Token::Dot => {
                    if let Some(ident) = self.eat_identifier() {
                        expr = Expression::Access(AccessExpression::Member(MemberAccess {
                            span: expr.span() + &ident.span,
                            inner: Box::new(expr),
                            name: ident,
                            type_: None,
                        }));
                    } else if let Some((num, span)) = self.eat_int() {
                        expr = Expression::Access(AccessExpression::Tuple(TupleAccess {
                            span: expr.span() + &span,
                            tuple: Box::new(expr),
                            index: num,
                        }));
                    } else {
                        let next = self.peek()?;
                        return Err(ParserError::unexpected_str(&next.token, "int or ident", &next.span).into());
                    }
                }
                Token::LeftParen => {
                    let mut arguments = Vec::new();
                    let end_span;
                    loop {
                        if let Some(end) = self.eat(Token::RightParen) {
                            end_span = end.span;
                            break;
                        }
                        arguments.push(self.parse_expression()?);
                        if self.eat(Token::Comma).is_none() {
                            end_span = self.expect(Token::RightParen)?;
                            break;
                        }
                    }
                    expr = Expression::Call(CallExpression {
                        span: expr.span() + &end_span,
                        function: Box::new(expr),
                        arguments,
                    });
                }
                Token::DoubleColon => {
                    let ident = self.expect_ident()?;
                    expr = Expression::Access(AccessExpression::Static(StaticAccess {
                        span: expr.span() + &ident.span,
                        inner: Box::new(expr),
                        type_: None,
                        name: ident,
                    }));
                }
                _ => unreachable!("parse_postfix_expression_ shouldn't produce this"),
            }
        }
        Ok(expr)
    }

    ///
    /// Returns a [`SpreadOrExpression`] AST node if the next tokens represent a
    /// spread or expression.
    ///
    /// This method should only be called in the context of an array construction expression.
    ///
    pub fn parse_spread_or_expression(&mut self) -> Result<SpreadOrExpression> {
        Ok(if self.eat(Token::DotDotDot).is_some() {
            SpreadOrExpression::Spread(self.parse_expression()?)
        } else {
            SpreadOrExpression::Expression(self.parse_expression()?)
        })
    }

    /// Returns an [`Expression`] AST node if the next tokens represent a
    /// circuit initialization expression.
    pub fn parse_circuit_expression(&mut self, identifier: Identifier) -> Result<Expression> {
        let (members, _, span) = self.parse_list(Token::LeftCurly, Token::RightCurly, Token::Comma, |p| {
            Ok(Some(CircuitVariableInitializer {
                identifier: p.expect_ident()?,
                expression: p.eat(Token::Colon).map(|_| p.parse_expression()).transpose()?,
            }))
        })?;
        Ok(Expression::CircuitInit(CircuitInitExpression {
            span: &identifier.span + &span,
            name: identifier,
            members,
        }))
    }

    ///
    /// Returns an [`Expression`] AST node if the next tokens represent a
    /// tuple initialization expression or an affine group literal.
    ///
    pub fn parse_tuple_expression(&mut self, span: &Span) -> Result<Expression> {
        if let Some((left, right, span)) = self.eat_group_partial().transpose()? {
            return Ok(Expression::Value(ValueExpression::Group(Box::new(GroupValue::Tuple(
                GroupTuple {
                    span,
                    x: left,
                    y: right,
                },
            )))));
        }
        let mut args = Vec::new();
        let end_span;
        loop {
            let end = self.eat(Token::RightParen);
            if let Some(end) = end {
                end_span = end.span;
                break;
            }
            let expr = self.parse_expression()?;
            args.push(expr);
            if self.eat(Token::Comma).is_none() {
                end_span = self.expect(Token::RightParen)?;
                break;
            }
        }
        if args.len() == 1 {
            Ok(args.remove(0))
        } else {
            Ok(Expression::TupleInit(TupleInitExpression {
                span: span + &end_span,
                elements: args,
            }))
        }
    }

    ///
    /// Returns an [`Expression`] AST node if the next tokens represent an
    /// array initialization expression.
    ///
    pub fn parse_array_expression(&mut self, span: &Span) -> Result<Expression> {
        if let Some(end) = self.eat(Token::RightSquare) {
            return Ok(Expression::ArrayInline(ArrayInlineExpression {
                elements: Vec::new(),
                span: span + &end.span,
            }));
        }
        let first = self.parse_spread_or_expression()?;
        if self.eat(Token::Semicolon).is_some() {
            let dimensions = self
                .parse_array_dimensions()
                .map_err(|_| ParserError::unable_to_parse_array_dimensions(span))?;
            let end = self.expect(Token::RightSquare)?;
            let first = match first {
                SpreadOrExpression::Spread(first) => {
                    let span = span + first.span();
                    return Err(ParserError::spread_in_array_init(&span).into());
                }
                SpreadOrExpression::Expression(x) => x,
            };
            Ok(Expression::ArrayInit(ArrayInitExpression {
                span: span + &end,
                element: Box::new(first),
                dimensions,
            }))
        } else {
            let end_span;
            let mut elements = vec![first];
            loop {
                if let Some(token) = self.eat(Token::RightSquare) {
                    end_span = token.span;
                    break;
                }
                if elements.len() == 1 {
                    self.expect(Token::Comma)?;
                    if let Some(token) = self.eat(Token::RightSquare) {
                        end_span = token.span;
                        break;
                    }
                }
                elements.push(self.parse_spread_or_expression()?);
                if self.eat(Token::Comma).is_none() {
                    end_span = self.expect(Token::RightSquare)?;
                    break;
                }
            }
            Ok(Expression::ArrayInline(ArrayInlineExpression {
                elements,
                span: span + &end_span,
            }))
        }
    }

    ///
    /// Returns an [`Expression`] AST node if the next token is a primary expression:
    /// - Literals: field, group, unsigned integer, signed integer, boolean, address
    /// - Aggregate types: array, tuple
    /// - Identifiers: variables, keywords
    /// - self
    ///
    /// Returns an expression error if the token cannot be matched.
    ///
    pub fn parse_primary_expression(&mut self) -> Result<Expression> {
        let SpannedToken { token, span } = self.expect_any()?;
        Ok(match token {
            Token::Int(value) => {
                let type_ = self.eat_any(INT_TYPES);
                match type_ {
                    Some(SpannedToken {
                        token: Token::Field,
                        span: type_span,
                    }) => {
                        assert_no_whitespace(&span, &type_span, &value, "field")?;
                        Expression::Value(ValueExpression::Field(value, span + type_span))
                    }
                    Some(SpannedToken {
                        token: Token::Group,
                        span: type_span,
                    }) => {
                        assert_no_whitespace(&span, &type_span, &value, "group")?;
                        Expression::Value(ValueExpression::Group(Box::new(GroupValue::Single(
                            value,
                            span + type_span,
                        ))))
                    }
                    Some(SpannedToken { token, span: type_span }) => {
                        assert_no_whitespace(&span, &type_span, &value, &token.to_string())?;
                        Expression::Value(ValueExpression::Integer(
                            Self::token_to_int_type(token).expect("unknown int type token"),
                            value,
                            span + type_span,
                        ))
                    }
                    None => Expression::Value(ValueExpression::Implicit(value, span)),
                }
            }
            Token::True => Expression::Value(ValueExpression::Boolean("true".into(), span)),
            Token::False => Expression::Value(ValueExpression::Boolean("false".into(), span)),
            Token::AddressLit(value) => Expression::Value(ValueExpression::Address(value, span)),
            Token::CharLit(value) => Expression::Value(ValueExpression::Char(CharValue {
                character: value.into(),
                span,
            })),
            Token::StringLit(value) => Expression::Value(ValueExpression::String(value, span)),
            Token::LeftParen => self.parse_tuple_expression(&span)?,
            Token::LeftSquare => self.parse_array_expression(&span)?,
            Token::Ident(name) => {
                let ident = Identifier { name, span };
                if !self.disallow_circuit_construction && self.peek_token().as_ref() == &Token::LeftCurly {
                    self.parse_circuit_expression(ident)?
                } else {
                    Expression::Identifier(ident)
                }
            }
            Token::BigSelf => {
                let ident = Identifier {
                    name: sym::SelfUpper,
                    span,
                };
                if !self.disallow_circuit_construction && self.peek_token().as_ref() == &Token::LeftCurly {
                    self.parse_circuit_expression(ident)?
                } else {
                    Expression::Identifier(ident)
                }
            }
            Token::LittleSelf => Expression::Identifier(Identifier {
                name: sym::SelfLower,
                span,
            }),
            Token::Input => Expression::Identifier(Identifier { name: sym::input, span }),
            t if crate::type_::TYPE_TOKENS.contains(&t) => Expression::Identifier(Identifier {
                name: t.keyword_to_symbol().unwrap(),
                span,
            }),
            token => {
                return Err(ParserError::unexpected_str(token, "expression", &span).into());
            }
        })
    }
}
