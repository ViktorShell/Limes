use atoi::atoi;
use limes_macro::limes_run;
use std::iter::Peekable;
use std::str::SplitWhitespace;

#[limes_run]
fn run(args: String) -> String {
    let expression = match extract(&args) {
        Some(exp) => exp,
        _ => return "ERROR: not a valid expression".to_string(),
    };
    let mut tokens = expression.split_whitespace().peekable();
    match parse_expr(&mut tokens, 0) {
        Ok(expr) => expr.solve().to_string(),
        Err(e) => e.to_string(),
    }
}

fn extract(input: &str) -> Option<&str> {
    // Cerca la posizione di "expression": "
    let pattern = r#""expression": ""#;
    let start = input.find(pattern)? + pattern.len();

    // Prende tutto ciò che segue fino alle virgolette di chiusura
    let rest = &input[start..];
    let end = rest.find('"')?;

    Some(&rest[..end])
}

#[derive(Debug)]
enum Expr {
    Int(i32),
    Binary(Box<Expr>, Op, Box<Expr>),
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    fn binding_power(&self) -> (u8, u8) {
        match self {
            Op::Add | Op::Sub => (1, 2),
            Op::Mul | Op::Div => (3, 4),
        }
    }
}

impl Expr {
    fn solve(&self) -> i32 {
        match self {
            Expr::Int(n) => *n,
            Expr::Binary(lhs, op, rhs) => {
                let l = lhs.solve();
                let r = rhs.solve();
                match op {
                    Op::Add => l + r,
                    Op::Sub => l - r,
                    Op::Mul => l * r,
                    Op::Div => l / r,
                }
            }
        }
    }
}

fn parse_expr(tokens: &mut Peekable<SplitWhitespace>, min_bp: u8) -> Result<Expr, &'static str> {
    let token = tokens.next().ok_or("Unexpected end of input")?;
    let mut lhs = match atoi::<i32>(token.as_bytes()) {
        Some(n) => Expr::Int(n),
        None => return Err("Expected a number"),
    };

    while let Some(&op_str) = tokens.peek() {
        let op = match op_str {
            "+" => Op::Add,
            "-" => Op::Sub,
            "*" => Op::Mul,
            "/" => Op::Div,
            _ => break,
        };

        let (l_bp, r_bp) = op.binding_power();

        if l_bp < min_bp {
            break;
        }

        tokens.next();

        let rhs = parse_expr(tokens, r_bp)?;

        lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs));
    }

    Ok(lhs)
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn test_basic_operations() {
//         assert_eq!(run("10 + 5".to_string()), "15");
//         assert_eq!(run("10 - 5".to_string()), "5");
//         assert_eq!(run("10 * 5".to_string()), "50");
//         assert_eq!(run("10 / 2".to_string()), "5");
//     }
//
//     #[test]
//     fn test_precedence() {
//         // Multiplicative operators should bind tighter than additive
//         // 1 + (2 * 3) = 7, NOT (1 + 2) * 3 = 9
//         assert_eq!(run("1 + 2 * 3".to_string()), "7");
//
//         // (10 / 2) - 3 = 2, NOT 10 / (2 - 3) = -10
//         assert_eq!(run("10 / 2 - 3".to_string()), "2");
//
//         // 5 * 4 + 3 * 2 = 20 + 6 = 26
//         assert_eq!(run("5 * 4 + 3 * 2".to_string()), "26");
//     }
//
//     #[test]
//     fn test_negative_results() {
//         assert_eq!(run("5 - 10".to_string()), "-5");
//     }
//
//     #[test]
//     fn test_errors() {
//         // Test invalid numbers
//         assert_eq!(run("10 + abc".to_string()), "Expected a number");
//
//         // Test invalid operators
//         assert_eq!(run("10 % 2".to_string()), "10");
//
//         // Test empty input
//         assert_eq!(run("".to_string()), "Unexpected end of input");
//     }
// }
