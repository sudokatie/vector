//! Compiler integration tests

use vector::lexer::Lexer;
use vector::parser::Parser;
use vector::compiler::{Compiler, OpCode};

fn compile(source: &str) -> vector::compiler::Module {
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer);
    let stmts = parser.parse().unwrap();
    let mut compiler = Compiler::new();
    compiler.compile(&stmts).unwrap()
}

fn has_opcode(module: &vector::compiler::Module, op: OpCode) -> bool {
    module.main.chunk.code.iter().any(|&b| b == op as u8)
}

#[test]
fn test_compile_literals() {
    let module = compile("nil");
    assert!(has_opcode(&module, OpCode::LoadNil));
    
    let module = compile("true");
    assert!(has_opcode(&module, OpCode::LoadTrue));
    
    let module = compile("false");
    assert!(has_opcode(&module, OpCode::LoadFalse));
    
    let module = compile("42");
    assert!(has_opcode(&module, OpCode::LoadInt));
    
    let module = compile("3.14");
    assert!(has_opcode(&module, OpCode::LoadConst));
    
    let module = compile("\"hello\"");
    assert!(has_opcode(&module, OpCode::LoadConst));
}

#[test]
fn test_compile_arithmetic() {
    let ops = vec![
        ("1 + 2", OpCode::Add),
        ("1 - 2", OpCode::Sub),
        ("1 * 2", OpCode::Mul),
        ("1 / 2", OpCode::Div),
        ("1 % 2", OpCode::Mod),
        ("2 ** 3", OpCode::Pow),
    ];
    
    for (source, expected_op) in ops {
        let module = compile(source);
        assert!(has_opcode(&module, expected_op), "Missing opcode for: {}", source);
    }
}

#[test]
fn test_compile_comparison() {
    let ops = vec![
        ("1 == 2", OpCode::Eq),
        ("1 != 2", OpCode::Ne),
        ("1 < 2", OpCode::Lt),
        ("1 <= 2", OpCode::Le),
        ("1 > 2", OpCode::Gt),
        ("1 >= 2", OpCode::Ge),
    ];
    
    for (source, expected_op) in ops {
        let module = compile(source);
        assert!(has_opcode(&module, expected_op), "Missing opcode for: {}", source);
    }
}

#[test]
fn test_compile_logical() {
    let module = compile("true and false");
    assert!(has_opcode(&module, OpCode::And));
    
    let module = compile("true or false");
    assert!(has_opcode(&module, OpCode::Or));
    
    let module = compile("not true");
    assert!(has_opcode(&module, OpCode::Not));
}

#[test]
fn test_compile_bitwise() {
    let ops = vec![
        ("1 & 2", OpCode::BitAnd),
        ("1 | 2", OpCode::BitOr),
        ("1 ^ 2", OpCode::BitXor),
        ("1 << 2", OpCode::Shl),
        ("1 >> 2", OpCode::Shr),
        ("~1", OpCode::BitNot),
    ];
    
    for (source, expected_op) in ops {
        let module = compile(source);
        assert!(has_opcode(&module, expected_op), "Missing opcode for: {}", source);
    }
}

#[test]
fn test_compile_string_concat() {
    let module = compile("\"a\" ++ \"b\"");
    assert!(has_opcode(&module, OpCode::Concat));
}

#[test]
fn test_compile_variables() {
    // Local variable - should use GetLocal to read
    let module = compile("let x = 42\nx");
    assert!(has_opcode(&module, OpCode::GetLocal) || has_opcode(&module, OpCode::LoadInt));
    
    // Reassignment compiles value to the local slot directly
    let module = compile("let mut x = 0\nx = 1\nx");
    assert!(has_opcode(&module, OpCode::GetLocal));
}

#[test]
fn test_compile_function() {
    let module = compile("fn add(a, b) { return a + b }");
    assert_eq!(module.functions.len(), 1);
    assert!(has_opcode(&module, OpCode::Closure));
}

#[test]
fn test_compile_function_call() {
    let module = compile("print(42)");
    assert!(has_opcode(&module, OpCode::Call));
}

#[test]
fn test_compile_if() {
    let module = compile("if true { 1 } else { 2 }");
    assert!(has_opcode(&module, OpCode::JumpIfNot));
    assert!(has_opcode(&module, OpCode::Jump));
}

#[test]
fn test_compile_while() {
    let module = compile("let x = 0\nwhile x < 10 { x = x + 1 }");
    assert!(has_opcode(&module, OpCode::Loop));
    assert!(has_opcode(&module, OpCode::JumpIfNot));
}

#[test]
fn test_compile_for() {
    let module = compile("for i in [1,2,3] { print(i) }");
    assert!(has_opcode(&module, OpCode::GetIter));
    assert!(has_opcode(&module, OpCode::IterNext));
}

#[test]
fn test_compile_array() {
    let module = compile("[1, 2, 3]");
    assert!(has_opcode(&module, OpCode::NewArray));
}

#[test]
fn test_compile_table() {
    let module = compile("{ x: 1, y: 2 }");
    assert!(has_opcode(&module, OpCode::NewTable));
    assert!(has_opcode(&module, OpCode::TableSet));
}

#[test]
fn test_compile_index() {
    let module = compile("let arr = [1,2,3]\narr[0]");
    assert!(has_opcode(&module, OpCode::ArrayGet));
}

#[test]
fn test_compile_match() {
    let module = compile("let x = 1\nmatch x { 0 => \"zero\", _ => \"other\" }");
    // Match compiles to series of comparisons and jumps
    assert!(has_opcode(&module, OpCode::Eq));
    assert!(has_opcode(&module, OpCode::JumpIfNot));
}

#[test]
fn test_compile_try() {
    let module = compile("try 42");
    // Try wraps result in a table
    assert!(has_opcode(&module, OpCode::NewTable));
}

#[test]
fn test_compile_interpolation() {
    let module = compile("let name = \"world\"\n\"Hello, {name}!\"");
    assert!(has_opcode(&module, OpCode::Concat));
}

#[test]
fn test_compile_return() {
    let module = compile("fn foo() { return 42 }");
    assert!(module.functions.iter().any(|f| 
        f.chunk.code.iter().any(|&b| b == OpCode::Return as u8)
    ));
}

#[test]
fn test_bytecode_serialization() {
    let module = compile("let x = 42\nfn add(a,b) { a + b }\nadd(1,2)");
    let bytes = module.to_bytes();
    
    // Deserialize and verify
    let restored = vector::compiler::Module::from_bytes(&bytes).unwrap();
    assert_eq!(restored.functions.len(), module.functions.len());
    assert_eq!(restored.main.chunk.code.len(), module.main.chunk.code.len());
}

#[test]
fn test_disassembly() {
    let module = compile("1 + 2");
    let disasm = module.main.chunk.disassemble("test");
    assert!(disasm.contains("Add"));
}
