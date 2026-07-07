#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    NumericLiteral(f64),
    StringLiteral(String),

    // Identifiers
    Identifier(String), // always UPPERCASE

    // Line structure
    LineNumber(u32),
    Colon,
    Newline,
    Eof,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Caret,    // ^
    Equal,    // = (assignment AND comparison)
    NotEqual, // <>
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Ampersand, // & (string concatenation)
    LeftParen,
    RightParen,
    Comma,
    Semicolon,
    Hash, // # for file numbers
    Dot,  // . for member access

    // Keywords
    KwPrint,
    KwInput,
    KwLineInput,
    KwLet,
    KwDim,
    KwConst,
    KwAs,
    KwIf,
    KwThen,
    KwElse,
    KwElseIf,
    KwEndIf,
    KwFor,
    KwTo,
    KwStep,
    KwNext,
    KwWhile,
    KwWend,
    KwDo,
    KwLoop,
    KwUntil,
    KwGoto,
    KwGosub,
    KwReturn,
    KwSelect,
    KwCase,
    KwIs,
    KwEnd,
    KwStop,
    KwExit,
    KwSub,
    KwFunction,
    KwCall,
    KwDeclare,
    KwShared,
    KwStatic,
    KwByVal,
    KwByRef,
    KwRedim,
    KwErase,
    KwPreserve,
    KwOption,
    KwBase,
    KwExplicit,
    KwSwap,
    KwEndSub,
    KwEndFunction,
    KwEndSelect,
    KwEndType,
    KwEndWhile,
    KwType,
    KwData,
    KwRead,
    KwRestore,
    KwOpen,
    KwClose,
    KwWrite,
    KwOutput,
    KwLen,
    KwAccess,
    KwOrganization,
    KwSequential,
    KwStream,
    KwOutIn,
    KwSet,
    KwAsk,
    KwPointer,
    KwGet,
    KwPut,
    KwFreefile,
    KwSeek,
    KwReset,
    KwLPrint,
    KwUsing,
    KwOn,
    KwOff,
    KwKey,
    KwError,
    KwResume,

    // Logical operators (keywords)
    KwAnd,
    KwOr,
    KwNot,
    KwXor,
    KwMod,

    KwRem,

    // PRINT helpers
    KwTab,
    KwSpc,

    // Type keywords
    KwInteger,
    KwLong,
    KwSingle,
    KwDouble,
    KwString,

    // Randomize
    KwRandomize,
    KwTimer,
    KwSystem,

    // Statements
    KwSleep,
    KwClear,
    KwName,
    KwKill,
    KwMkdir,
    KwRmdir,
    KwChdir,
    KwChdrive,
    KwFiles,
    KwShell,

    // String mutation
    KwLset,
    KwRset,

    // DEFtype and DEF FN
    KwDef,
    KwEndDef,
    KwDefInt,
    KwDefLng,
    KwDefSng,
    KwDefDbl,
    KwDefStr,

    // CHAIN/COMMON support
    KwChain,
    KwCommon,

    // Console
    KwCls,
    KwBeep,
    KwLocate,
    KwColor,
    KwWidth,
    KwView,

    // FIELD (legacy, unsupported)
    KwField,

    // WHEN EXCEPTION
    KwWhen,
    KwException,
    KwUse,
    KwRetry,
    KwContinue,
    KwEndWhen,

    // MAT operations
    KwMat,
}
