use std::str;

#[derive(Debug, PartialEq)]
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

    Error,
    Eof,
}

#[derive(Debug, PartialEq)]
pub struct Token<'a> {
    pub token_type: TokenType,
    pub lexeme: &'a str,
    pub line: u16,
    pub column: u16,
}
pub struct Scanner<'a> {
    source: &'a [u8],
    current: usize,
    line: u16,
    column: u16,
    token_start: usize,
    token_line: u16,
    token_column: u16,
}

impl Scanner<'_> {
    pub fn new(source: &str) -> Scanner {
        Scanner {
            source: source.as_bytes(),
            current: 0,
            line: 1,
            column: 1,
            token_start: 0,
            token_line: 0,
            token_column: 0,
        }
    }

    fn is_at_end(&self) -> bool {
        self.source.len() <= self.current
    }

    fn peek(&self) -> u8 {
        if self.is_at_end() {
            0
        } else {
            self.source[self.current]
        }
    }

    fn peek_ahead(&self) -> u8 {
        if self.current + 1 >= self.source.len() {
            return 0;
        }
        self.source[self.current + 1]
    }

    fn advance(&mut self) -> u8 {
        if self.is_at_end() {
            return 0;
        }
        let ch = self.source[self.current];
        if ch == b'\n' {
            self.line += 1;
            self.column = 1;
        } else if ch != b'\r' {
            self.column += 1;
        }
        // for unicode
        loop {
            self.current += 1;
            if self.is_at_end() || self.source[self.current] as i8 >= -64 {
                return ch;
            }
        }
    }

    fn match_eq(&mut self) -> bool {
        if self.peek() == b'=' {
            self.current += 1;
            true
        } else {
            false
        }
    }

    fn token(&self, typ: TokenType) -> Token {
        Token {
            token_type: typ,
            lexeme: &str::from_utf8(&self.source[self.token_start..self.current]).unwrap(),
            line: self.token_line,
            column: self.token_column,
        }
    }

    fn error<'c>(&self, message: &'c str) -> Token<'c> {
        Token {
            token_type: TokenType::Error,
            lexeme: message,
            line: self.token_line,
            column: self.token_column,
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
        let start = self.current - word.len();
        if self.source[start..self.current] == *word.as_bytes() {
            return typ;
        }
        TokenType::Identifier
    }

    fn identifier_type(&self) -> TokenType {
        let start = self.source[self.token_start];
        println!("{}", start as char);
        match start {
            b'a' => self.check_keyword("nd", TokenType::And),
            b'c' => self.check_keyword("lass", TokenType::Class),
            b'e' => self.check_keyword("lse", TokenType::Else),
            b'f' => {
                if self.current > self.token_start + 1 {
                    match self.source[self.token_start + 1] {
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
                if self.current > self.token_start + 1 {
                    match self.source[self.token_start + 1] {
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

    fn identifier(&mut self) -> Token {
        while self.peek().is_ascii_alphanumeric() {
            self.advance();
        }
        self.token(self.identifier_type())
    }

    fn number(&mut self) -> Token {
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

    fn string(&mut self) -> Token {
        loop {
            if self.is_at_end() {
                return self.error("unterminated string");
            }
            if self.advance() == b'"' {
                return self.token(TokenType::String);
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();
        self.token_start = self.current;
        self.token_line = self.line;
        self.token_column = self.column;
        if self.is_at_end() {
            return self.token(TokenType::Eof);
        }
        let ch = self.advance();
        if ch.is_ascii_digit() {
            return self.number();
        }
        if ch.is_ascii_alphabetic() {
            return self.identifier();
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
            _ => self.error("unexpected character"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_string() {
        let mut scanner = Scanner::new("print \"one 😲\";");
        let print = scanner.next_token();
        assert_eq!(
            print,
            Token {
                token_type: TokenType::Print,
                lexeme: "print",
                line: 1,
                column: 1
            }
        );
        let unicode = scanner.next_token();
        assert_eq!(
            unicode,
            Token {
                token_type: TokenType::String,
                lexeme: "\"one 😲\"",
                line: 1,
                column: 7
            }
        );
        let semi = scanner.next_token();
        assert_eq!(
            semi,
            Token {
                token_type: TokenType::Semicolon,
                lexeme: ";",
                line: 1,
                column: 14
            }
        );
    }

    #[test]
    fn var_a_is_true() {
        let mut scanner = Scanner::new("var a = true;");
        let var = scanner.next_token();
        assert_eq!(
            var,
            Token {
                token_type: TokenType::Var,
                lexeme: "var",
                line: 1,
                column: 1
            }
        );
        let a = scanner.next_token();
        assert_eq!(
            a,
            Token {
                token_type: TokenType::Identifier,
                lexeme: "a",
                line: 1,
                column: 5
            }
        );
        let is = scanner.next_token();
        assert_eq!(
            is,
            Token {
                token_type: TokenType::Equal,
                lexeme: "=",
                line: 1,
                column: 7
            }
        );
        let _true = scanner.next_token();
        assert_eq!(
            _true,
            Token {
                token_type: TokenType::True,
                lexeme: "true",
                line: 1,
                column: 9
            }
        );
    }

    #[test]
    fn block_one_plus_two() {
        let mut scanner = Scanner::new(
            "{ 
            // let's make this more interesting 😉
            1 + 2; }",
        );
        let left_brace = scanner.next_token();
        assert_eq!(
            left_brace,
            Token {
                token_type: TokenType::LeftBrace,
                lexeme: "{",
                line: 1,
                column: 1
            }
        );
        let one = scanner.next_token();
        assert_eq!(
            one,
            Token {
                token_type: TokenType::Number,
                lexeme: "1",
                line: 3,
                column: 13
            }
        );
        let plus = scanner.next_token();
        assert_eq!(
            plus,
            Token {
                token_type: TokenType::Plus,
                lexeme: "+",
                line: 3,
                column: 15
            }
        );
        let two = scanner.next_token();
        assert_eq!(
            two,
            Token {
                token_type: TokenType::Number,
                lexeme: "2",
                line: 3,
                column: 17
            }
        );
        let semi = scanner.next_token();
        assert_eq!(
            semi,
            Token {
                token_type: TokenType::Semicolon,
                lexeme: ";",
                line: 3,
                column: 18
            }
        );
        let right_brace = scanner.next_token();
        assert_eq!(
            right_brace,
            Token {
                token_type: TokenType::RightBrace,
                lexeme: "}",
                line: 3,
                column: 20
            }
        );
    }
}
