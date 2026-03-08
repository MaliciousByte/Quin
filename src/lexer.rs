use crate::token::{Token, TokenType};

pub struct Lexer {
    source: Vec<char>,
    start: usize,
    current: usize,
    line: usize,
    column: usize,
    start_column: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            source: source.chars().collect(),
            start: 0,
            current: 0,
            line: 1,
            column: 1,
            start_column: 1,
        }
    }

    pub fn scan_tokens(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        while !self.is_at_end() {
            self.start = self.current;
            self.start_column = self.column;
            
            if let Some(token) = self.scan_token()? {
                tokens.push(token);
            }
        }

        tokens.push(Token::new(TokenType::Eof, "".to_string(), self.line, self.column));
        Ok(tokens)
    }

    fn scan_token(&mut self) -> Result<Option<Token>, String> {
        let c = self.advance();
        match c {
            '(' => Ok(Some(self.make_token(TokenType::LeftParen))),
            ')' => Ok(Some(self.make_token(TokenType::RightParen))),
            '{' => Ok(Some(self.make_token(TokenType::LeftBrace))),
            '}' => Ok(Some(self.make_token(TokenType::RightBrace))),
            '[' => Ok(Some(self.make_token(TokenType::LeftBracket))),
            ']' => Ok(Some(self.make_token(TokenType::RightBracket))),
            ',' => Ok(Some(self.make_token(TokenType::Comma))),
            ':' => Ok(Some(self.make_token(TokenType::Colon))),
            ';' => Ok(Some(self.make_token(TokenType::Semicolon))),
            '*' => Ok(Some(self.make_token(TokenType::Star))),
            '|' => {
                let ty = if self.match_char('>') { TokenType::PipeGreater } else { TokenType::Pipe };
                Ok(Some(self.make_token(ty)))
            }
            '?' => {
                let ty = if self.match_char('?') { 
                    TokenType::Nullish 
                } else if self.match_char('.') {
                    TokenType::QuestionDot
                } else { 
                    TokenType::Question 
                };
                Ok(Some(self.make_token(ty)))
            }
            '.' => {
                let ty = if self.match_char('.') { 
                    if self.match_char('.') {
                        TokenType::DotDotDot
                    } else {
                        TokenType::DotDot 
                    }
                } else { 
                    TokenType::Dot 
                };
                Ok(Some(self.make_token(ty)))
            }
            '-' => {
                let ty = if self.match_char('>') {
                    TokenType::Arrow
                } else if self.match_char('=') {
                    TokenType::MinusEqual
                } else {
                    TokenType::Minus
                };
                Ok(Some(self.make_token(ty)))
            }
            '+' => {
                let ty = if self.match_char('=') { TokenType::PlusEqual } else { TokenType::Plus };
                Ok(Some(self.make_token(ty)))
            }
            '=' => {
                let ty = if self.match_char('=') {
                    TokenType::EqualEqual
                } else if self.match_char('>') {
                    TokenType::FatArrow
                } else {
                    TokenType::Equal
                };
                Ok(Some(self.make_token(ty)))
            }
            '<' => {
                let ty = if self.match_char('=') { TokenType::LessEqual } else { TokenType::Less };
                Ok(Some(self.make_token(ty)))
            }
            '>' => {
                let ty = if self.match_char('=') { TokenType::GreaterEqual } else { TokenType::Greater };
                Ok(Some(self.make_token(ty)))
            }
            '!' => {
                let ty = if self.match_char('=') { TokenType::BangEqual } else { TokenType::Bang };
                Ok(Some(self.make_token(ty)))
            }
            '/' => {
                Ok(Some(self.make_token(TokenType::Slash)))
            }
            '#' => {
                if self.match_char('~') {
                    // Multi-line comment: #~ ... ~#
                    while !self.is_at_end() {
                        if self.peek() == '~' && self.peek_next() == '#' {
                            self.advance(); // consume ~
                            self.advance(); // consume #
                            break;
                        }
                        self.advance();
                    }
                    Ok(None)
                } else {
                    // Single line comment
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                    Ok(None)
                }
            }
            ' ' | '\r' | '\t' => Ok(None),
            '\n' => {
                self.line += 1;
                self.column = 1;
                Ok(None)
            }
            '"' => self.string(),
            c if c.is_ascii_digit() => self.number(),
            c if c.is_ascii_alphabetic() || c == '_' => self.identifier(),
            _ => Err(format!("Unexpected character '{}' at line {}", c, self.line)),
        }
    }

    fn identifier(&mut self) -> Result<Option<Token>, String> {
        while self.peek().is_ascii_alphanumeric() || self.peek() == '_' {
            self.advance();
        }

        let text = self.current_text();
        let ty = self.match_keyword(&text).unwrap_or(TokenType::Identifier);
        Ok(Some(self.make_token(ty)))
    }

    fn number(&mut self) -> Result<Option<Token>, String> {
        let mut is_float = false;
        while self.peek().is_ascii_digit() {
            self.advance();
        }

        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            is_float = true;
            self.advance(); // consume '.'
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }

        let ty = if is_float { TokenType::FloatLit } else { TokenType::IntLit };
        Ok(Some(self.make_token(ty)))
    }

    fn string(&mut self) -> Result<Option<Token>, String> {
        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line += 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            return Err(format!("Unterminated string at line {}", self.line));
        }

        self.advance(); // The closing quote
        Ok(Some(self.make_token(TokenType::StringLit)))
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.source[self.current] != expected {
            return false;
        }
        self.current += 1;
        self.column += 1;
        true
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current];
        self.current += 1;
        self.column += 1;
        c
    }

    fn peek(&self) -> char {
        if self.is_at_end() { '\0' } else { self.source[self.current] }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.source.len() {
            '\0'
        } else {
            self.source[self.current + 1]
        }
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn current_text(&self) -> String {
        self.source[self.start..self.current].iter().collect()
    }

    fn make_token(&self, ty: TokenType) -> Token {
        Token::new(ty, self.current_text(), self.line, self.start_column)
    }

    fn match_keyword(&self, text: &str) -> Option<TokenType> {
        match text {
            "let" => Some(TokenType::Let),
            "const" => Some(TokenType::Const),
            "task" => Some(TokenType::Task),
            "return" => Some(TokenType::Return),
            "if" => Some(TokenType::If),
            "elif" => Some(TokenType::Elif),
            "else" => Some(TokenType::Else),
            "for" => Some(TokenType::For),
            "in" => Some(TokenType::In),
            "while" => Some(TokenType::While),
            "struct" => Some(TokenType::Struct),
            "class" => Some(TokenType::Class),
            "extends" => Some(TokenType::Extends),
            "match" => Some(TokenType::Match),
            "try" => Some(TokenType::Try),
            "catch" => Some(TokenType::Catch),
            "import" => Some(TokenType::Import),
            "from" => Some(TokenType::From),
            "self" => Some(TokenType::Self_),
            "void" => Some(TokenType::Void),
            "true" => Some(TokenType::True),
            "false" => Some(TokenType::False),
            "enum" => Some(TokenType::Enum),
            "type" => Some(TokenType::Type),
            "any" => Some(TokenType::Any),
            "dict" => Some(TokenType::Dict),
            "set" => Some(TokenType::Set),
            "tuple" => Some(TokenType::Tuple),
            "int" => Some(TokenType::TypeInt),
            "float" => Some(TokenType::TypeFloat),
            "str" => Some(TokenType::TypeStr),
            "bool" => Some(TokenType::TypeBool),
            "throw" => Some(TokenType::Throw),
            "finally" => Some(TokenType::Finally),
            "export" => Some(TokenType::Export),
            "async" => Some(TokenType::Async),
            "await" => Some(TokenType::Await),
            "as" => Some(TokenType::As),
            "pub" => Some(TokenType::Pub),
            "priv" => Some(TokenType::Priv),
            "interface" => Some(TokenType::Interface),
            "abstract" => Some(TokenType::Abstract),
            "static" => Some(TokenType::Static),
            "implements" => Some(TokenType::Implements),
            _ => None,
        }
    }
}
