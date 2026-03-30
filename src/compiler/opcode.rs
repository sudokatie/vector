//! Bytecode instruction definitions

/// Bytecode opcodes (register-based)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    // Load constants to register
    LoadNil = 0,      // dst
    LoadTrue = 1,     // dst
    LoadFalse = 2,    // dst
    LoadInt = 3,      // dst, imm32 (4 bytes)
    LoadConst = 4,    // dst, const_idx (2 bytes)

    // Register operations
    Move = 10,        // dst, src
    Copy = 11,        // dst, src (deep copy)

    // Arithmetic (dst, left, right)
    Add = 20,
    Sub = 21,
    Mul = 22,
    Div = 23,
    Mod = 24,
    Pow = 25,
    Neg = 26,         // dst, src (unary)

    // Comparison (dst, left, right)
    Eq = 30,
    Ne = 31,
    Lt = 32,
    Le = 33,
    Gt = 34,
    Ge = 35,

    // Logical
    And = 40,         // dst, left, right
    Or = 41,          // dst, left, right
    Not = 42,         // dst, src

    // Bitwise (dst, left, right)
    BitAnd = 50,
    BitOr = 51,
    BitXor = 52,
    BitNot = 53,      // dst, src (unary)
    Shl = 54,
    Shr = 55,

    // String
    Concat = 60,      // dst, left, right

    // Control flow
    Jump = 70,        // offset (2 bytes, signed)
    JumpIf = 71,      // cond, offset
    JumpIfNot = 72,   // cond, offset
    Loop = 73,        // offset (jump backward)

    // Functions
    Call = 80,        // dst, func, argc
    TailCall = 81,    // func, argc
    Return = 82,      // src
    ReturnNil = 83,
    MethodCall = 84,  // dst, obj, method_name, argc - calls obj.method or method(obj, args)

    // Variables
    GetGlobal = 90,   // dst, name_idx
    SetGlobal = 91,   // name_idx, src
    GetLocal = 92,    // dst, slot
    SetLocal = 93,    // slot, src
    GetUpvalue = 94,  // dst, idx
    SetUpvalue = 95,  // idx, src

    // Collections
    NewArray = 100,   // dst, count (count elements on stack)
    ArrayGet = 101,   // dst, arr, idx
    ArraySet = 102,   // arr, idx, val
    NewTable = 103,   // dst
    TableGet = 104,   // dst, tbl, key
    TableSet = 105,   // tbl, key, val

    // Closures
    Closure = 110,    // dst, func_idx
    CloseUpvalue = 111, // slot

    // Ranges
    MakeRange = 115,      // dst, start, end (exclusive)
    MakeRangeIncl = 116,  // dst, start, end (inclusive)

    // Iteration
    GetIter = 120,    // dst, iterable
    IterNext = 121,   // dst_val, dst_done, iter - sets dst_val to next value, dst_done to true if exhausted
}

impl OpCode {
    /// Get the number of operand bytes for this opcode
    pub fn operand_size(self) -> usize {
        match self {
            OpCode::LoadNil | OpCode::LoadTrue | OpCode::LoadFalse |
            OpCode::ReturnNil => 1, // just dst

            OpCode::Move | OpCode::Copy | OpCode::Neg | OpCode::Not | OpCode::BitNot |
            OpCode::Return | OpCode::GetLocal | OpCode::SetLocal |
            OpCode::GetUpvalue | OpCode::SetUpvalue | OpCode::CloseUpvalue |
            OpCode::NewArray | OpCode::GetIter => 2, // dst, src/slot

            OpCode::LoadInt => 5, // dst + i32
            OpCode::LoadConst | OpCode::GetGlobal | OpCode::SetGlobal |
            OpCode::Closure | OpCode::NewTable => 3, // dst + u16

            OpCode::Jump | OpCode::Loop => 2, // i16 offset

            OpCode::JumpIf | OpCode::JumpIfNot => 3, // cond + i16 offset

            OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div |
            OpCode::Mod | OpCode::Pow | OpCode::Eq | OpCode::Ne |
            OpCode::Lt | OpCode::Le | OpCode::Gt | OpCode::Ge |
            OpCode::And | OpCode::Or | OpCode::BitAnd | OpCode::BitOr |
            OpCode::BitXor | OpCode::Shl | OpCode::Shr | OpCode::Concat |
            OpCode::ArrayGet | OpCode::ArraySet | OpCode::TableGet |
            OpCode::TableSet | OpCode::Call | OpCode::TailCall |
            OpCode::IterNext | OpCode::MakeRange | OpCode::MakeRangeIncl => 3, // 3 register operands
            
            OpCode::MethodCall => 4, // dst, obj, method_name, argc
        }
    }

    /// Check if this opcode is a jump
    pub fn is_jump(self) -> bool {
        matches!(self, OpCode::Jump | OpCode::JumpIf | OpCode::JumpIfNot | OpCode::Loop | OpCode::IterNext)
    }
}

impl std::fmt::Display for OpCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
