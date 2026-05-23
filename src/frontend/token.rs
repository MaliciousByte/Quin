#[derive(Debug, PartialEq, Clone)]
pub enum TokenType {
    // Single-character tokens.
    LeftParen, RightParen, LeftBrace, RightBrace, LeftBracket, RightBracket,
    Comma, Dot, Minus, Plus, Semicolon, Slash, Star, Colon, Question,

    // One or two character tokens.
    Bang, BangEqual,
    Equal, EqualEqual,
    Greater, GreaterEqual,
    Less, LessEqual,
    MinusEqual, PlusEqual,

    // Two-character specifics for Quin
    Arrow,    // ->
    FatArrow, // =>
    Nullish,  // ??
    DotDot,   // ..
    DotDotDot, // ...
    Pipe,     // |
    PipeGreater, // |>
    QuestionDot, // ?.

    // Literals.
    Identifier,
    StringLit,
    IntLit,
    FloatLit,

    // Keywords.
    Let, Mut, Const, Task, Return, If, Elif, Else, For, In, While,
    Struct, Class, Extends, Match, Attempt, Rescue, Use, From,
    Self_, Void, True, False,
    Enum, Type, Any, Dict, Set, Tuple,
    Raise, Export, Async,
    Pub, Priv, Trait, Base, Shared, With,

    // Types (often parsed from identifiers, but good to have if we syntax highlight them as keywords)
    TypeInt, TypeFloat, TypeStr, TypeBool,

    Error,
    Eof,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub ty: TokenType,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

impl Token {
    pub fn new(ty: TokenType, lexeme: String, line: usize, column: usize) -> Self {
        Token { ty, lexeme, line, column }
    }
}
