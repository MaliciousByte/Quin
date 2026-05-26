use crate::frontend::token::Token;

#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Variable(Token), // name
    Assign { name: Token, value: Box<Expr> },
    Binary { left: Box<Expr>, operator: Token, right: Box<Expr> },
    Unary { operator: Token, right: Box<Expr> },
    Logical { left: Box<Expr>, operator: Token, right: Box<Expr> },
    Call { callee: Box<Expr>, paren: Token, arguments: Vec<Expr> },
    Get { object: Box<Expr>, name: Token },
    Set { object: Box<Expr>, name: Token, value: Box<Expr> },
    Array { elements: Vec<Expr> },
    Dict { entries: Vec<(Expr, Expr)> },
    Tuple { elements: Vec<Expr> },
    SetInit { elements: Vec<Expr> },
    Index { object: Box<Expr>, bracket: Token, index: Box<Expr> },
    IndexSet { object: Box<Expr>, bracket: Token, index: Box<Expr>, value: Box<Expr> },
    StructInit { name: Token, fields: Vec<(Token, Expr)> },
    Lambda { 
        params: Vec<(Token, Option<Type>, Option<Expr>)>, 
        body: Vec<Stmt>,
        is_async: bool,
    },
    OptionalGet { object: Box<Expr>, name: Token },
    Ternary { condition: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr> },
    Spread(Box<Expr>),
    Pipe { left: Box<Expr>, right: Box<Expr> }, // value |> function
}

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expression(Expr),
    Emit(Expr),
    Let { 
        pattern: Pattern, 
        is_mut: bool, 
        type_annotation: Option<Type>, 
        initializer: Option<Expr> 
    },
    Block(Vec<Stmt>),
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        elif_branches: Vec<(Expr, Stmt)>,
        else_branch: Option<Box<Stmt>>,
    },
    While { condition: Expr, body: Box<Stmt> },
    For { item: Token, iterable: Expr, body: Box<Stmt> },
    Function {
        name: Token,
        params: Vec<(Token, Option<Type>, Option<Expr>)>, // name, type, and default value
        variadic: bool,
        is_async: bool,
        is_static: bool,
        is_abstract: bool,
        visibility: Visibility,
        return_type: Option<Type>,
        body: Vec<Stmt>,
    },
    Return { keyword: Token, value: Option<Expr> },
    Struct {
        name: Token,
        fields: Vec<(Token, Option<Type>)>,
    },
    Enum {
        name: Token,
        variants: Vec<Token>,
    },
    TypeAlias {
        name: Token,
        target: Type,
    },
    Class {
        name: Token,
        superclass: Option<Token>,
        is_abstract: bool,
        interfaces: Vec<Token>,
        methods: Vec<Stmt>, // Only Function stmts expected
    },
    Interface {
        name: Token,
        methods: Vec<Stmt>,
    },
    TryCatch {
        try_body: Box<Stmt>,
        catch_param: Token,
        catch_body: Box<Stmt>,
    },
    Throw(Expr),
    Export(Box<Stmt>),
    Match {
        expression: Expr,
        arms: Vec<MatchArm>,
    },
    Import {
        module: Token,
        items: Vec<Token>, // empty means `import math`
    },
}

#[derive(Debug, Clone)]
pub enum MatchArm {
    Range(i64, i64, Stmt), // min, max, body
    Value(Expr, Stmt),
    Default(Stmt),
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Identifier(Token),
    Object(Vec<Token>), // { name, age }
    Array(Vec<Token>),  // [ x, y ]
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Simple(String),
    Array(Box<Type>),
    Dict(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),
    Set(Box<Type>),
    Union(Vec<Type>),
    Any,
}
