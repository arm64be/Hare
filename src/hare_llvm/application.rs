use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::{LlvmError, TargetLayout};

#[derive(Clone, Debug, PartialEq)]
enum TokenKind {
    Identifier(Box<str>),
    Number(f64),
    String(Box<str>),
    Punct(u8),
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
struct Token {
    kind: TokenKind,
    offset: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnaryOperator {
    Plus,
    Negate,
}

#[derive(Clone, Debug, PartialEq)]
enum StaticExpression {
    Number(f64),
    String(Box<str>),
    Boolean(bool),
    Null,
    Undefined,
    Identifier(Box<str>),
    Unary {
        operator: UnaryOperator,
        value: Box<StaticExpression>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<StaticExpression>,
        right: Box<StaticExpression>,
    },
    Call {
        callee: Box<str>,
        arguments: Vec<StaticExpression>,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct StaticFunction {
    parameters: Vec<Box<str>>,
    result: StaticExpression,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum StaticValue {
    Number(f64),
    String(Box<str>),
    Boolean(bool),
    Null,
    Undefined,
}

impl StaticValue {
    fn to_js_string(&self) -> Result<String, LlvmError> {
        match self {
            Self::Number(value) if value.is_nan() => Ok("NaN".into()),
            Self::Number(value) if *value == f64::INFINITY => Ok("Infinity".into()),
            Self::Number(value) if *value == f64::NEG_INFINITY => Ok("-Infinity".into()),
            Self::Number(value) if *value == 0.0 => Ok("0".into()),
            Self::Number(value)
                if value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0 =>
            {
                Ok(value.to_string())
            }
            Self::Number(_) => Err(static_error(
                "non-integral Number formatting is not yet admitted by native application lowering",
            )),
            Self::String(value) => Ok(value.to_string()),
            Self::Boolean(value) => Ok(value.to_string()),
            Self::Null => Ok("null".into()),
            Self::Undefined => Ok("undefined".into()),
        }
    }

    fn to_console_string(&self) -> Result<String, LlvmError> {
        if let Self::Number(value) = self
            && *value == 0.0
            && value.is_sign_negative()
        {
            return Ok("-0".into());
        }
        self.to_js_string()
    }

    fn to_number(&self) -> Result<f64, LlvmError> {
        match self {
            Self::Number(value) => Ok(*value),
            Self::Boolean(true) => Ok(1.0),
            Self::Boolean(false) | Self::Null => Ok(0.0),
            Self::Undefined => Ok(f64::NAN),
            Self::String(_) => Err(static_error(
                "String-to-Number coercion is not yet admitted by native application lowering",
            )),
        }
    }
}

/// A build-time-lowered application entry. The emitted object contains the
/// application-specific entry symbol and calls only the versioned Hare native
/// ABI; it contains neither JSC bytecode nor a runtime compiler request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeApplication {
    target: TargetLayout,
    stdout: Box<[u8]>,
}

impl NativeApplication {
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn emit_llvm_ir(&self) -> String {
        let mut output = String::new();
        output.push_str("; Hare native application\n");
        output.push_str("source_filename = \"hare-application\"\n");
        let _ = writeln!(
            output,
            "target datalayout = \"{}\"",
            self.target.data_layout
        );
        let _ = writeln!(output, "target triple = \"{}\"\n", self.target.triple);

        let byte_count = self.stdout.len();
        if byte_count != 0 {
            let _ = write!(
                output,
                "@.hare.stdout = private unnamed_addr constant [{byte_count} x i8] c\""
            );
            for byte in &self.stdout {
                match byte {
                    b' '..=b'~' if !matches!(byte, b'"' | b'\\') => {
                        output.push(char::from(*byte));
                    }
                    _ => {
                        let _ = write!(output, "\\{byte:02X}");
                    }
                }
            }
            output.push_str("\", align 1\n\n");
        }

        output.push_str("declare i64 @Bun__Hare__writeStdout(ptr, i64) nounwind\n\n");
        output.push_str(
            "define i32 @Bun__Hare__nativeApplicationEntry(i32 %argc, ptr %argv) nounwind {\nentry:\n",
        );
        if byte_count == 0 {
            output.push_str("  ret i32 0\n");
        } else {
            let _ = writeln!(
                output,
                "  %written = call i64 @Bun__Hare__writeStdout(ptr @.hare.stdout, i64 {byte_count})"
            );
            let _ = writeln!(output, "  %ok = icmp eq i64 %written, {byte_count}");
            output.push_str("  %status = select i1 %ok, i32 0, i32 1\n  ret i32 %status\n");
        }
        output.push_str("}\n");
        output
    }
}

/// Lower the statically provable, side-effecting shell of a bundled program.
///
/// This first native convergence path intentionally accepts only declarations,
/// proven-pure scalar expressions, and `console.log` writes. Any other source
/// is a compile error; it is never serialized for runtime execution.
pub fn compile_static_application(
    source: &[u8],
    target: TargetLayout,
) -> Result<NativeApplication, LlvmError> {
    let tokens = Lexer::new(source).lex()?;
    let stdout = Parser::new(tokens).parse_program()?;
    Ok(NativeApplication {
        target,
        stdout: stdout.into_boxed_slice(),
    })
}

pub(super) fn evaluate_static_expression(source: &[u8]) -> Result<StaticValue, LlvmError> {
    let tokens = Lexer::new(source).lex()?;
    let mut parser = Parser::new(tokens);
    let expression = parser.parse_expression()?;
    if !matches!(parser.current().kind, TokenKind::Eof) {
        return parser.error("unexpected token after static expression");
    }
    parser.evaluate(&expression, &BTreeMap::new(), 0)
}

struct Lexer<'a> {
    source: &'a [u8],
    offset: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a [u8]) -> Self {
        Self { source, offset: 0 }
    }

    fn lex(mut self) -> Result<Vec<Token>, LlvmError> {
        let mut tokens = Vec::new();
        while self.offset < self.source.len() {
            self.skip_trivia()?;
            if self.offset == self.source.len() {
                break;
            }
            let offset = self.offset;
            let byte = self.source[self.offset];
            let kind = if byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$') {
                self.lex_identifier()
            } else if byte.is_ascii_digit() {
                self.lex_number()?
            } else if matches!(byte, b'\'' | b'"') {
                self.lex_string()?
            } else if b"(){}[],;.=+-*/%".contains(&byte) {
                self.offset += 1;
                TokenKind::Punct(byte)
            } else {
                return Err(static_error(format!(
                    "unsupported source byte 0x{byte:02x} at byte {offset}"
                )));
            };
            tokens.push(Token { kind, offset });
        }
        tokens.push(Token {
            kind: TokenKind::Eof,
            offset: self.source.len(),
        });
        Ok(tokens)
    }

    fn skip_trivia(&mut self) -> Result<(), LlvmError> {
        loop {
            while self
                .source
                .get(self.offset)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.offset += 1;
            }
            if self.source.get(self.offset..self.offset + 2) == Some(b"//") {
                self.offset += 2;
                while self
                    .source
                    .get(self.offset)
                    .is_some_and(|byte| *byte != b'\n')
                {
                    self.offset += 1;
                }
                continue;
            }
            if self.source.get(self.offset..self.offset + 2) == Some(b"/*") {
                let start = self.offset;
                self.offset += 2;
                while self.source.get(self.offset..self.offset + 2) != Some(b"*/") {
                    if self.offset + 1 >= self.source.len() {
                        return Err(static_error(format!(
                            "unterminated block comment at byte {start}"
                        )));
                    }
                    self.offset += 1;
                }
                self.offset += 2;
                continue;
            }
            return Ok(());
        }
    }

    fn lex_identifier(&mut self) -> TokenKind {
        let start = self.offset;
        self.offset += 1;
        while self
            .source
            .get(self.offset)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
        {
            self.offset += 1;
        }
        TokenKind::Identifier(
            String::from_utf8_lossy(&self.source[start..self.offset])
                .into_owned()
                .into_boxed_str(),
        )
    }

    fn lex_number(&mut self) -> Result<TokenKind, LlvmError> {
        let start = self.offset;
        self.offset += 1;
        while self.source.get(self.offset).is_some_and(u8::is_ascii_digit) {
            self.offset += 1;
        }
        if self.source.get(self.offset) == Some(&b'.') {
            self.offset += 1;
            while self.source.get(self.offset).is_some_and(u8::is_ascii_digit) {
                self.offset += 1;
            }
        }
        let text = core::str::from_utf8(&self.source[start..self.offset])
            .map_err(|_| static_error(format!("invalid number at byte {start}")))?;
        let value = text
            .parse::<f64>()
            .map_err(|_| static_error(format!("invalid number at byte {start}")))?;
        Ok(TokenKind::Number(value))
    }

    fn lex_string(&mut self) -> Result<TokenKind, LlvmError> {
        let start = self.offset;
        let quote = self.source[self.offset];
        self.offset += 1;
        let mut value = String::new();
        while let Some(byte) = self.source.get(self.offset).copied() {
            self.offset += 1;
            if byte == quote {
                return Ok(TokenKind::String(value.into_boxed_str()));
            }
            if byte == b'\n' || byte == b'\r' {
                return Err(static_error(format!(
                    "unterminated string literal at byte {start}"
                )));
            }
            if byte != b'\\' {
                if !byte.is_ascii() {
                    return Err(static_error(format!(
                        "non-ASCII literal source is not yet admitted at byte {}",
                        self.offset - 1
                    )));
                }
                value.push(char::from(byte));
                continue;
            }
            let escape_offset = self.offset;
            let escaped = self.source.get(self.offset).copied().ok_or_else(|| {
                static_error(format!("unterminated escape at byte {escape_offset}"))
            })?;
            self.offset += 1;
            match escaped {
                b'n' => value.push('\n'),
                b'r' => value.push('\r'),
                b't' => value.push('\t'),
                b'b' => value.push('\u{0008}'),
                b'f' => value.push('\u{000c}'),
                b'v' => value.push('\u{000b}'),
                b'0' => value.push('\0'),
                b'\\' => value.push('\\'),
                b'\'' => value.push('\''),
                b'"' => value.push('"'),
                b'x' => value.push(self.lex_hex_escape(2, escape_offset)?),
                b'u' => value.push(self.lex_hex_escape(4, escape_offset)?),
                _ => {
                    return Err(static_error(format!(
                        "unsupported string escape at byte {escape_offset}"
                    )));
                }
            }
        }
        Err(static_error(format!(
            "unterminated string literal at byte {start}"
        )))
    }

    fn lex_hex_escape(&mut self, digits: usize, offset: usize) -> Result<char, LlvmError> {
        let end = self
            .offset
            .checked_add(digits)
            .filter(|end| *end <= self.source.len())
            .ok_or_else(|| static_error(format!("short hex escape at byte {offset}")))?;
        let text = core::str::from_utf8(&self.source[self.offset..end])
            .map_err(|_| static_error(format!("invalid hex escape at byte {offset}")))?;
        let value = u32::from_str_radix(text, 16)
            .map_err(|_| static_error(format!("invalid hex escape at byte {offset}")))?;
        self.offset = end;
        char::from_u32(value)
            .ok_or_else(|| static_error(format!("invalid Unicode scalar at byte {offset}")))
    }
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    functions: BTreeMap<Box<str>, StaticFunction>,
    globals: BTreeMap<Box<str>, StaticValue>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            index: 0,
            functions: BTreeMap::new(),
            globals: BTreeMap::new(),
        }
    }

    fn parse_program(mut self) -> Result<Vec<u8>, LlvmError> {
        let mut stdout = Vec::new();
        while !matches!(self.current().kind, TokenKind::Eof) {
            if self.take_punct(b';') {
                continue;
            }
            if self.take_keyword("function") {
                self.parse_function()?;
                continue;
            }
            if self.take_keyword("const") || self.take_keyword("let") || self.take_keyword("var") {
                self.parse_binding()?;
                continue;
            }
            if self.is_console_log() {
                self.parse_console_log(&mut stdout)?;
                continue;
            }
            if matches!(self.current().kind, TokenKind::String(_)) {
                let directive = self.parse_expression()?;
                self.expect_punct(b';')?;
                if !matches!(directive, StaticExpression::String(_)) {
                    return self.error("only string directives are admitted here");
                }
                continue;
            }
            return self.error("unsupported top-level statement");
        }
        Ok(stdout)
    }

    fn parse_function(&mut self) -> Result<(), LlvmError> {
        let name = self.take_identifier()?;
        self.expect_punct(b'(')?;
        let mut parameters = Vec::new();
        if !self.take_punct(b')') {
            loop {
                parameters.push(self.take_identifier()?);
                if self.take_punct(b')') {
                    break;
                }
                self.expect_punct(b',')?;
            }
        }
        self.expect_punct(b'{')?;
        self.expect_keyword("return")?;
        let result = self.parse_expression()?;
        let _ = self.take_punct(b';');
        self.expect_punct(b'}')?;
        if self
            .functions
            .insert(name.clone(), StaticFunction { parameters, result })
            .is_some()
        {
            return self.error(format!("duplicate function {name}"));
        }
        Ok(())
    }

    fn parse_binding(&mut self) -> Result<(), LlvmError> {
        let name = self.take_identifier()?;
        self.expect_punct(b'=')?;
        let expression = self.parse_expression()?;
        self.expect_punct(b';')?;
        let value = self.evaluate(&expression, &BTreeMap::new(), 0)?;
        if self.globals.insert(name.clone(), value).is_some() {
            return self.error(format!("duplicate binding {name}"));
        }
        Ok(())
    }

    fn is_console_log(&self) -> bool {
        self.keyword_at(0, "console")
            && self.punct_at(1, b'.')
            && self.keyword_at(2, "log")
            && self.punct_at(3, b'(')
    }

    fn parse_console_log(&mut self, stdout: &mut Vec<u8>) -> Result<(), LlvmError> {
        self.expect_keyword("console")?;
        self.expect_punct(b'.')?;
        self.expect_keyword("log")?;
        self.expect_punct(b'(')?;
        if !self.take_punct(b')') {
            let expression = self.parse_expression()?;
            let value = self.evaluate(&expression, &BTreeMap::new(), 0)?;
            stdout.extend_from_slice(value.to_console_string()?.as_bytes());
            self.expect_punct(b')')?;
        }
        self.expect_punct(b';')?;
        stdout.push(b'\n');
        Ok(())
    }

    fn parse_expression(&mut self) -> Result<StaticExpression, LlvmError> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Result<StaticExpression, LlvmError> {
        let mut expression = self.parse_multiplicative()?;
        loop {
            let operator = if self.take_punct(b'+') {
                BinaryOperator::Add
            } else if self.take_punct(b'-') {
                BinaryOperator::Subtract
            } else {
                return Ok(expression);
            };
            expression = StaticExpression::Binary {
                operator,
                left: Box::new(expression),
                right: Box::new(self.parse_multiplicative()?),
            };
        }
    }

    fn parse_multiplicative(&mut self) -> Result<StaticExpression, LlvmError> {
        let mut expression = self.parse_unary()?;
        loop {
            let operator = if self.take_punct(b'*') {
                BinaryOperator::Multiply
            } else if self.take_punct(b'/') {
                BinaryOperator::Divide
            } else if self.take_punct(b'%') {
                BinaryOperator::Remainder
            } else {
                return Ok(expression);
            };
            expression = StaticExpression::Binary {
                operator,
                left: Box::new(expression),
                right: Box::new(self.parse_unary()?),
            };
        }
    }

    fn parse_unary(&mut self) -> Result<StaticExpression, LlvmError> {
        if self.take_punct(b'+') {
            return Ok(StaticExpression::Unary {
                operator: UnaryOperator::Plus,
                value: Box::new(self.parse_unary()?),
            });
        }
        if self.take_punct(b'-') {
            return Ok(StaticExpression::Unary {
                operator: UnaryOperator::Negate,
                value: Box::new(self.parse_unary()?),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<StaticExpression, LlvmError> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Number(value) => {
                self.index += 1;
                Ok(StaticExpression::Number(value))
            }
            TokenKind::String(value) => {
                self.index += 1;
                Ok(StaticExpression::String(value))
            }
            TokenKind::Identifier(name) => {
                self.index += 1;
                let bare = match name.as_ref() {
                    "true" => StaticExpression::Boolean(true),
                    "false" => StaticExpression::Boolean(false),
                    "null" => StaticExpression::Null,
                    "undefined" => StaticExpression::Undefined,
                    _ => StaticExpression::Identifier(name.clone()),
                };
                if self.take_punct(b'(') {
                    if !matches!(bare, StaticExpression::Identifier(_)) {
                        return self.error("literal is not callable");
                    }
                    let mut arguments = Vec::new();
                    if !self.take_punct(b')') {
                        loop {
                            arguments.push(self.parse_expression()?);
                            if self.take_punct(b')') {
                                break;
                            }
                            self.expect_punct(b',')?;
                        }
                    }
                    Ok(StaticExpression::Call {
                        callee: name,
                        arguments,
                    })
                } else {
                    Ok(bare)
                }
            }
            TokenKind::Punct(b'(') => {
                self.index += 1;
                let expression = self.parse_expression()?;
                self.expect_punct(b')')?;
                Ok(expression)
            }
            _ => self.error("expected a statically provable scalar expression"),
        }
    }

    fn evaluate(
        &self,
        expression: &StaticExpression,
        locals: &BTreeMap<Box<str>, StaticValue>,
        depth: usize,
    ) -> Result<StaticValue, LlvmError> {
        if depth > 64 {
            return Err(static_error("pure call graph exceeds 64 frames"));
        }
        match expression {
            StaticExpression::Number(value) => Ok(StaticValue::Number(*value)),
            StaticExpression::String(value) => Ok(StaticValue::String(value.clone())),
            StaticExpression::Boolean(value) => Ok(StaticValue::Boolean(*value)),
            StaticExpression::Null => Ok(StaticValue::Null),
            StaticExpression::Undefined => Ok(StaticValue::Undefined),
            StaticExpression::Identifier(name) => locals
                .get(name)
                .or_else(|| self.globals.get(name))
                .cloned()
                .or_else(|| match name.as_ref() {
                    "NaN" => Some(StaticValue::Number(f64::NAN)),
                    "Infinity" => Some(StaticValue::Number(f64::INFINITY)),
                    _ => None,
                })
                .ok_or_else(|| static_error(format!("unknown scalar binding {name}"))),
            StaticExpression::Unary { operator, value } => {
                let value = self.evaluate(value, locals, depth)?.to_number()?;
                Ok(StaticValue::Number(match operator {
                    UnaryOperator::Plus => value,
                    UnaryOperator::Negate => -value,
                }))
            }
            StaticExpression::Binary {
                operator,
                left,
                right,
            } => {
                let left = self.evaluate(left, locals, depth)?;
                let right = self.evaluate(right, locals, depth)?;
                if *operator == BinaryOperator::Add
                    && (matches!(left, StaticValue::String(_))
                        || matches!(right, StaticValue::String(_)))
                {
                    return Ok(StaticValue::String(
                        format!("{}{}", left.to_js_string()?, right.to_js_string()?)
                            .into_boxed_str(),
                    ));
                }
                let left = left.to_number()?;
                let right = right.to_number()?;
                Ok(StaticValue::Number(match operator {
                    BinaryOperator::Add => left + right,
                    BinaryOperator::Subtract => left - right,
                    BinaryOperator::Multiply => left * right,
                    BinaryOperator::Divide => left / right,
                    BinaryOperator::Remainder => left % right,
                }))
            }
            StaticExpression::Call { callee, arguments } => {
                let function = self
                    .functions
                    .get(callee)
                    .ok_or_else(|| static_error(format!("unknown pure function {callee}")))?;
                if function.parameters.len() != arguments.len() {
                    return Err(static_error(format!(
                        "pure function {callee} expects {} argument(s), received {}",
                        function.parameters.len(),
                        arguments.len()
                    )));
                }
                let mut call_locals = BTreeMap::new();
                for (parameter, argument) in function.parameters.iter().zip(arguments) {
                    call_locals.insert(
                        parameter.clone(),
                        self.evaluate(argument, locals, depth + 1)?,
                    );
                }
                self.evaluate(&function.result, &call_locals, depth + 1)
            }
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn keyword_at(&self, lookahead: usize, expected: &str) -> bool {
        matches!(
            self.tokens.get(self.index + lookahead).map(|token| &token.kind),
            Some(TokenKind::Identifier(actual)) if actual.as_ref() == expected
        )
    }

    fn punct_at(&self, lookahead: usize, expected: u8) -> bool {
        matches!(
            self.tokens.get(self.index + lookahead).map(|token| &token.kind),
            Some(TokenKind::Punct(actual)) if *actual == expected
        )
    }

    fn take_keyword(&mut self, expected: &str) -> bool {
        if self.keyword_at(0, expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect_keyword(&mut self, expected: &str) -> Result<(), LlvmError> {
        if self.take_keyword(expected) {
            Ok(())
        } else {
            self.error(format!("expected keyword {expected}"))
        }
    }

    fn take_identifier(&mut self) -> Result<Box<str>, LlvmError> {
        let TokenKind::Identifier(value) = self.current().kind.clone() else {
            return self.error("expected identifier");
        };
        self.index += 1;
        Ok(value)
    }

    fn take_punct(&mut self, expected: u8) -> bool {
        if self.punct_at(0, expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect_punct(&mut self, expected: u8) -> Result<(), LlvmError> {
        if self.take_punct(expected) {
            Ok(())
        } else {
            self.error(format!("expected '{}'", char::from(expected)))
        }
    }

    fn error<T>(&self, message: impl Into<String>) -> Result<T, LlvmError> {
        Err(static_error(format!(
            "{} at byte {}",
            message.into(),
            self.current().offset
        )))
    }
}

fn static_error(message: impl Into<String>) -> LlvmError {
    LlvmError::StaticApplication(message.into().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_pure_nested_call_to_native_stdout() {
        let application = compile_static_application(
            br#"
                // bundled source
                function nested(value) { return value + 1; }
                console.log(nested(41));
            "#,
            TargetLayout::host().unwrap(),
        )
        .unwrap();
        assert_eq!(application.stdout(), b"42\n");
        let ir = application.emit_llvm_ir();
        assert!(ir.contains("@Bun__Hare__nativeApplicationEntry"));
        assert!(ir.contains("c\"42\\0A\""));
        assert!(!ir.contains("bytecode"));
    }

    #[test]
    fn rejects_effectful_or_unknown_source() {
        let error = compile_static_application(
            b"fetch('https://example.com');",
            TargetLayout::host().unwrap(),
        )
        .unwrap_err();
        assert!(matches!(error, LlvmError::StaticApplication(_)));
    }

    #[test]
    fn rejects_console_formatting_and_unproven_number_formatting() {
        for source in [
            b"console.log('%s', 1);".as_slice(),
            b"console.log(1 / 2);".as_slice(),
        ] {
            assert!(matches!(
                compile_static_application(source, TargetLayout::host().unwrap()),
                Err(LlvmError::StaticApplication(_))
            ));
        }
    }
}
