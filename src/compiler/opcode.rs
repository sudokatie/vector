//! Bytecode instruction definitions

/// Bytecode opcodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    // Load constants
    LoadNil,
    LoadTrue,
    LoadFalse,
    LoadInt,
    LoadConst,

    // Stack operations
    Pop,
    Dup,

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Neg,

    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical
    And,
    Or,
    Not,

    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    Shl,
    Shr,

    // String
    Concat,

    // Control flow
    Jump,
    JumpIf,
    JumpIfNot,
    Loop,

    // Functions
    Call,
    TailCall,
    Return,
    ReturnNil,

    // Variables
    GetGlobal,
    SetGlobal,
    GetLocal,
    SetLocal,
    GetUpvalue,
    SetUpvalue,

    // Collections
    NewArray,
    ArrayGet,
    ArraySet,
    NewTable,
    TableGet,
    TableSet,

    // Closures
    Closure,
    CloseUpvalue,
}
