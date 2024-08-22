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
    Begin,
    End,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Token(pub TokenType, pub usize);

pub struct Scanner<'src> {
    source: &'src str,
    current: usize,
    token_start: usize,
}

impl<'src> Scanner<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            current: 0,
            token_start: 0,
        }
    }

    pub fn line_and_column(&self, offset: usize) -> (u16, u16) {
        assert!((offset as usize) <= self.source.len());
        let mut line = 1;
        let mut column = 1;
        let mut index = 0;
        loop {
            if index >= offset {
                return (line, column);
            }
            let byte = self.get_byte(index);
            if byte == b'\n' {
                line += 1;
                column = 1;
            } else if byte != b'\r' {
                column += 1;
            }
            index = self.next_utf8(index);
        }
    }

    fn next_utf8(&self, index: usize) -> usize {
        let mut next = index;
        loop {
            next += 1;
            if next == self.source.len() || (self.get_byte(next) as i8) >= -64 { return next; }
        }
    }

    pub fn get_str(&self, offset: usize) -> Result<&str, String> {
        if self.get_byte(offset) != b'\"' {
            let (l, c) = self.line_and_column(offset);
            return err!("No string at ({l},{c})");
        }
        let mut end = offset + 1;
        loop {
            if end >= self.source.len() {
                let (l, c) = self.line_and_column(offset);
                return err!("Unterminated string at ({l},{c})");
            }
            let byte = self.get_byte(end);
            if byte == b'\"' {
                return Ok(&self.source[offset + 1..end]);
            }
            end = self.next_utf8(end);
        }
    }

    pub fn get_identifier_name(&self, offset: usize) -> Result<&str, String> {
        let id_start = self.get_byte(offset);
        if id_start != b'_' && !id_start.is_ascii_alphabetic() {
            let (l, c) = self.line_and_column(offset);
            return err!("No identifier at ({l},{c})");
        }
        let mut end = offset + 1;
        loop {
            if end >= self.source.len() {
                return Ok(&self.source[offset..]);
            }
            let id_part = self.get_byte(end);
            if id_part != b'_' && !id_part.is_ascii_alphanumeric() {
                return Ok(&self.source[offset..end]);
            }
            end += 1;
        }
    }

    pub fn get_number(&self, offset: usize) -> Result<f64, String> {
        let mut index = offset;
        while self.get_byte(index).is_ascii_digit() {
            index += 1;
        }
        if self.get_byte(index) == b'.' {
            index += 1;
            while self.get_byte(index).is_ascii_digit() {
                index += 1;
            }
        }
        self.source[offset..index].parse::<f64>().map_err(|_| {
            let (l, c) = self.line_and_column(offset);
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

    fn identifier_type(&self) -> TokenType {
        let start = self.get_byte(self.token_start);
        match start {
            b'a' => self.check_keyword("nd", TokenType::And),
            b'c' => self.check_keyword("lass", TokenType::Class),
            b'e' => self.check_keyword("lse", TokenType::Else),
            b'f' => {
                if self.current > self.token_start + 1 {
                    match self.get_byte(self.token_start + 1) {
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
                    match self.get_byte(self.token_start + 1) {
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
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.advance();
        }

        Token(self.identifier_type(), self.token_start)
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
        {
            let this = &self;
            let typ = TokenType::Number;
            Token(typ, this.token_start)
        }
    }

    fn string(&mut self) -> Token {
        loop {
            if self.is_at_end() {
                return {
                    let this = &self;
                    let typ = TokenType::EndlessString;
                    Token(typ, this.token_start)
                };
            }
            if self.advance() == b'"' {
                return {
                    let this = &self;
                    let typ = TokenType::String;
                    Token(typ, this.token_start)
                };
            }
        }
    }

    pub fn next(&mut self) -> Token {
        self.skip_whitespace();
        self.token_start = self.current;
        if self.is_at_end() {
            return {
                let this = &self;
                let typ = TokenType::End;
                Token(typ, this.token_start)
            };
        }
        let ch = self.advance();
        if ch.is_ascii_digit() {
            return self.number();
        }
        if ch.is_ascii_alphabetic() || ch == b'_' {
            return self.identifier();
        }
        match ch {
            b'(' => {
                let this = &self;
                let typ = TokenType::LeftParen;
                Token(typ, this.token_start)
            },
            b')' => {
                let this = &self;
                let typ = TokenType::RightParen;
                Token(typ, this.token_start)
            },
            b'{' => {
                let this = &self;
                let typ = TokenType::LeftBrace;
                Token(typ, this.token_start)
            },
            b'}' => {
                let this = &self;
                let typ = TokenType::RightBrace;
                Token(typ, this.token_start)
            },
            b';' => {
                let this = &self;
                let typ = TokenType::Semicolon;
                Token(typ, this.token_start)
            },
            b',' => {
                let this = &self;
                let typ = TokenType::Comma;
                Token(typ, this.token_start)
            },
            b'.' => {
                let this = &self;
                let typ = TokenType::Dot;
                Token(typ, this.token_start)
            },
            b'-' => {
                let this = &self;
                let typ = TokenType::Minus;
                Token(typ, this.token_start)
            },
            b'+' => {
                let this = &self;
                let typ = TokenType::Plus;
                Token(typ, this.token_start)
            },
            b'/' => {
                let this = &self;
                let typ = TokenType::Slash;
                Token(typ, this.token_start)
            },
            b'*' => {
                let this = &self;
                let typ = TokenType::Star;
                Token(typ, this.token_start)
            },
            b'!' => {
                if self.match_eq() {
                    {
                        let this = &self;
                        let typ = TokenType::BangEqual;
                        Token(typ, this.token_start)
                    }
                } else {
                    {
                        let this = &self;
                        let typ = TokenType::Bang;
                        Token(typ, this.token_start)
                    }
                }
            }
            b'=' => {
                if self.match_eq() {
                    {
                        let this = &self;
                        let typ = TokenType::EqualEqual;
                        Token(typ, this.token_start)
                    }
                } else {
                    {
                        let this = &self;
                        let typ = TokenType::Equal;
                        Token(typ, this.token_start)
                    }
                }
            }
            b'<' => {
                if self.match_eq() {
                    {
                        let this = &self;
                        let typ = TokenType::LessEqual;
                        Token(typ, this.token_start)
                    }
                } else {
                    {
                        let this = &self;
                        let typ = TokenType::Less;
                        Token(typ, this.token_start)
                    }
                }
            }
            b'>' => {
                if self.match_eq() {
                    {
                        let this = &self;
                        let typ = TokenType::GreaterEqual;
                        Token(typ, this.token_start)
                    }
                } else {
                    {
                        let this = &self;
                        let typ = TokenType::Greater;
                        Token(typ, this.token_start)
                    }
                }
            }
            b'"' => self.string(),
            _ => {
                let this = &self;
                let typ = TokenType::BadTokenStart;
                Token(typ, this.token_start)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_string() {
        let mut scanner = Scanner::new("print \"one 😲\";");
        assert_eq!(scanner.next(), Token(TokenType::Print, 0));
        assert_eq!(scanner.line_and_column(0), (1, 1));
        assert_eq!(scanner.next(), Token(TokenType::String, 6));
        assert_eq!(scanner.get_str(6).unwrap(), "one 😲");
        assert_eq!(scanner.line_and_column(6), (1, 7));
        // some differences expected because of the smiley
        assert_eq!(scanner.next(), Token(TokenType::Semicolon, 16));
        assert_eq!(scanner.line_and_column(16), (1, 14));
        assert_eq!(scanner.next(), Token(TokenType::End, 17));
        assert_eq!(scanner.line_and_column(17), (1, 15));
    }

    #[test]
    fn var_a_is_true() {
        let mut scanner = Scanner::new("var a = true;");
        assert_eq!(scanner.next(), Token(TokenType::Var, 0));
        assert_eq!(scanner.get_identifier_name(0).unwrap(), "var");
        assert_eq!(scanner.next(), Token(TokenType::Identifier, 4));
        assert_eq!(scanner.get_identifier_name(4).unwrap(), "a");
        assert_eq!(scanner.next(), Token(TokenType::Equal, 6));
        assert_eq!(scanner.next(), Token(TokenType::True, 8));
        assert_eq!(scanner.get_identifier_name(8).unwrap(), "true");
    }

    #[test]
    fn block_one_plus_two() {
        let mut scanner = Scanner::new(
            "{ 
            // let's make this more interesting 😉
            1 + 2; }",
        );
        assert_eq!(scanner.next(), Token(TokenType::LeftBrace, 0));
        assert_eq!(scanner.next(), Token(TokenType::Number, 68));
        assert_eq!(scanner.get_number(68).unwrap(), 1.);
        assert_eq!(scanner.next(), Token(TokenType::Plus, 70));
        assert_eq!(scanner.next(), Token(TokenType::Number, 72));
        assert_eq!(scanner.get_number(72).unwrap(), 2.);
        assert_eq!(scanner.next(), Token(TokenType::Semicolon, 73));
        assert_eq!(scanner.next(), Token(TokenType::RightBrace, 75));
    }
}
