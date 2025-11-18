//! Streaming tokenizer for `.isa`-family source files.

use super::error::IsaError;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Colon,
    DoubleColon,
    DoublePeriod,
    DirectTo,
    BitExpr,
    Range,
    Identifier,
    Number,
    String,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LessThan,
    GreaterThan,
    Pipe,
    Equals,
    Comma,
    Semicolon,
    Question,
    Dash,
    Plus,
    Bang,
    Percent,
    Caret,
    Ampersand,
    Asterisk,
    Tilde,
    Backtick,
    Apostrophe,
    EOF,
}

#[derive(Clone, Copy)]
enum Radix {
    Binary,
    Octal,
    Decimal,
    Hex,
}

impl Radix {
    fn accepts(self, ch: char) -> bool {
        match self {
            Radix::Binary => matches!(ch, '0' | '1'),
            Radix::Octal => matches!(ch, '0'..='7'),
            Radix::Decimal => ch.is_ascii_digit(),
            Radix::Hex => ch.is_ascii_hexdigit(),
        }
    }
}

pub struct Lexer<'src> {
    src: &'src str,
    offset: usize,
    line: usize,
    column: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self {
            src,
            offset: 0,
            line: 1,
            column: 0,
        }
    }

    /// Produces the next token.
    pub fn next_token(&mut self) -> Result<Token, IsaError> {
        self.skip_ignorable();
        if self.is_eof() {
            let (line, column) = self.position();
            return Ok(self.make_token(TokenKind::EOF, "", line, column));
        }
        let ch = self.peek_char().expect("not eof");

        if ch == '.' && self.peek_next_char() == Some('.') {
            return Ok(self.consume_double_period());
        }

        match ch {
            ':' => Ok(self.consume_colon()),
            '{' => Ok(self.consume_single(TokenKind::LBrace)),
            '}' => Ok(self.consume_single(TokenKind::RBrace)),
            '(' => Ok(self.consume_single(TokenKind::LParen)),
            ')' => Ok(self.consume_single(TokenKind::RParen)),
            '<' => Ok(self.consume_single(TokenKind::LessThan)),
            '>' => Ok(self.consume_single(TokenKind::GreaterThan)),
            '[' => self.consume_range_token(),
            '|' => Ok(self.consume_single(TokenKind::Pipe)),
            '=' => Ok(self.consume_single(TokenKind::Equals)),
            ',' => Ok(self.consume_single(TokenKind::Comma)),
            ';' => Ok(self.consume_single(TokenKind::Semicolon)),
            '@' => self.consume_bit_expr(),
            '?' => Ok(self.consume_single(TokenKind::Question)),
            '-' => {
                if self.peek_next_char() == Some('>') {
                    Ok(self.consume_direct_to())
                } else if self
                    .peek_next_char()
                    .map(|next| next.is_ascii_digit())
                    .unwrap_or(false)
                {
                    self.consume_number()
                } else {
                    Ok(self.consume_single(TokenKind::Dash))
                }
            }
            '+' => Ok(self.consume_single(TokenKind::Plus)),
            '!' => Ok(self.consume_single(TokenKind::Bang)),
            '%' => Ok(self.consume_single(TokenKind::Percent)),
            '^' => Ok(self.consume_single(TokenKind::Caret)),
            '&' => Ok(self.consume_single(TokenKind::Ampersand)),
            '*' => Ok(self.consume_single(TokenKind::Asterisk)),
            '~' => Ok(self.consume_single(TokenKind::Tilde)),
            '`' => Ok(self.consume_single(TokenKind::Backtick)),
            '\'' => Ok(self.consume_single(TokenKind::Apostrophe)),
            '#' => {
                self.consume_line_comment();
                self.next_token()
            }
            '"' => self.consume_string(),
            ch if ch.is_ascii_digit() => self.consume_number(),
            ch if is_ident_start(ch) => self.consume_identifier(),
            _ => Err(IsaError::Lexer(format!(
                "unexpected character '{}', line {}, column {}",
                ch, self.line, self.column + 1
            ))),
        }
    }

    fn consume_identifier(&mut self) -> Result<Token, IsaError> {
        let start = self.offset;
        let (line, column) = self.position();
        self.advance_char();
        while let Some(ch) = self.peek_char() {
            if is_ident_part(ch) {
                self.advance_char();
            } else {
                break;
            }
        }
        Ok(self.make_token_from_span(TokenKind::Identifier, start, self.offset, line, column))
    }

    fn consume_number(&mut self) -> Result<Token, IsaError> {
        let start = self.offset;
        let (line, column) = self.position();
        let mut radix = Radix::Decimal;
        let mut digits_consumed = 0usize;
        let mut require_digit = false;

        if self.peek_char() == Some('-') {
            self.advance_char();
            match self.peek_char() {
                Some(next) if next.is_ascii_digit() => {}
                _ => {
                    return Err(IsaError::Lexer(
                        "numeric literal requires digits after '-'".into(),
                    ));
                }
            }
        }

        if self.peek_char() == Some('0') {
            self.advance_char();
            digits_consumed += 1;
            if let Some(next) = self.peek_char() {
                match next {
                    'x' | 'X' => {
                        radix = Radix::Hex;
                        self.advance_char();
                        digits_consumed = 0;
                        require_digit = true;
                    }
                    'b' | 'B' => {
                        radix = Radix::Binary;
                        self.advance_char();
                        digits_consumed = 0;
                        require_digit = true;
                    }
                    'o' | 'O' => {
                        radix = Radix::Octal;
                        self.advance_char();
                        digits_consumed = 0;
                        require_digit = true;
                    }
                    _ => {}
                }
            }
        } else {
            self.advance_char();
            digits_consumed += 1;
        }

        while let Some(ch) = self.peek_char() {
            if ch == '_' {
                self.advance_char();
                continue;
            }
            if radix.accepts(ch) {
                self.advance_char();
                digits_consumed += 1;
            } else {
                break;
            }
        }

        if require_digit && digits_consumed == 0 {
            return Err(IsaError::Lexer("numeric literal requires digits after prefix".into()));
        }

        Ok(self.make_token_from_span(TokenKind::Number, start, self.offset, line, column))
    }

    fn consume_colon(&mut self) -> Token {
        let start = self.offset;
        let (line, column) = self.position();
        self.advance_char();
        if self.peek_char() == Some(':') {
            self.advance_char();
            self.make_token_from_span(TokenKind::DoubleColon, start, self.offset, line, column)
        } else {
            self.make_token_from_span(TokenKind::Colon, start, self.offset, line, column)
        }
    }

    fn consume_double_period(&mut self) -> Token {
        let start = self.offset;
        let (line, column) = self.position();
        self.advance_char();
        self.advance_char();
        self.make_token_from_span(TokenKind::DoublePeriod, start, self.offset, line, column)
    }

    fn consume_direct_to(&mut self) -> Token {
        let start = self.offset;
        let (line, column) = self.position();
        self.advance_char();
        self.advance_char();
        self.make_token_from_span(TokenKind::DirectTo, start, self.offset, line, column)
    }

    fn consume_bit_expr(&mut self) -> Result<Token, IsaError> {
        let start = self.offset;
        let (line, column) = self.position();
        self.advance_char(); // '@'

        if self.peek_char() != Some('(') {
            return Err(IsaError::Lexer("bit expression must start with '@('".into()));
        }
        self.advance_char(); // consume '('
        let mut depth = 1usize;

        while let Some(ch) = self.peek_char() {
            self.advance_char();
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }

        if depth != 0 {
            return Err(IsaError::Lexer("unterminated bit expression".into()));
        }

        Ok(self.make_token_from_span(TokenKind::BitExpr, start, self.offset, line, column))
    }

    fn consume_range_token(&mut self) -> Result<Token, IsaError> {
        enum RangeOperator {
            Size,
            Inclusive,
        }

        let start_offset = self.offset;
        let (line, column) = self.position();
        self.advance_char(); // '['
        self.skip_inline_whitespace();

        self.consume_range_literal()?;
        self.skip_inline_whitespace();

        let operator = match (self.peek_char(), self.peek_next_char()) {
            (Some('+'), _) => {
                self.advance_char();
                RangeOperator::Size
            }
            (Some('.'), Some('.')) => {
                self.advance_char();
                self.advance_char();
                RangeOperator::Inclusive
            }
            _ => {
                return Err(IsaError::Lexer(
                    "range must use '+' or '..' after the starting literal".into(),
                ));
            }
        };

        self.skip_inline_whitespace();
        self.consume_range_literal()?;
        if matches!(operator, RangeOperator::Size) {
            self.consume_optional_size_unit()?;
        }

        self.skip_inline_whitespace();
        if self.peek_char() != Some(']') {
            return Err(IsaError::Lexer("range missing closing ']'".into()));
        }
        self.advance_char();

        Ok(self.make_token_from_span(TokenKind::Range, start_offset, self.offset, line, column))
    }

    fn consume_range_literal(&mut self) -> Result<(), IsaError> {
        let mut radix = Radix::Decimal;
        let mut digits_consumed = 0usize;
        let mut require_digit = false;

        if let Some(sign @ ('+' | '-')) = self.peek_char() {
            self.advance_char();
            if sign == '+' {
                // optional plus; nothing more to do
            }
        }

        match self.peek_char() {
            Some('0') => {
                self.advance_char();
                digits_consumed += 1;
                if let Some(next) = self.peek_char() {
                    match next {
                        'x' | 'X' => {
                            radix = Radix::Hex;
                            self.advance_char();
                            digits_consumed = 0;
                            require_digit = true;
                        }
                        'b' | 'B' => {
                            radix = Radix::Binary;
                            self.advance_char();
                            digits_consumed = 0;
                            require_digit = true;
                        }
                        'o' | 'O' => {
                            radix = Radix::Octal;
                            self.advance_char();
                            digits_consumed = 0;
                            require_digit = true;
                        }
                        _ => {}
                    }
                }
            }
            Some(ch) if ch.is_ascii_digit() => {
                self.advance_char();
                digits_consumed += 1;
            }
            _ => {
                return Err(IsaError::Lexer("expected numeric literal in range".into()));
            }
        }

        while let Some(ch) = self.peek_char() {
            if ch == '_' {
                self.advance_char();
                continue;
            }
            if radix.accepts(ch) {
                self.advance_char();
                digits_consumed += 1;
            } else {
                break;
            }
        }

        if require_digit && digits_consumed == 0 {
            return Err(IsaError::Lexer("range literal missing digits".into()));
        }

        Ok(())
    }

    fn consume_optional_size_unit(&mut self) -> Result<(), IsaError> {
        let mut unit = String::new();
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphabetic() {
                unit.push(ch);
                self.advance_char();
            } else {
                break;
            }
        }

        if unit.is_empty() {
            return Ok(());
        }

        let normalized = unit.to_ascii_lowercase();
        match normalized.as_str() {
            "kb" | "mb" | "gb" | "tb" | "pb" => Ok(()),
            _ => Err(IsaError::Lexer(format!(
                "unknown range size unit '{}': expected kB/MB/GB/TB/PB",
                unit
            ))),
        }
    }

    fn consume_string(&mut self) -> Result<Token, IsaError> {
        let start_line = self.line;
        let start_col = self.column + 1;
        self.advance_char(); // opening quote
        let mut value = String::new();
        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    self.advance_char();
                    return Ok(Token {
                        kind: TokenKind::String,
                        lexeme: value,
                        line: start_line,
                        column: start_col,
                    });
                }
                '\\' => {
                    self.advance_char();
                    if let Some(escaped) = self.peek_char() {
                        let actual = match escaped {
                            'n' => '\n',
                            't' => '\t',
                            '"' => '"',
                            '\\' => '\\',
                            other => other,
                        };
                        value.push(actual);
                        self.advance_char();
                    } else {
                        return Err(IsaError::Lexer("unterminated escape sequence".into()));
                    }
                }
                '\n' => {
                    return Err(IsaError::Lexer("unterminated string literal".into()));
                }
                other => {
                    value.push(other);
                    self.advance_char();
                }
            }
        }
        Err(IsaError::Lexer("unterminated string literal".into()))
    }

    fn consume_line_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            self.advance_char();
            if ch == '\n' {
                break;
            }
        }
    }

    fn consume_single(&mut self, kind: TokenKind) -> Token {
        let start = self.offset;
        let (line, column) = self.position();
        self.advance_char();
        self.make_token_from_span(kind, start, self.offset, line, column)
    }

    fn skip_ignorable(&mut self) {
        loop {
            self.skip_whitespace();
            if let Some('#') = self.peek_char() {
                self.consume_line_comment();
            } else {
                break;
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    fn skip_inline_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            match ch {
                ' ' | '\t' | '\r' => self.advance_char(),
                '\n' => break,
                _ if ch.is_whitespace() && ch != '\n' => self.advance_char(),
                _ => return,
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.offset..].chars().next()
    }

    fn peek_next_char(&self) -> Option<char> {
        let mut iter = self.src[self.offset..].chars();
        iter.next()?;
        iter.next()
    }

    fn advance_char(&mut self) {
        if let Some(ch) = self.peek_char() {
            let len = ch.len_utf8();
            self.offset += len;
            if ch == '\n' {
                self.line += 1;
                self.column = 0;
            } else {
                self.column += 1;
            }
        } else {
            self.offset = self.src.len();
        }
    }

    fn is_eof(&self) -> bool {
        self.offset >= self.src.len()
    }

    fn position(&self) -> (usize, usize) {
        (self.line, self.column + 1)
    }

    fn make_token(&self, kind: TokenKind, lexeme: &str, line: usize, column: usize) -> Token {
        Token {
            kind,
            lexeme: lexeme.to_string(),
            line,
            column,
        }
    }

    fn make_token_from_span(
        &self,
        kind: TokenKind,
        start: usize,
        end: usize,
        line: usize,
        column: usize,
    ) -> Token {
        let slice = &self.src[start..end];
        self.make_token(kind, slice, line, column)
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '$'
}

fn is_ident_part(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit() || ch == '.'
}

#[cfg(test)]
mod tests {
    use super::{Lexer, TokenKind};

    fn kinds(src: &str) -> Vec<TokenKind> {
        let mut lexer = Lexer::new(src);
        let mut kinds = Vec::new();
        loop {
            let token = lexer.next_token().expect("tokenize");
            kinds.push(token.kind.clone());
            if token.kind == TokenKind::EOF {
                break;
            }
        }
        kinds
    }

    #[test]
    fn lexes_basic_directive() {
        let stream = kinds(":space insn addr=32");
        assert_eq!(
            stream,
            vec![
                TokenKind::Colon,
                TokenKind::Identifier,
                TokenKind::Identifier,
                TokenKind::Identifier,
                TokenKind::Equals,
                TokenKind::Number,
                TokenKind::EOF
            ]
        );
    }

    #[test]
    fn lexes_double_tokens() {
        let stream = kinds(":: alias .. target");
        assert_eq!(
            stream,
            vec![
                TokenKind::DoubleColon,
                TokenKind::Identifier,
                TokenKind::DoublePeriod,
                TokenKind::Identifier,
                TokenKind::EOF
            ]
        );
    }

    #[test]
    fn distinguishes_direct_to_from_dash() {
        let stream = kinds("alias -> target - leftover");
        assert_eq!(
            stream,
            vec![
                TokenKind::Identifier,
                TokenKind::DirectTo,
                TokenKind::Identifier,
                TokenKind::Dash,
                TokenKind::Identifier,
                TokenKind::EOF
            ]
        );
    }

    #[test]
    fn lexes_signed_numbers() {
        let stream = kinds("-10 -0x2A 42");
        assert_eq!(
            stream,
            vec![TokenKind::Number, TokenKind::Number, TokenKind::Number, TokenKind::EOF]
        );
    }

    #[test]
    fn lexes_additional_punctuation() {
        let stream = kinds("! % ^ & * ~ ` ' < > ;");
        assert_eq!(
            stream,
            vec![
                TokenKind::Bang,
                TokenKind::Percent,
                TokenKind::Caret,
                TokenKind::Ampersand,
                TokenKind::Asterisk,
                TokenKind::Tilde,
                TokenKind::Backtick,
                TokenKind::Apostrophe,
                TokenKind::LessThan,
                TokenKind::GreaterThan,
                TokenKind::Semicolon,
                TokenKind::EOF
            ]
        );
    }

    #[test]
    fn lexes_bit_expr_as_single_token() {
        let mut lexer = Lexer::new("@(0..5|0b10)");
        let token = lexer.next_token().expect("bit expr");
        assert_eq!(token.kind, TokenKind::BitExpr);
        assert_eq!(token.lexeme, "@(0..5|0b10)");
    }

    #[test]
    fn lexes_range_variants() {
        let mut lexer = Lexer::new("[0x10 + 4kB] [0..31]");
        let first = lexer.next_token().expect("range token");
        assert_eq!(first.kind, TokenKind::Range, "size form");
        let second = lexer.next_token().expect("whitespace");
        assert_eq!(second.kind, TokenKind::Range, "inclusive form");
    }

    #[test]
    fn rejects_unknown_range_unit() {
        let mut lexer = Lexer::new("[0 + 4mib]");
        let err = lexer.next_token().unwrap_err();
        assert!(
            err.to_string().contains("unknown range size unit"),
            "unexpected error: {err:?}"
        );
    }
}