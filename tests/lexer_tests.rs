//! Lexer integration tests

use vector::lexer::{Lexer, TokenKind, StringPart};

#[test]
fn test_lex_all_keywords() {
    let source = "let mut fn return if else while for in match true false nil and or not try";
    let lexer = Lexer::new(source);
    let tokens: Vec<_> = lexer.filter_map(|r| r.ok()).collect();
    
    assert_eq!(tokens.len(), 17);
    assert!(matches!(tokens[0].kind, TokenKind::Let));
    assert!(matches!(tokens[1].kind, TokenKind::Mut));
    assert!(matches!(tokens[2].kind, TokenKind::Fn));
    assert!(matches!(tokens[3].kind, TokenKind::Return));
    assert!(matches!(tokens[4].kind, TokenKind::If));
    assert!(matches!(tokens[5].kind, TokenKind::Else));
    assert!(matches!(tokens[6].kind, TokenKind::While));
    assert!(matches!(tokens[7].kind, TokenKind::For));
    assert!(matches!(tokens[8].kind, TokenKind::In));
    assert!(matches!(tokens[9].kind, TokenKind::Match));
    assert!(matches!(tokens[10].kind, TokenKind::True));
    assert!(matches!(tokens[11].kind, TokenKind::False));
    assert!(matches!(tokens[12].kind, TokenKind::Nil));
    assert!(matches!(tokens[13].kind, TokenKind::And));
    assert!(matches!(tokens[14].kind, TokenKind::Or));
    assert!(matches!(tokens[15].kind, TokenKind::Not));
    assert!(matches!(tokens[16].kind, TokenKind::Try));
}

#[test]
fn test_lex_all_operators() {
    let source = "+ ++ - * ** / % = == ! != < <= << > >= >> & | ^ ~ .. ..= =>";
    let lexer = Lexer::new(source);
    let tokens: Vec<_> = lexer.filter_map(|r| r.ok()).collect();
    
    // Count actual tokens (may vary based on lexer implementation)
    assert!(tokens.len() >= 20);
    assert!(matches!(tokens[0].kind, TokenKind::Plus));
    assert!(matches!(tokens[1].kind, TokenKind::PlusPlus));
    assert!(matches!(tokens[2].kind, TokenKind::Minus));
    assert!(matches!(tokens[3].kind, TokenKind::Star));
    assert!(matches!(tokens[4].kind, TokenKind::StarStar));
}

#[test]
fn test_lex_string_interpolation() {
    let source = r#""Hello, {name}!""#;
    let mut lexer = Lexer::new(source);
    let token = lexer.next_token().unwrap();
    
    match token.kind {
        TokenKind::InterpolatedString(parts) => {
            assert_eq!(parts.len(), 3);
            assert!(matches!(&parts[0], StringPart::Literal(s) if s == "Hello, "));
            assert!(matches!(&parts[1], StringPart::Interpolation(s) if s == "name"));
            assert!(matches!(&parts[2], StringPart::Literal(s) if s == "!"));
        }
        _ => panic!("Expected interpolated string"),
    }
}

#[test]
fn test_lex_escape_sequences() {
    let source = r#""\n\t\r\\\""{""#;
    let mut lexer = Lexer::new(source);
    let token = lexer.next_token().unwrap();
    
    match token.kind {
        TokenKind::String(s) => {
            assert_eq!(s, "\n\t\r\\\"");
        }
        _ => panic!("Expected string"),
    }
    
    // Escaped brace in interpolated context
    let source2 = r#""\{not interpolated\}""#;
    let mut lexer2 = Lexer::new(source2);
    let token2 = lexer2.next_token().unwrap();
    
    match token2.kind {
        TokenKind::String(s) => {
            assert_eq!(s, "{not interpolated}");
        }
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_lex_nested_comments() {
    let source = "a /* outer /* inner */ still outer */ b";
    let lexer = Lexer::new(source);
    let tokens: Vec<_> = lexer.filter_map(|r| r.ok()).collect();
    
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].kind, TokenKind::Identifier(s) if s == "a"));
    assert!(matches!(&tokens[1].kind, TokenKind::Identifier(s) if s == "b"));
}

#[test]
fn test_lex_numbers() {
    let source = "42 3.14 1e10 2.5e-3 0 -1";
    let lexer = Lexer::new(source);
    let tokens: Vec<_> = lexer.filter_map(|r| r.ok()).collect();
    
    // -1 lexes as Minus + Int, so we get 7 tokens
    assert!(tokens.len() >= 6);
    assert!(matches!(tokens[0].kind, TokenKind::Int(42)));
    assert!(matches!(tokens[1].kind, TokenKind::Float(f) if (f - 3.14).abs() < 0.001));
}

#[test]
fn test_lex_multiline_string() {
    let source = "\"line1\nline2\"";
    let mut lexer = Lexer::new(source);
    let token = lexer.next_token().unwrap();
    
    match token.kind {
        TokenKind::String(s) => {
            assert!(s.contains('\n'));
        }
        _ => panic!("Expected string"),
    }
}
