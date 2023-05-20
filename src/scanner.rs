use std::str;

#[derive(Clone, Debug, PartialEq)]
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

    ErrorOddChar,
    ErrorNoStringEnd,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub token_type: TokenType,
    from: usize,
    to: usize,
    pub line: u16,
    pub column: u16,
}

impl Token {
    pub fn lexeme<'a>(&self, source: &'a str) -> &'a str {
        &source[self.from..self.to]
    }
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
            token_line: 1,
            token_column: 1,
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
            from: self.token_start,
            to: self.current,
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
                return self.token(TokenType::ErrorNoStringEnd);
            }
            if self.advance() == b'"' {
                return self.token(TokenType::String);
            }
        }
    }
}

impl Iterator for Scanner<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_whitespace();
        self.token_start = self.current;
        self.token_line = self.line;
        self.token_column = self.column;
        if self.is_at_end() {
            return None;
        }
        let ch = self.advance();
        if ch.is_ascii_digit() {
            return Some(self.number());
        }
        if ch.is_ascii_alphabetic() {
            return Some(self.identifier());
        }
        let token = match ch {
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
            _ => self.token(TokenType::ErrorOddChar),
        };
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_string() {
        let mut scanner = Scanner::new("print \"one 😲\";");
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::Print,
                from: 0,
                to: 5,
                line: 1,
                column: 1
            })
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::String,
                from: 6,
                to: 16,
                //lexeme: "\"one 😲\"",
                line: 1,
                column: 7
            })
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::Semicolon,
                from: 16,
                to: 17,
                line: 1,
                column: 14
            })
        );
        assert_eq!(scanner.next(), None);
    }

    #[test]
    fn var_a_is_true() {
        let mut scanner = Scanner::new("var a = true;");
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::Var,
                from: 0,
                to: 3,
                line: 1,
                column: 1
            })
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::Identifier,
                from: 4,
                to: 5,
                line: 1,
                column: 5
            })
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::Equal,
                from: 6,
                to: 7,
                line: 1,
                column: 7
            })
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::True,
                from: 8,
                to: 12,
                line: 1,
                column: 9
            })
        );
    }

    #[test]
    fn block_one_plus_two() {
        let mut scanner = Scanner::new(
            "{ 
            // let's make this more interesting 😉
            1 + 2; }",
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::LeftBrace,
                from: 0,
                to: 1,
                line: 1,
                column: 1
            })
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::Number,
                from: 68,
                to: 69,
                line: 3,
                column: 13
            })
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::Plus,
                from: 70,
                to: 71,
                line: 3,
                column: 15
            })
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::Number,
                from: 72,
                to: 73,
                line: 3,
                column: 17
            })
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::Semicolon,
                from: 73,
                to: 74,
                line: 3,
                column: 18
            })
        );
        assert_eq!(
            scanner.next(),
            Some(Token {
                token_type: TokenType::RightBrace,
                from: 75,
                to: 76,
                line: 3,
                column: 20
            })
        );
    }
}
