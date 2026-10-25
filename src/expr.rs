//! Signed 64-bit expression parser. This module does not evaluate expressions.
use crate::Issue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Integer(i64),
    Variable(String),
    Neg(Box<Expr>),
    Binary(Box<Expr>, Operator, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator { Add, Sub, Mul, Div }

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Integer(i64), Ident(String), Add, Sub, Mul, Div, Left, Right,
}

pub fn valid_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
        && chars.all(|c| c == '_' || c.is_alphanumeric())
        && !matches!(s, "出勤" | "残業" | "定時退社" | "有給" | "過労死" | "報告")
}

fn lex(source: &str, line: usize) -> Result<Vec<Token>, Issue> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_digit() {
            let mut n = String::new();
            while chars.peek().is_some_and(char::is_ascii_digit) {
                n.push(chars.next().unwrap());
            }
            let value = n.parse::<i64>().map_err(|_| Issue {
                line, message: format!("整数リテラルがi64の範囲外です: {n}"),
            })?;
            tokens.push(Token::Integer(value));
        } else if c == '_' || c.is_alphabetic() {
            let mut name = String::new();
            while chars.peek().is_some_and(|c| *c == '_' || c.is_alphanumeric()) {
                name.push(chars.next().unwrap());
            }
            tokens.push(Token::Ident(name));
        } else {
            chars.next();
            tokens.push(match c {
                '+' => Token::Add,
                '-' => Token::Sub,
                '*' => Token::Mul,
                '/' => Token::Div,
                '(' => Token::Left,
                ')' => Token::Right,
                _ => return Err(Issue {
                    line, message: format!("式で使用できない文字です: {c}"),
                }),
            });
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
    line: usize,
}

impl Parser {
    fn error(&self, message: &str) -> Issue {
        Issue { line: self.line, message: message.into() }
    }

    fn atom(&mut self) -> Result<Expr, Issue> {
        let token = self.tokens.get(self.at).cloned()
            .ok_or_else(|| self.error("式が途中で終わっています"))?;
        self.at += 1;
        match token {
            Token::Integer(n) => Ok(Expr::Integer(n)),
            Token::Ident(s) if valid_name(&s) => Ok(Expr::Variable(s)),
            Token::Sub => Ok(Expr::Neg(Box::new(self.atom()?))),
            Token::Left => {
                let e = self.binary(0)?;
                if self.tokens.get(self.at) != Some(&Token::Right) {
                    return Err(self.error("閉じ括弧がありません"));
                }
                self.at += 1;
                Ok(e)
            }
            _ => Err(self.error("式のオペランドが不正です")),
        }
    }

    fn binary(&mut self, min_prec: u8) -> Result<Expr, Issue> {
        let mut left = self.atom()?;
        loop {
            let (op, prec) = match self.tokens.get(self.at) {
                Some(Token::Add) => (Operator::Add, 1),
                Some(Token::Sub) => (Operator::Sub, 1),
                Some(Token::Mul) => (Operator::Mul, 2),
                Some(Token::Div) => (Operator::Div, 2),
                _ => break,
            };
            if prec < min_prec { break; }
            self.at += 1;
            let right = self.binary(prec + 1)?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }
}

pub fn parse_expression(source: &str, line: usize) -> Result<Expr, Issue> {
    let mut p = Parser { tokens: lex(source, line)?, at: 0, line };
    let expr = p.binary(0)?;
    if p.at != p.tokens.len() {
        return Err(p.error("式の末尾に不要なトークンがあります"));
    }
    Ok(expr)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precedence() {
        assert_eq!(parse_expression("1 + 2 * 3", 1).unwrap(),
            Expr::Binary(Box::new(Expr::Integer(1)), Operator::Add, Box::new(
                Expr::Binary(Box::new(Expr::Integer(2)), Operator::Mul, Box::new(Expr::Integer(3))))));
    }
    #[test]
    fn syntax_errors() {
        for src in ["", "1 +", "(1 + 2", "1 + 2)", "1 ** 3", "10.2", "9223372036854775808", "1 $ 2"] {
            assert!(parse_expression(src, 4).is_err(), "{src}");
        }
    }
    #[test]
    fn unicode_names() {
        assert!(parse_expression("基本給 + 残業時間", 1).is_ok());
        assert!(!valid_name("残業"));
        assert!(!valid_name("12円"));
    }
}
