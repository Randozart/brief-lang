use crate::errors::Span;

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
    TypeVar(String),
    Generic(String, Vec<Type>),
    Applied(String, Vec<Type>),
    Sig(String),       // Signature used as function type: sig name -> ...
    Option(Box<Type>), // Option<T> - Some(T) or None
}

#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    pub bounds: Vec<TypeBound>,
}

#[derive(Debug, Clone)]
pub enum TypeBound {
    Eq(Type),
    SubTypeOf(Type),
    SuperTypeOf(Type),
    HasTrait(String),
}

#[derive(Debug, Clone)]
pub enum ResultType {
    Projection(Vec<Type>),
    TrueAssertion,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Identifier(String),
    OwnedRef(String),
    PriorState(String),
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
    Not(Box<Expr>),
    Neg(Box<Expr>),
    BitNot(Box<Expr>),
    Call(String, Vec<Expr>),
    ListLiteral(Vec<Expr>),
    ListIndex(Box<Expr>, Box<Expr>),
    ListLen(Box<Expr>),
    FieldAccess(Box<Expr>, String),
}

impl Expr {
    pub fn span(&self) -> Option<Span> {
        None
    }
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
    pub span: Option<Span>,
}

impl Contract {
    pub fn new(pre: Expr, post: Expr) -> Self {
        Contract {
            pre_condition: pre,
            post_condition: post,
            span: None,
        }
    }
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
    pub type_params: Vec<TypeParam>,
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
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct StateDecl {
    pub name: String,
    pub ty: Type,
    pub expr: Option<Expr>,
    pub span: Option<Span>,
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
    Struct(StructDefinition),
    RStruct(RStructDefinition),
    RenderBlock(RenderBlock),
}

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub name: String,
    pub fields: Vec<StructField>,
    pub transactions: Vec<Transaction>,
    pub view_html: Option<String>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
}

impl StructDefinition {
    pub fn new(name: String) -> Self {
        StructDefinition {
            name,
            fields: Vec::new(),
            transactions: Vec::new(),
            view_html: None,
            span: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RStructDefinition {
    pub name: String,
    pub fields: Vec<StructField>,
    pub transactions: Vec<Transaction>,
    pub view_html: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct RenderBlock {
    pub struct_name: String,
    pub view_html: String,
    pub span: Option<Span>,
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
