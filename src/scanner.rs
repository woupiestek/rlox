use std::str;

#[repr(u8)] // what was this for again?
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TokenType {
    // Single-character tokens.
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,

    // One or two character tokens.
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Literals.
    Identifier,
    String,
    Number,

    // Keywords.
    And,
    Class,
    Else,
    False,
    Fun,
    For,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Var,
    While,

    // Error
    EndlessString,
    BadTokenStart,

    // Virtual tokens
    End,
}

pub struct Tokens {
    pub types: Vec<TokenType>,
    pub froms: Vec<usize>,
}

pub struct Scanner<'src> {
    source: &'src str,
    current: usize,
    result: Tokens,
}

impl<'src> Scanner<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            current: 0,
            result: Tokens {
                types: Vec::new(),
                froms: Vec::new(),
            },
        }
    }

    pub fn line_and_column(source: &str, offset: usize) -> (u16, u16) {
        let mut line = 1;
        let mut column = 1;
        for char in source[0..offset].chars() {
            if char == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        return (line, column);
    }

    pub fn line_numbers(source: &str, token_offsets: &[usize]) -> Box<[u16]> {
        let mut lines = vec![0; token_offsets.len()].into_boxed_slice();
        let mut line: u16 = 1;
        let mut index = 0;
        for (i, c) in source.char_indices() {
            if c != '\n' {
                continue;
            }
            while token_offsets[index] < i {
                lines[index] = line;
                index += 1;
            }
            line += 1;
        }
        while token_offsets[index] < source.len() {
            lines[index] = line;
            index += 1;
        }
        lines
    }

    fn next_utf8(&self, index: usize) -> usize {
        let mut next = index;
        loop {
            next += 1;
            if next == self.source.len() || (self.get_byte(next) as i8) >= -64 {
                return next;
            }
        }
    }

    pub fn get_str(source: &str, offset: usize) -> Result<&str, String> {
        if source.as_bytes()[offset] != b'\"' {
            let (l, c) = Self::line_and_column(source, offset);
            return err!("No string at ({l},{c})");
        }
        let mut end = offset + 1;
        loop {
            if end >= source.len() {
                let (l, c) = Self::line_and_column(source, offset);
                return err!("Unterminated string at ({l},{c})");
            }
            let byte = source.as_bytes()[end];
            if byte == b'\"' {
                return Ok(&source[offset + 1..end]);
            }
            end += 1;
        }
    }

    pub fn get_identifier_name(source: &str, offset: usize) -> Result<&str, String> {
        let id_start = source.as_bytes()[offset];
        if id_start != b'_' && !id_start.is_ascii_alphabetic() {
            let (l, c) = Self::line_and_column(source, offset);
            return err!("No identifier at ({l},{c})");
        }
        let mut end = offset + 1;
        loop {
            if end >= source.len() {
                return Ok(&source[offset..]);
            }
            let id_part = source.as_bytes()[end];
            if id_part != b'_' && !id_part.is_ascii_alphanumeric() {
                return Ok(&source[offset..end]);
            }
            end += 1;
        }
    }

    pub fn get_number(source: &str, offset: usize) -> Result<f64, String> {
        let mut index = offset;
        while source.as_bytes()[index].is_ascii_digit() {
            index += 1;
        }
        if source.as_bytes()[index] == b'.' {
            index += 1;
            while source.as_bytes()[index].is_ascii_digit() {
                index += 1;
            }
        }
        source[offset..index].parse::<f64>().map_err(|_| {
            let (l, c) = Self::line_and_column(source, offset);
            format!("No number at ({l},{c})")
        })
    }

    fn is_at_end(&self) -> bool {
        self.source.len() <= self.current as usize
    }

    fn get_byte(&self, index: usize) -> u8 {
        self.source.as_bytes()[index]
    }

    fn peek(&self) -> u8 {
        if self.is_at_end() {
            0
        } else {
            self.get_byte(self.current)
        }
    }

    fn peek_ahead(&self) -> u8 {
        if self.current + 1 >= self.source.len() {
            return 0;
        }
        self.get_byte(self.current + 1)
    }

    fn advance(&mut self) -> u8 {
        if self.is_at_end() {
            return 0;
        }
        let ch = self.get_byte(self.current);
        self.current = self.next_utf8(self.current);
        return ch;
    }

    fn match_eq(&mut self) -> bool {
        if self.peek() == b'=' {
            self.current += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        loop {
            let ch = self.peek();
            if ch.is_ascii_whitespace() {
                self.advance();
                continue;
            }

            // skip comments while we are at it
            if ch != b'/' {
                return;
            }
            if self.peek_ahead() != b'/' {
                return;
            }
            self.current += 2;
            loop {
                if self.is_at_end() {
                    return;
                }
                if self.advance() == b'\n' {
                    break;
                }
            }
        }
    }

    fn check_keyword(&self, word: &str, typ: TokenType) -> TokenType {
        let start = self.current as usize - word.len();
        if self.source[start as usize..self.current as usize] == *word {
            return typ;
        }
        TokenType::Identifier
    }

    fn identifier_type(&self, from: usize) -> TokenType {
        let start = self.get_byte(from);
        match start {
            b'a' => self.check_keyword("nd", TokenType::And),
            b'c' => self.check_keyword("lass", TokenType::Class),
            b'e' => self.check_keyword("lse", TokenType::Else),
            b'f' => {
                if self.current > from + 1 {
                    match self.get_byte(from + 1) {
                        b'a' => self.check_keyword("lse", TokenType::False),
                        b'o' => self.check_keyword("r", TokenType::For),
                        b'u' => self.check_keyword("n", TokenType::Fun),
                        _ => TokenType::Identifier,
                    }
                } else {
                    TokenType::Identifier
                }
            }
            b'i' => self.check_keyword("f", TokenType::If),
            b'n' => self.check_keyword("il", TokenType::Nil),
            b'o' => self.check_keyword("r", TokenType::Or),
            b'p' => self.check_keyword("rint", TokenType::Print),
            b'r' => self.check_keyword("eturn", TokenType::Return),
            b's' => self.check_keyword("uper", TokenType::Super),
            b't' => {
                if self.current > from + 1 {
                    match self.get_byte(from + 1) {
                        b'h' => self.check_keyword("is", TokenType::This),
                        b'r' => self.check_keyword("ue", TokenType::True),
                        _ => TokenType::Identifier,
                    }
                } else {
                    TokenType::Identifier
                }
            }
            b'v' => self.check_keyword("ar", TokenType::Var),
            b'w' => self.check_keyword("hile", TokenType::While),
            _ => TokenType::Identifier,
        }
    }

    fn token(&mut self, typ: TokenType) {
        self.result.types.push(typ)
    }

    fn identifier(&mut self) {
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.advance();
        }
        self.result
            .types
            .push(self.identifier_type(*self.result.froms.last().unwrap()))
    }

    fn number(&mut self) {
        while self.peek().is_ascii_digit() {
            self.advance();
        }
        if self.peek() == b'.' && self.peek_ahead().is_ascii_digit() {
            self.advance();
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }
        self.token(TokenType::Number)
    }

    fn string(&mut self) {
        loop {
            if self.is_at_end() {
                return self.token(TokenType::EndlessString);
            }
            if self.advance() == b'"' {
                return self.token(TokenType::String);
            }
        }
    }

    pub fn scan<'s>(source: &'s str) -> Tokens {
        let mut scanner = Scanner::<'s>::new(source);
        scanner.run();
        return scanner.result;
    }

    fn run(&mut self) {
        loop {
            self.skip_whitespace();
            self.result.froms.push(self.current);
            if self.is_at_end() {
                self.token(TokenType::End);
                return;
            }
            let ch = self.advance();
            if ch.is_ascii_digit() {
                self.number();
                continue;
            }
            if ch.is_ascii_alphabetic() || ch == b'_' {
                self.identifier();
                continue;
            }
            match ch {
                b'(' => self.token(TokenType::LeftParen),
                b')' => self.token(TokenType::RightParen),
                b'{' => self.token(TokenType::LeftBrace),
                b'}' => self.token(TokenType::RightBrace),
                b';' => self.token(TokenType::Semicolon),
                b',' => self.token(TokenType::Comma),
                b'.' => self.token(TokenType::Dot),
                b'-' => self.token(TokenType::Minus),
                b'+' => self.token(TokenType::Plus),
                b'/' => self.token(TokenType::Slash),
                b'*' => self.token(TokenType::Star),
                b'!' => {
                    if self.match_eq() {
                        self.token(TokenType::BangEqual)
                    } else {
                        self.token(TokenType::Bang)
                    }
                }
                b'=' => {
                    if self.match_eq() {
                        self.token(TokenType::EqualEqual)
                    } else {
                        self.token(TokenType::Equal)
                    }
                }
                b'<' => {
                    if self.match_eq() {
                        self.token(TokenType::LessEqual)
                    } else {
                        self.token(TokenType::Less)
                    }
                }
                b'>' => {
                    if self.match_eq() {
                        self.token(TokenType::GreaterEqual)
                    } else {
                        self.token(TokenType::Greater)
                    }
                }
                b'"' => self.string(),
                _ => self.token(TokenType::BadTokenStart),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcs(scanner: Tokens, source: &str) -> Vec<(u16, u16)> {
        scanner
            .froms
            .iter()
            .map(|it| Scanner::line_and_column(source, *it))
            .collect::<Vec<_>>()
    }

    #[test]
    fn print_string() {
        let source = "print \"one 😲\";";
        let scanner = Scanner::scan(source);
        assert_eq!(
            scanner.types,
            vec![
                TokenType::Print,
                TokenType::String,
                TokenType::Semicolon,
                TokenType::End
            ]
        );

        assert_eq!(lcs(scanner, source), vec![(1, 1), (1, 7), (1, 14), (1, 15)]);
    }

    #[test]
    fn var_a_is_true() {
        let source = "var a = true;";
        let scanner = Scanner::scan(source);
        assert_eq!(
            scanner.types,
            vec![
                TokenType::Var,
                TokenType::Identifier,
                TokenType::Equal,
                TokenType::True,
                TokenType::Semicolon,
                TokenType::End
            ]
        );
        assert_eq!(Scanner::get_identifier_name(source, 0).unwrap(), "var");
        assert_eq!(Scanner::get_identifier_name(source, 4).unwrap(), "a");
        assert_eq!(Scanner::get_identifier_name(source, 8).unwrap(), "true");
    }

    #[test]
    fn block_one_plus_two() {
        let source = "{ 
            // let's make this more interesting 😉
            1 + 2; }";
        let tokens = Scanner::scan(source);
        assert_eq!(
            tokens.types,
            vec![
                TokenType::LeftBrace,
                TokenType::Number,
                TokenType::Plus,
                TokenType::Number,
                TokenType::Semicolon,
                TokenType::RightBrace,
                TokenType::End
            ]
        );
        assert_eq!(Scanner::get_number(source, 68).unwrap(), 1.);
        assert_eq!(Scanner::get_number(source, 72).unwrap(), 2.);
    }
}
