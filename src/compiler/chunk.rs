//! Bytecode chunk representation

use super::OpCode;
use crate::vm::Value;

/// A chunk of bytecode
#[derive(Debug, Clone, Default)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub lines: Vec<u32>,
}

impl Chunk {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emit an opcode
    pub fn emit(&mut self, op: OpCode, line: u32) {
        self.code.push(op as u8);
        self.lines.push(line);
    }

    /// Emit a byte operand
    pub fn emit_byte(&mut self, byte: u8, line: u32) {
        self.code.push(byte);
        self.lines.push(line);
    }

    /// Emit a u16 operand (little-endian)
    pub fn emit_u16(&mut self, value: u16, line: u32) {
        self.code.push((value & 0xFF) as u8);
        self.code.push((value >> 8) as u8);
        self.lines.push(line);
        self.lines.push(line);
    }

    /// Emit an i16 operand (little-endian)
    pub fn emit_i16(&mut self, value: i16, line: u32) {
        self.emit_u16(value as u16, line);
    }

    /// Emit an i32 operand (little-endian)
    pub fn emit_i32(&mut self, value: i32, line: u32) {
        let bytes = value.to_le_bytes();
        for byte in bytes {
            self.code.push(byte);
            self.lines.push(line);
        }
    }

    /// Add a constant and return its index
    pub fn add_constant(&mut self, value: Value) -> u16 {
        // Check if constant already exists
        for (i, c) in self.constants.iter().enumerate() {
            if c == &value {
                return i as u16;
            }
        }
        self.constants.push(value);
        (self.constants.len() - 1) as u16
    }

    /// Get current code offset
    pub fn offset(&self) -> usize {
        self.code.len()
    }

    /// Patch a jump offset at the given position
    pub fn patch_jump(&mut self, offset: usize) {
        let jump = self.code.len() - offset - 2; // -2 for the offset bytes
        if jump > i16::MAX as usize {
            panic!("Jump too large");
        }
        let bytes = (jump as i16).to_le_bytes();
        self.code[offset] = bytes[0];
        self.code[offset + 1] = bytes[1];
    }

    /// Get line number for an offset
    pub fn get_line(&self, offset: usize) -> u32 {
        self.lines.get(offset).copied().unwrap_or(0)
    }

    /// Disassemble the chunk
    pub fn disassemble(&self, name: &str) -> String {
        let mut result = format!("== {} ==\n", name);
        let mut offset = 0;

        while offset < self.code.len() {
            let (disasm, new_offset) = self.disassemble_instruction(offset);
            result.push_str(&format!("{:04} {}\n", offset, disasm));
            offset = new_offset;
        }

        result
    }

    fn disassemble_instruction(&self, offset: usize) -> (String, usize) {
        let op = OpCode::try_from(self.code[offset]).unwrap_or(OpCode::LoadNil);
        let _line = self.get_line(offset);

        match op {
            OpCode::LoadNil | OpCode::LoadTrue | OpCode::LoadFalse | OpCode::ReturnNil => {
                let dst = self.code.get(offset + 1).copied().unwrap_or(0);
                (format!("{:12} r{}", op, dst), offset + 2)
            }

            OpCode::LoadConst => {
                let dst = self.code.get(offset + 1).copied().unwrap_or(0);
                let idx = self.read_u16(offset + 2);
                let value = self.constants.get(idx as usize).cloned().unwrap_or(Value::Nil);
                (format!("{:12} r{} <- const[{}] ({})", op, dst, idx, value), offset + 4)
            }

            OpCode::LoadInt => {
                let dst = self.code.get(offset + 1).copied().unwrap_or(0);
                let value = self.read_i32(offset + 2);
                (format!("{:12} r{} <- {}", op, dst, value), offset + 6)
            }

            OpCode::Move | OpCode::Neg | OpCode::Not | OpCode::BitNot | OpCode::Return => {
                let dst = self.code.get(offset + 1).copied().unwrap_or(0);
                let src = self.code.get(offset + 2).copied().unwrap_or(0);
                (format!("{:12} r{} <- r{}", op, dst, src), offset + 3)
            }

            OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div | OpCode::Mod |
            OpCode::Pow | OpCode::Eq | OpCode::Ne | OpCode::Lt | OpCode::Le |
            OpCode::Gt | OpCode::Ge | OpCode::And | OpCode::Or | OpCode::BitAnd |
            OpCode::BitOr | OpCode::BitXor | OpCode::Shl | OpCode::Shr | OpCode::Concat => {
                let dst = self.code.get(offset + 1).copied().unwrap_or(0);
                let left = self.code.get(offset + 2).copied().unwrap_or(0);
                let right = self.code.get(offset + 3).copied().unwrap_or(0);
                (format!("{:12} r{} <- r{} op r{}", op, dst, left, right), offset + 4)
            }

            OpCode::Jump | OpCode::Loop => {
                let jmp = self.read_i16(offset + 1);
                (format!("{:12} -> {}", op, (offset as i32) + 3 + (jmp as i32)), offset + 3)
            }

            OpCode::JumpIf | OpCode::JumpIfNot => {
                let cond = self.code.get(offset + 1).copied().unwrap_or(0);
                let jmp = self.read_i16(offset + 2);
                (format!("{:12} r{} -> {}", op, cond, (offset as i32) + 4 + (jmp as i32)), offset + 4)
            }

            OpCode::Call | OpCode::TailCall => {
                let dst = self.code.get(offset + 1).copied().unwrap_or(0);
                let func = self.code.get(offset + 2).copied().unwrap_or(0);
                let argc = self.code.get(offset + 3).copied().unwrap_or(0);
                (format!("{:12} r{} <- r{}({})", op, dst, func, argc), offset + 4)
            }

            OpCode::GetGlobal | OpCode::SetGlobal => {
                let reg = self.code.get(offset + 1).copied().unwrap_or(0);
                let idx = self.read_u16(offset + 2);
                (format!("{:12} r{} global[{}]", op, reg, idx), offset + 4)
            }

            OpCode::GetLocal | OpCode::SetLocal => {
                let reg = self.code.get(offset + 1).copied().unwrap_or(0);
                let slot = self.code.get(offset + 2).copied().unwrap_or(0);
                (format!("{:12} r{} local[{}]", op, reg, slot), offset + 3)
            }

            _ => (format!("{}", op), offset + 1 + op.operand_size()),
        }
    }

    fn read_u16(&self, offset: usize) -> u16 {
        let lo = self.code.get(offset).copied().unwrap_or(0) as u16;
        let hi = self.code.get(offset + 1).copied().unwrap_or(0) as u16;
        lo | (hi << 8)
    }

    fn read_i16(&self, offset: usize) -> i16 {
        self.read_u16(offset) as i16
    }

    fn read_i32(&self, offset: usize) -> i32 {
        let bytes = [
            self.code.get(offset).copied().unwrap_or(0),
            self.code.get(offset + 1).copied().unwrap_or(0),
            self.code.get(offset + 2).copied().unwrap_or(0),
            self.code.get(offset + 3).copied().unwrap_or(0),
        ];
        i32::from_le_bytes(bytes)
    }
}

impl TryFrom<u8> for OpCode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        // Safety: We only convert valid opcodes
        if value <= 121 {
            Ok(unsafe { std::mem::transmute(value) })
        } else {
            Err(())
        }
    }
}

/// A compiled function
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub arity: u8,
    pub num_locals: u8,
    pub chunk: Chunk,
    pub upvalues: Vec<UpvalueInfo>,
}

impl Function {
    pub fn new(name: impl Into<String>, arity: u8) -> Self {
        Self {
            name: name.into(),
            arity,
            num_locals: 0,
            chunk: Chunk::new(),
            upvalues: Vec::new(),
        }
    }
}

/// Information about an upvalue capture
#[derive(Debug, Clone, Copy)]
pub struct UpvalueInfo {
    pub index: u8,
    pub is_local: bool,
}

/// A compiled module
#[derive(Debug, Clone)]
pub struct Module {
    pub main: Function,
    pub functions: Vec<Function>,
}

/// Magic number for Vector bytecode files: "VECT"
const BYTECODE_MAGIC: u32 = 0x56454354;
/// Bytecode format version
const BYTECODE_VERSION: u32 = 1;

impl Module {
    /// Serialize the module to bytecode format
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // Header
        bytes.extend_from_slice(&BYTECODE_MAGIC.to_le_bytes());
        bytes.extend_from_slice(&BYTECODE_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(self.functions.len() as u32).to_le_bytes());
        
        // Main function
        self.write_function(&self.main, &mut bytes);
        
        // Other functions
        for func in &self.functions {
            self.write_function(func, &mut bytes);
        }
        
        bytes
    }

    fn write_function(&self, func: &Function, bytes: &mut Vec<u8>) {
        // Function name (length-prefixed string)
        let name_bytes = func.name.as_bytes();
        bytes.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name_bytes);
        
        // Arity and num_locals
        bytes.push(func.arity);
        bytes.push(func.num_locals);
        
        // Upvalues
        bytes.push(func.upvalues.len() as u8);
        for uv in &func.upvalues {
            bytes.push(uv.index);
            bytes.push(if uv.is_local { 1 } else { 0 });
        }
        
        // Code
        bytes.extend_from_slice(&(func.chunk.code.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&func.chunk.code);
        
        // Constants
        bytes.extend_from_slice(&(func.chunk.constants.len() as u32).to_le_bytes());
        for constant in &func.chunk.constants {
            self.write_value(constant, bytes);
        }
        
        // Line info (RLE compressed)
        bytes.extend_from_slice(&(func.chunk.lines.len() as u32).to_le_bytes());
        for &line in &func.chunk.lines {
            bytes.extend_from_slice(&line.to_le_bytes());
        }
    }

    fn write_value(&self, value: &Value, bytes: &mut Vec<u8>) {
        match value {
            Value::Nil => bytes.push(0),
            Value::Bool(b) => {
                bytes.push(1);
                bytes.push(if *b { 1 } else { 0 });
            }
            Value::Int(i) => {
                bytes.push(2);
                bytes.extend_from_slice(&i.to_le_bytes());
            }
            Value::Float(f) => {
                bytes.push(3);
                bytes.extend_from_slice(&f.to_le_bytes());
            }
            Value::String(s) => {
                bytes.push(4);
                let s_bytes = s.as_bytes();
                bytes.extend_from_slice(&(s_bytes.len() as u32).to_le_bytes());
                bytes.extend_from_slice(s_bytes);
            }
            // Arrays, Tables, Functions, etc. are runtime-only
            _ => bytes.push(0), // Serialize as nil
        }
    }

    /// Deserialize a module from bytecode format
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut offset = 0;
        
        // Header
        if bytes.len() < 12 {
            return None;
        }
        
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != BYTECODE_MAGIC {
            return None;
        }
        offset += 4;
        
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != BYTECODE_VERSION {
            return None;
        }
        offset += 4;
        
        let num_functions = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        offset += 4;
        
        // Main function
        let (main, new_offset) = Self::read_function(bytes, offset)?;
        offset = new_offset;
        
        // Other functions
        let mut functions = Vec::with_capacity(num_functions);
        for _ in 0..num_functions {
            let (func, new_offset) = Self::read_function(bytes, offset)?;
            functions.push(func);
            offset = new_offset;
        }
        
        Some(Module { main, functions })
    }

    fn read_function(bytes: &[u8], mut offset: usize) -> Option<(Function, usize)> {
        // Name length
        if offset + 4 > bytes.len() { return None; }
        let name_len = u32::from_le_bytes([
            bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]
        ]) as usize;
        offset += 4;
        
        // Name
        if offset + name_len > bytes.len() { return None; }
        let name = String::from_utf8(bytes[offset..offset+name_len].to_vec()).ok()?;
        offset += name_len;
        
        // Arity and num_locals
        if offset + 2 > bytes.len() { return None; }
        let arity = bytes[offset];
        let num_locals = bytes[offset + 1];
        offset += 2;
        
        // Upvalues
        if offset + 1 > bytes.len() { return None; }
        let num_upvalues = bytes[offset] as usize;
        offset += 1;
        
        let mut upvalues = Vec::with_capacity(num_upvalues);
        for _ in 0..num_upvalues {
            if offset + 2 > bytes.len() { return None; }
            let index = bytes[offset];
            let is_local = bytes[offset + 1] != 0;
            upvalues.push(UpvalueInfo { index, is_local });
            offset += 2;
        }
        
        // Code
        if offset + 4 > bytes.len() { return None; }
        let code_len = u32::from_le_bytes([
            bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]
        ]) as usize;
        offset += 4;
        
        if offset + code_len > bytes.len() { return None; }
        let code = bytes[offset..offset+code_len].to_vec();
        offset += code_len;
        
        // Constants
        if offset + 4 > bytes.len() { return None; }
        let num_constants = u32::from_le_bytes([
            bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]
        ]) as usize;
        offset += 4;
        
        let mut constants = Vec::with_capacity(num_constants);
        for _ in 0..num_constants {
            let (value, new_offset) = Self::read_value(bytes, offset)?;
            constants.push(value);
            offset = new_offset;
        }
        
        // Lines
        if offset + 4 > bytes.len() { return None; }
        let num_lines = u32::from_le_bytes([
            bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]
        ]) as usize;
        offset += 4;
        
        let mut lines = Vec::with_capacity(num_lines);
        for _ in 0..num_lines {
            if offset + 4 > bytes.len() { return None; }
            let line = u32::from_le_bytes([
                bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]
            ]);
            lines.push(line);
            offset += 4;
        }
        
        let func = Function {
            name,
            arity,
            num_locals,
            chunk: Chunk { code, constants, lines },
            upvalues,
        };
        
        Some((func, offset))
    }

    fn read_value(bytes: &[u8], mut offset: usize) -> Option<(Value, usize)> {
        if offset >= bytes.len() { return None; }
        
        let tag = bytes[offset];
        offset += 1;
        
        match tag {
            0 => Some((Value::Nil, offset)),
            1 => {
                if offset >= bytes.len() { return None; }
                let b = bytes[offset] != 0;
                Some((Value::Bool(b), offset + 1))
            }
            2 => {
                if offset + 8 > bytes.len() { return None; }
                let i = i64::from_le_bytes([
                    bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3],
                    bytes[offset+4], bytes[offset+5], bytes[offset+6], bytes[offset+7],
                ]);
                Some((Value::Int(i), offset + 8))
            }
            3 => {
                if offset + 8 > bytes.len() { return None; }
                let f = f64::from_le_bytes([
                    bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3],
                    bytes[offset+4], bytes[offset+5], bytes[offset+6], bytes[offset+7],
                ]);
                Some((Value::Float(f), offset + 8))
            }
            4 => {
                if offset + 4 > bytes.len() { return None; }
                let len = u32::from_le_bytes([
                    bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3],
                ]) as usize;
                offset += 4;
                
                if offset + len > bytes.len() { return None; }
                let s = String::from_utf8(bytes[offset..offset+len].to_vec()).ok()?;
                Some((Value::String(s), offset + len))
            }
            _ => Some((Value::Nil, offset)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_emit() {
        let mut chunk = Chunk::new();
        chunk.emit(OpCode::LoadNil, 1);
        chunk.emit_byte(0, 1);
        assert_eq!(chunk.code.len(), 2);
    }

    #[test]
    fn test_chunk_constants() {
        let mut chunk = Chunk::new();
        let idx1 = chunk.add_constant(Value::Int(42));
        let idx2 = chunk.add_constant(Value::Int(42)); // Should reuse
        let idx3 = chunk.add_constant(Value::Int(100));
        assert_eq!(idx1, 0);
        assert_eq!(idx2, 0); // Same constant
        assert_eq!(idx3, 1);
    }

    #[test]
    fn test_chunk_patch_jump() {
        let mut chunk = Chunk::new();
        chunk.emit(OpCode::Jump, 1);
        let jump_offset = chunk.offset();
        chunk.emit_i16(0, 1); // Placeholder
        chunk.emit(OpCode::LoadNil, 1);
        chunk.emit_byte(0, 1);
        chunk.patch_jump(jump_offset);
        // Should have patched to skip 2 bytes
    }
}
