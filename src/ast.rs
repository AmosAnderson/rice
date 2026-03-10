use crate::token::TypeSuffix;

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<LabeledStmt>,
}

#[derive(Debug, Clone)]
pub struct LabeledStmt {
    pub label: Option<Label>,
    pub stmt: Stmt,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Label {
    Number(u32),
    Name(String),
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Label::Number(n) => write!(f, "{n}"),
            Label::Name(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Print(PrintStmt),
    Let {
        var: Variable,
        expr: Expr,
    },
    Dim(Vec<DimDecl>),
    Const {
        name: String,
        value: Expr,
    },
    Input(InputStmt),
    LineInput {
        prompt: Option<String>,
        var: Variable,
    },

    // Control flow
    If(IfStmt),
    For(ForStmt),
    WhileWend {
        condition: Expr,
        body: Vec<LabeledStmt>,
    },
    DoLoop(DoLoopStmt),
    SelectCase(SelectCaseStmt),
    Goto(Label),
    Gosub(Label),
    Return,
    ExitFor,
    ExitDo,

    // Procedures
    SubDef(SubDef),
    FunctionDef(FunctionDef),
    Call {
        name: String,
        args: Vec<Expr>,
    },
    ExitSub,
    ExitFunction,
    Declare(DeclareStmt),

    // Arrays
    Redim {
        preserve: bool,
        decls: Vec<DimDecl>,
    },
    Erase(Vec<String>),
    OptionBase(i32),
    Swap {
        a: Variable,
        b: Variable,
    },

    // Data
    Data(Vec<DataItem>),
    Read(Vec<Variable>),
    Restore(Option<Label>),

    // File I/O
    Open(OpenStmt),
    Close(Vec<Expr>),
    PrintFile(FilePrintStmt),
    WriteFile(FileWriteStmt),
    InputFile(FileInputStmt),
    LineInputFile {
        file_num: Expr,
        var: Variable,
    },
    GetPut(GetPutStmt),

    // Error handling
    OnErrorGoto(Option<Label>),
    OnGoto { expr: Expr, labels: Vec<Label> },
    OnGosub { expr: Expr, labels: Vec<Label> },
    Resume(ResumeTarget),

    // Random
    Randomize(Option<Expr>),

    // Phase 1: new statements
    Write(Vec<Expr>),
    Sleep(Option<Expr>),
    Clear,
    Name { old: Expr, new: Expr },
    Kill(Expr),
    Mkdir(Expr),
    Rmdir(Expr),
    Chdir(Expr),
    Shell(Option<Expr>),

    // Phase 2: string mutation
    MidAssign { var: Variable, start: Expr, length: Option<Expr>, replacement: Expr },
    Lset { var: Variable, expr: Expr },
    Rset { var: Variable, expr: Expr },

    // Phase 3: scope
    Shared(Vec<Variable>),
    Static(Vec<DimDecl>),

    // Phase 4: DEFtype and DEF FN
    DefType { typ: BasicType, ranges: Vec<(char, char)> },
    DefFn { name: String, params: Vec<Param>, body: DefFnBody },

    // User-defined types
    TypeDef { name: String, fields: Vec<TypeField> },
    MemberAssign { target: Expr, value: Expr },

    // CHAIN/COMMON
    Chain { filespec: Expr },
    Common(CommonStmt),

    // FIELD/SEEK
    Field { file_num: Expr, fields: Vec<FieldDef> },
    Seek { file_num: Expr, position: Expr },

    // Console
    Cls,
    Beep,
    Locate { row: Option<Expr>, col: Option<Expr> },
    Color { fg: Option<Expr>, bg: Option<Expr> },
    Width { columns: Option<Expr>, rows: Option<Expr> },
    ViewPrint { top: Option<Expr>, bottom: Option<Expr> },

    // Misc
    End,
    System,
    Stop,
    Rem,
    ExprStmt(Expr),
}

#[derive(Debug, Clone)]
pub struct TypeField {
    pub name: String,
    pub field_type: BasicType,
}

#[derive(Debug, Clone)]
pub struct CommonVar {
    pub name: String,
    pub suffix: Option<TypeSuffix>,
    pub as_type: Option<BasicType>,
    pub is_array: bool,
}

#[derive(Debug, Clone)]
pub struct CommonStmt {
    pub shared: bool,
    pub block_name: Option<String>,
    pub vars: Vec<CommonVar>,
}

#[derive(Debug, Clone)]
pub enum DefFnBody {
    SingleLine(Expr),
    MultiLine(Vec<LabeledStmt>),
}

#[derive(Debug, Clone)]
pub struct PrintStmt {
    pub format: Option<Expr>,
    pub items: Vec<PrintItem>,
    pub trailing: PrintSep,
}

#[derive(Debug, Clone)]
pub enum PrintItem {
    Expr(Expr),
    Tab(Expr),
    Spc(Expr),
    Comma, // zone separator
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrintSep {
    Newline,
    Semicolon,
    Comma,
}

#[derive(Debug, Clone)]
pub struct InputStmt {
    pub prompt: Option<String>,
    pub vars: Vec<Variable>,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_body: Vec<LabeledStmt>,
    pub elseif_clauses: Vec<(Expr, Vec<LabeledStmt>)>,
    pub else_body: Option<Vec<LabeledStmt>>,
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub var: Variable,
    pub start: Expr,
    pub end: Expr,
    pub step: Option<Expr>,
    pub body: Vec<LabeledStmt>,
}

#[derive(Debug, Clone)]
pub struct DoLoopStmt {
    pub condition: Option<Expr>,
    pub check_at_top: bool,
    pub is_while: bool, // true = WHILE, false = UNTIL
    pub body: Vec<LabeledStmt>,
}

#[derive(Debug, Clone)]
pub struct SelectCaseStmt {
    pub expr: Expr,
    pub cases: Vec<CaseClause>,
    pub else_body: Option<Vec<LabeledStmt>>,
}

#[derive(Debug, Clone)]
pub struct CaseClause {
    pub tests: Vec<CaseTest>,
    pub body: Vec<LabeledStmt>,
}

#[derive(Debug, Clone)]
pub enum CaseTest {
    Value(Expr),
    Range(Expr, Expr),
    Comparison(CompareOp, Expr),
}

#[derive(Debug, Clone)]
pub struct DimDecl {
    pub name: String,
    pub suffix: Option<TypeSuffix>,
    pub as_type: Option<BasicType>,
    pub dimensions: Option<Vec<(Expr, Option<Expr>)>>, // (upper,) or (lower, upper)
}

#[derive(Debug, Clone)]
pub struct SubDef {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<LabeledStmt>,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: String,
    pub suffix: Option<TypeSuffix>,
    pub params: Vec<Param>,
    pub as_type: Option<BasicType>,
    pub body: Vec<LabeledStmt>,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub suffix: Option<TypeSuffix>,
    pub as_type: Option<BasicType>,
    pub by_val: bool,
    pub is_array: bool,
}

#[derive(Debug, Clone)]
pub struct DeclareStmt {
    pub is_function: bool, // true = FUNCTION, false = SUB
    pub name: String,
    pub suffix: Option<TypeSuffix>,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone)]
pub enum DataItem {
    Number(f64),
    Str(String),
}

#[derive(Debug, Clone)]
pub struct OpenStmt {
    pub filename: Expr,
    pub mode: FileMode,
    pub file_num: Expr,
    pub rec_len: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileMode {
    Input,
    Output,
    Append,
    Random,
    Binary,
}

#[derive(Debug, Clone)]
pub struct FilePrintStmt {
    pub file_num: Expr,
    pub format: Option<Expr>,
    pub items: Vec<PrintItem>,
    pub trailing: PrintSep,
}

#[derive(Debug, Clone)]
pub struct FileWriteStmt {
    pub file_num: Expr,
    pub exprs: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct FileInputStmt {
    pub file_num: Expr,
    pub vars: Vec<Variable>,
}

#[derive(Debug, Clone)]
pub struct GetPutStmt {
    pub is_get: bool,
    pub file_num: Expr,
    pub record: Option<Expr>,
    pub var: Option<Variable>,
}

#[derive(Debug, Clone)]
pub enum ResumeTarget {
    Default,
    Next,
    Label(Label),
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub width: Expr,
    pub var: Variable,
}

// Expressions

#[derive(Debug, Clone)]
pub enum Expr {
    IntegerLit(i64),
    DoubleLit(f64),
    StringLit(String),
    Variable(Variable),
    ArrayIndex {
        name: String,
        suffix: Option<TypeSuffix>,
        indices: Vec<Expr>,
    },
    BinaryOp {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    FunctionCall {
        name: String,
        suffix: Option<TypeSuffix>,
        args: Vec<Expr>,
    },
    Paren(Box<Expr>),
    MemberAccess {
        object: Box<Expr>,
        field: String,
    },
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub suffix: Option<TypeSuffix>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    IntDiv,
    Mod,
    Pow,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Xor,
    Eqv,
    Imp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
    Pos,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

// Re-export BasicType from value.rs to avoid duplication
pub use crate::value::BasicType;
