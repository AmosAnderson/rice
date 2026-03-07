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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeSuffix {
    Integer, // %
    Long,    // &
    Single,  // !
    Double,  // #
    String,  // $
}

impl TypeSuffix {
    pub fn from_char(ch: char) -> Option<Self> {
        match ch {
            '%' => Some(Self::Integer),
            '&' => Some(Self::Long),
            '!' => Some(Self::Single),
            '#' => Some(Self::Double),
            '$' => Some(Self::String),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Self::Integer => '%',
            Self::Long => '&',
            Self::Single => '!',
            Self::Double => '#',
            Self::String => '$',
        }
    }

    pub fn to_basic_type(self) -> crate::value::BasicType {
        match self {
            Self::Integer => crate::value::BasicType::Integer,
            Self::Long => crate::value::BasicType::Long,
            Self::Single => crate::value::BasicType::Single,
            Self::Double => crate::value::BasicType::Double,
            Self::String => crate::value::BasicType::String,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    IntegerLiteral(i64),
    DoubleLiteral(f64),
    StringLiteral(String),

    // Identifiers
    Identifier {
        name: String, // always UPPERCASE
        suffix: Option<TypeSuffix>,
    },

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
    Backslash, // \ integer division
    Caret,     // ^
    Equal,     // = (assignment AND comparison)
    NotEqual,  // <>
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
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
    KwRedim,
    KwErase,
    KwPreserve,
    KwOption,
    KwBase,
    KwSwap,
    KwEndSub,
    KwEndFunction,
    KwEndSelect,
    KwEndType,
    KwType,
    KwData,
    KwRead,
    KwRestore,
    KwOpen,
    KwClose,
    KwWrite,
    KwAppend,
    KwOutput,
    KwBinary,
    KwRandom,
    KwLen,
    KwGet,
    KwPut,
    KwFreefile,
    KwLPrint,
    KwUsing,
    KwOn,
    KwError,
    KwResume,

    // Logical/bitwise operators (keywords)
    KwAnd,
    KwOr,
    KwNot,
    KwXor,
    KwEqv,
    KwImp,
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

    // Phase 1: new keywords
    KwSleep,
    KwClear,
    KwName,
    KwKill,
    KwMkdir,
    KwRmdir,
    KwChdir,
    KwShell,

    // Phase 2: string mutation
    KwLset,
    KwRset,

    // Phase 4: DEFtype and DEF FN
    KwDef,
    KwEndDef,
    KwDefInt,
    KwDefLng,
    KwDefSng,
    KwDefDbl,
    KwDefStr,
}
