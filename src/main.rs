mod lexer;
use lexer::Lexer;
use lexer::Token;

fn main() {
    test_lexing();
}


fn test_lexing() {
    let input = "let result = (42 + 8) * result_v2 / 2;";
    let lexer = Lexer::new(input);
    let tokens = lexer.tokenize();

    assert_eq!(
        tokens,
        vec![
            Token::Let,
            Token::Ident("result"),
            Token::Assign,
            Token::LParen,
            Token::Int(42),
            Token::Plus,
            Token::Int(8),
            Token::RParen,
            Token::Star,
            Token::Ident("result_v2"),
            Token::Slash,
            Token::Int(2),
            Token::Semi,
        ]
    );
    println!("lexer passed test!");
}

