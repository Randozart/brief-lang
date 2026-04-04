#[derive(Debug, Clone)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Data,
    Void,
    Custom(String),
    Union(Vec<Type>),
    ContractBound(Box<Type>, Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum ResultType {
    Projection(Vec<Type>),
    TrueAssertion,
}

#[derive(Debug, Clone)]
pub enum Expr {
    // Literals
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),

    // Identifiers and references
    Identifier(String),
    OwnedRef(String),
    PriorState(String),

    // Binary operators
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),

    // Unary operators
    Not(Box<Expr>),
    Neg(Box<Expr>),
    BitNot(Box<Expr>),

    // Call
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub enum Statement {
    // Assignment: &identifier = expr; or identifier = expr;
    Assignment {
        is_owned: bool,
        name: String,
        expr: Expr,
    },

    // Unification: identifier(pattern) = expr;
    Unification {
        name: String,
        pattern: String,
        expr: Expr,
    },

    // Guarded statement: [expr] statement
    Guarded {
        condition: Expr,
        statement: Box<Statement>,
    },

    // Term statement: term expr?, expr?, ... (multi-output with trailing commas for void)
    Term(Vec<Option<Expr>>),

    // Escape statement: escape expr?;
    Escape(Option<Expr>),

    // Expression statement: expr;
    Expression(Expr),

    // Let binding: let name: Type = expr;
    Let {
        name: String,
        ty: Option<Type>,
        expr: Option<Expr>,
    },
}

#[derive(Debug, Clone)]
pub struct Contract {
    pub pre_condition: Expr,
    pub post_condition: Expr,
}

#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub input_types: Vec<Type>,
    pub result_type: ResultType,
    pub source: Option<String>,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub outputs: Vec<Type>,
    pub contract: Contract,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub is_async: bool,
    pub is_reactive: bool,
    pub name: String,
    pub contract: Contract,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct StateDecl {
    pub name: String,
    pub ty: Type,
    pub expr: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct Constant {
    pub name: String,
    pub ty: Type,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub items: Vec<ImportItem>,
    pub path: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ImportItem {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ForeignSig {
    pub name: String,
    pub input_types: Vec<Type>,
    pub outputs: Vec<Type>,
}

#[derive(Debug, Clone)]
pub enum TopLevel {
    Signature(Signature),
    Definition(Definition),
    Transaction(Transaction),
    StateDecl(StateDecl),
    Constant(Constant),
    Import(Import),
    ForeignSig(ForeignSig),
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<TopLevel>,
    pub comments: Vec<Comment>,
}
