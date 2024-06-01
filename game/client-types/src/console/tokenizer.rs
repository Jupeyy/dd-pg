use std::ops::Range;

use anyhow::anyhow;
use logos::Logos;

pub trait HumanReadableToken {
    fn human_readable(&self) -> String;
}

#[derive(Logos, Debug, Copy, Clone, PartialEq)]
#[logos(skip r"[ \t\n\f]+")] // Ignore this regex pattern between tokens
pub enum Token {
    #[token(";")]
    Semicolon,
    #[regex(r"[+]?[a-zA-Z]+[\[\]a-zA-Z_\.0-9+]*")]
    Text,
    #[regex("\"([^\"\\\\]+|\\\\\\\"|\\\\\\\\)*\"")]
    Quoted,
    #[regex(r"[-+]?[0-9]+(\.[0-9]+)?")]
    Number,
    #[token("{")]
    BraceLeft,
    #[token("}")]
    BraceRight,
    #[token("[")]
    BracketLeft,
    #[token("]")]
    BracketRight,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
}

impl HumanReadableToken for Token {
    fn human_readable(&self) -> String {
        format!(
            "\"{}\"",
            match self {
                Token::Semicolon => ";",
                Token::Text => "text",
                Token::Quoted => "quoted expression",
                Token::Number => "number",
                Token::BraceLeft => "[",
                Token::BraceRight => "]",
                Token::BracketLeft => "{",
                Token::BracketRight => "}",
                Token::Colon => ":",
                Token::Comma => ",",
            }
        )
    }
}

pub fn token_err(s: &str) -> Option<anyhow::Error> {
    if s.trim().starts_with("\"") {
        Some(anyhow!("Expected closing \""))
    } else {
        None
    }
}

pub type Tokens = Vec<(Token, String, Range<usize>)>;
pub fn tokenize(s: &str) -> anyhow::Result<Tokens, (Tokens, (String, Range<usize>))> {
    let mut lexer = Token::lexer(s);
    let mut res: Tokens = Default::default();

    while let Some(token) = lexer.next() {
        match token {
            Ok(token) => res.push((token, lexer.slice().to_string(), lexer.span())),
            Err(_) => {
                return Err((res, (lexer.slice().to_string(), lexer.span())));
            }
        }
    }
    Ok(res)
}

#[cfg(test)]
mod test {
    use logos::Logos;

    use crate::console::tokenizer::{tokenize, Token};

    #[test]
    fn console_tests() {
        let mut lex = Token::lexer("cl.map \"name with spaces\"");

        assert_eq!(lex.next(), Some(Ok(Token::Text)));
        assert_eq!(lex.slice(), "cl.map");

        assert_eq!(lex.next(), Some(Ok(Token::Quoted)));
        assert_eq!(lex.slice(), "\"name with spaces\"");

        assert_eq!(lex.next(), None);

        let mut lex = Token::lexer("+toggle cl.refresh_rate -0 +1000");

        assert_eq!(lex.next(), Some(Ok(Token::Text)));
        assert_eq!(lex.slice(), "+toggle");

        assert_eq!(lex.next(), Some(Ok(Token::Text)));
        assert_eq!(lex.slice(), "cl.refresh_rate");

        assert_eq!(lex.next(), Some(Ok(Token::Number)));
        assert_eq!(lex.slice(), "-0");

        assert_eq!(lex.next(), Some(Ok(Token::Number)));
        assert_eq!(lex.slice(), "+1000");

        assert_eq!(lex.next(), None);

        let mut lex = Token::lexer("\"test\\\"test\"");

        assert_eq!(lex.next(), Some(Ok(Token::Quoted)));
        assert_eq!(lex.slice(), "\"test\\\"test\"");

        assert_eq!(lex.next(), None);

        let mut lex = Token::lexer("{\"test\":5,\"test2\":false,\"test3\":\"str\"}");

        assert_eq!(lex.next(), Some(Ok(Token::BraceLeft)));
        dbg!(&lex.collect::<Vec<_>>());

        let mut lex = Token::lexer("\"name with spaces\"d");

        assert_eq!(lex.next(), Some(Ok(Token::Quoted)));
        assert_eq!(lex.slice(), "\"name with spaces\"");

        assert_eq!(lex.next(), Some(Ok(Token::Text)));
        assert_eq!(lex.slice(), "d");

        assert_eq!(lex.next(), None);

        let mut lex = Token::lexer("player[0].name");

        assert_eq!(lex.next(), Some(Ok(Token::Text)));
        assert_eq!(lex.slice(), "player[0].name");

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn err_console_tests() {
        let (tokens, (broken_token, _)) = tokenize("cl.map \"name with spaces").unwrap_err();
        let mut tokens = tokens.into_iter();

        assert_eq!(
            tokens.next(),
            Some((Token::Text, "cl.map".to_string(), 0..6))
        );

        assert_eq!(tokens.next(), None);

        assert_eq!(broken_token, "\"name with spaces");
    }
}
