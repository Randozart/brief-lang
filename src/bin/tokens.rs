use brief_compiler::lexer::Token;
use logos::Logos;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --bin tokens -- <file.bv>");
        return;
    }

    let file_path = &args[1];
    let source = std::fs::read_to_string(file_path).expect("Failed to read file");

    let lexer = Token::lexer(&source);
    for (token, span) in lexer.spanned() {
        match token {
            Ok(t) => println!("{:?} at {:?}", t, span),
            Err(_) => println!("Lexer error at {:?}", span),
        }
    }
}
