use std::mem;

use crate::scanner::{Scanner, Token, TokenType};

pub struct ParseFail {
    location: Option<Token>,
    message: String,
}
pub struct Parser<'a> {
    source: &'a str,
    scanner: Scanner<'a>,
    current: Option<Token>,
    previous: Option<Token>,
    panic_mode: bool,
    errors: Vec<ParseFail>,
}

impl Parser<'_> {
    pub fn new(source: &str) -> Parser {
        let mut scanner = Scanner::new(source);
        let current = scanner.next();
        Parser {
            source,
            scanner,
            current,
            previous: None,
            panic_mode: false,
            errors: Vec::new(),
        }
    }

    fn fail(&mut self, location: Option<Token>, msg: &str) {
        self.errors.push(ParseFail {
            location,
            message: msg.to_string(),
        });
        self.panic_mode = true;
    }

    fn advance(&mut self) {
        loop {
            if let Some(token) = self.scanner.next() {
                match token.token_type {
                    TokenType::ErrorNoStringEnd => {
                        self.fail(Some(token), "missing string ending");
                    }
                    TokenType::ErrorOddChar => {
                        self.fail(Some(token), "unexpected character");
                    }
                    _ => {
                        self.previous = self.current.replace(token);
                        return;
                    }
                }
            } else {
                // None means end of input
                self.current = None;
                return;
            }
        }
    }

    fn check(&mut self, token_type: TokenType) -> bool {
        if let Some(token) = &self.current {
            token.token_type == token_type
        } else {
            false
        }
    }

    fn match_type(&mut self, token_type: TokenType) -> bool {
        if self.check(token_type) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, token_type: TokenType, msg: &str) {
        if self.check(token_type) {
            self.advance();
        } else {
            // maybe the clone isn't needed?
            // let's first see if the error handling won't be uprooted.
            self.fail(self.current.clone(), msg);
        }
    }

    fn synchronize(&mut self) {
        self.panic_mode = false;
        loop {
            if let Some(token) = &self.current {
                if let Some(previous) = &self.previous {
                    if let TokenType::Semicolon = previous.token_type {
                        return;
                    }
                }
                match token.token_type {
                    TokenType::Class
                    | TokenType::Fun
                    | TokenType::Var
                    | TokenType::For
                    | TokenType::If
                    | TokenType::While
                    | TokenType::Print
                    | TokenType::Return => {
                        return;
                    }
                    _ => {
                        self.advance();
                        continue;
                    }
                }
            }
            return;
        }
    }
}

pub enum Precedence {
    None,
    Assignment, // =
    Or,         // or
    And,        // and
    Equality,   // == !=
    Comparison, // < > <= >=
    Term,       // + -
    Factor,     // * /
    Unary,      // ! -
    Call,       // . ()
    Primary,
}

trait ParseRule {
    fn prefix(can_assign: bool);
    fn infix(can_assign: bool);
}
