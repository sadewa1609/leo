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

//! This module contains Visitor trait implementations for the AST.
//! It implements default methods for each node to be made
//! given the type of node its visiting.

use crate::*;

pub trait ExpressionVisitor<'a> {
    type AdditionalInput: Default;
    type Output;

    fn visit_expression(&mut self, input: &'a Expression, additional: &Self::AdditionalInput) -> Option<Self::Output> {
        match input {
            Expression::Identifier(expr) => self.visit_identifier(expr, additional),
            Expression::Value(expr) => self.visit_value(expr, additional),
            Expression::Binary(expr) => self.visit_binary(expr, additional),
            Expression::Unary(expr) => self.visit_unary(expr, additional),
            Expression::Ternary(expr) => self.visit_ternary(expr, additional),
            Expression::Call(expr) => self.visit_call(expr, additional),
            Expression::Err(expr) => self.visit_err(expr, additional),
        }
    }

    fn visit_identifier(
        &mut self,
        _input: &'a Identifier,
        _additional: &Self::AdditionalInput,
    ) -> Option<Self::Output> {
        None
    }

    fn visit_value(
        &mut self,
        _input: &'a ValueExpression,
        _additional: &Self::AdditionalInput,
    ) -> Option<Self::Output> {
        None
    }

    fn visit_binary(
        &mut self,
        input: &'a BinaryExpression,
        additional: &Self::AdditionalInput,
    ) -> Option<Self::Output> {
        self.visit_expression(&input.left, additional);
        self.visit_expression(&input.right, additional);
        None
    }

    fn visit_unary(&mut self, input: &'a UnaryExpression, additional: &Self::AdditionalInput) -> Option<Self::Output> {
        self.visit_expression(&input.inner, additional);
        None
    }

    fn visit_ternary(
        &mut self,
        input: &'a TernaryExpression,
        additional: &Self::AdditionalInput,
    ) -> Option<Self::Output> {
        self.visit_expression(&input.condition, additional);
        self.visit_expression(&input.if_true, additional);
        self.visit_expression(&input.if_false, additional);
        None
    }

    fn visit_call(&mut self, input: &'a CallExpression, additional: &Self::AdditionalInput) -> Option<Self::Output> {
        input.arguments.iter().for_each(|expr| {
            self.visit_expression(expr, additional);
        });
        None
    }

    fn visit_err(&mut self, _input: &'a ErrExpression, _additional: &Self::AdditionalInput) -> Option<Self::Output> {
        None
    }
}

pub trait StatementVisitor<'a>: ExpressionVisitor<'a> {
    fn visit_statement(&mut self, input: &'a Statement) {
        match input {
            Statement::Return(stmt) => self.visit_return(stmt),
            Statement::Definition(stmt) => self.visit_definition(stmt),
            Statement::Assign(stmt) => self.visit_assign(stmt),
            Statement::Conditional(stmt) => self.visit_conditional(stmt),
            Statement::Iteration(stmt) => self.visit_iteration(stmt),
            Statement::Console(stmt) => self.visit_console(stmt),
            Statement::Block(stmt) => self.visit_block(stmt),
        }
    }

    fn visit_return(&mut self, input: &'a ReturnStatement) {
        self.visit_expression(&input.expression, &Default::default());
    }

    fn visit_definition(&mut self, input: &'a DefinitionStatement) {
        self.visit_expression(&input.value, &Default::default());
    }

    fn visit_assign(&mut self, input: &'a AssignStatement) {
        self.visit_expression(&input.value, &Default::default());
    }

    fn visit_conditional(&mut self, input: &'a ConditionalStatement) {
        self.visit_expression(&input.condition, &Default::default());
        self.visit_block(&input.block);
        if let Some(stmt) = input.next.as_ref() {
            self.visit_statement(stmt);
        }
    }

    fn visit_iteration(&mut self, input: &'a IterationStatement) {
        self.visit_expression(&input.start, &Default::default());
        self.visit_expression(&input.stop, &Default::default());
        self.visit_block(&input.block);
    }

    fn visit_console(&mut self, input: &'a ConsoleStatement) {
        match &input.function {
            ConsoleFunction::Assert(expr) => self.visit_expression(expr, &Default::default()),
            ConsoleFunction::Error(fmt) | ConsoleFunction::Log(fmt) => {
                fmt.parameters.iter().for_each(|expr| {
                    self.visit_expression(expr, &Default::default());
                });
                None
            }
        };
    }

    fn visit_block(&mut self, input: &'a Block) {
        input.statements.iter().for_each(|stmt| self.visit_statement(stmt));
    }
}

pub trait ProgramVisitor<'a>: StatementVisitor<'a> {
    fn visit_program(&mut self, input: &'a Program) {
        input
            .functions
            .values()
            .for_each(|function| self.visit_function(function));
    }

    fn visit_function(&mut self, input: &'a Function) {
        self.visit_block(&input.block);
    }
}