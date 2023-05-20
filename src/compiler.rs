use crate::{
    memory::Heap,
    object::{Class, Method},
    scanner::{Scanner, Token, TokenType},
    stack::Stack,
};

pub struct ParseFail(Token, String);
type ParseResult<T> = Result<T, ParseFail>;
struct Local {
    name: Token,
    depth: i32,
    is_captured: bool,
}
struct Upvalue {
    index: u8,
    is_local: bool,
}
enum FunctionType {
    Function,
    Initializer,
    Method,
    Script,
}

struct MethodCompiler {
    method: Method,
    function_type: FunctionType,
    locals: Stack<Local>,
    upvalues: Stack<Upvalue>,
}

struct ClassCompiler {
    class: Class,
    has_super: bool,
}

pub struct Parser<'a> {
    source: &'a str,
    scanner: Scanner<'a>,
    current: Token,
    next: Token,
    methods: Vec<MethodCompiler>,
    classes: Vec<ClassCompiler>,
    heap: Heap,
}

impl Parser<'_> {
    pub fn new(source: &str, heap: Heap) -> Parser {
        let mut scanner = Scanner::new(source);
        let current = scanner.next();
        let next = scanner.next();
        Parser {
            source,
            scanner,
            current,
            next,
            methods: Vec::new(),
            classes: Vec::new(),
            heap,
        }
    }

    fn fail(&self, msg: &str) -> ParseResult<()> {
        Err(ParseFail(self.current, msg.to_string()))
    }

    // forgot about the loop here...
    fn advance(&mut self) -> Result<(), ParseFail> {
        if self.check(TokenType::End) {
            return Ok(());
        }
        let token = self.scanner.next();
        match token.token_type {
            TokenType::ErrorNoStringEnd => self.fail("missing string ending"),
            TokenType::ErrorOddChar => self.fail("unexpected character"),
            _ => {
                self.current = self.next;
                self.next = token;
                Ok(())
            }
        }
    }

    fn check(&mut self, token_type: TokenType) -> bool {
        self.current.token_type == token_type
    }

    fn match_type(&mut self, token_type: TokenType) -> Result<bool, ParseFail> {
        if self.check(token_type) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn consume(&mut self, token_type: TokenType, msg: &str) -> Result<(), ParseFail> {
        if self.check(token_type) {
            self.advance()
        } else {
            self.fail(msg)
        }
    }

    fn synchronize(&mut self) -> Result<(), ParseFail> {
        loop {
            match self.current.token_type {
                TokenType::Class
                | TokenType::End
                | TokenType::Fun
                | TokenType::Var
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => {
                    return Ok(());
                }
                TokenType::Semicolon => {
                    self.advance()?;
                    return Ok(());
                }
                _ => {
                    self.advance()?;
                    continue;
                }
            }
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

type ParseFn = fn(can_assign: bool);
struct ParseRule {
    prefix: Option<ParseFn>,
    infix: Option<ParseFn>,
    precedence: Precedence,
}

const DEFAULT_RULE: ParseRule = ParseRule {
    prefix: None,
    infix: None,
    precedence: Precedence::None,
};
const RULES: [ParseRule; 41] = {
    let mut rules = [DEFAULT_RULE; 41];
    rules[TokenType::And as usize] = DEFAULT_RULE;
    rules
};
