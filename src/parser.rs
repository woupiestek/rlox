use std::iter::Peekable;

use crate::scanner::{Scanner, TokenType};

pub struct Parser<'a> {
    source: &'a str,
    tokens: Peekable<Scanner<'a>>,
    panic_mode: bool,
    had_error: bool,
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

impl Parser<'_> {
    pub fn new(source: &str) -> Parser {
        let tokens = Scanner::new(source).peekable();
        Parser {
            source,
            tokens,
            panic_mode: false,
            had_error: false,
        }
    }

    fn check(&mut self, token_type: TokenType) -> bool {
        if let Some(token) = self.tokens.peek() {
            token.token_type == token_type
        } else {
            false
        }
    }

    fn match_type(&mut self, token_type: TokenType) -> bool {
        if self.check(token_type) {
            self.tokens.next();
            true
        } else {
            false
        }
    }
}
