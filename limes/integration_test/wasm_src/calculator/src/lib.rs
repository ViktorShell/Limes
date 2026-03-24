use atoi::atoi;
use limes_macro::limes_run;
use serde_json::Value;
use std::iter::Peekable;
use std::str::SplitWhitespace;

#[limes_run]
fn run(expression: String) -> String {
    let expression = extract_expression(&expression);
    if expression.contains("FAILED TO PARSE") {
        return "ERROR: not a valid expression of wrong json format".to_string();
    }

    let mut tokens = expression.split_whitespace().peekable();
    match parse_expr(&mut tokens, 0) {
        Ok(expr) => expr.solve().to_string(),
        Err(e) => e.to_string(),
    }
}

fn remove_apx(value: &str) -> &str {
    let len = value.len();
    &value[1..len - 1]
}

fn extract_expression(json_expr_str: &str) -> String {
    let json_expr: Value = match serde_json::from_str(json_expr_str) {
        Ok(v) => v,
        _ => return "FAILED TO PARSE".to_string(),
    };
    let expression = json_expr["expression"].to_string();
    let expression = remove_apx(&expression);
    expression.to_string()
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
        None => {
            return Err(
                "This is not a valid expression, e.g of a valid one is `5 + 2 - 3 * 4 / 1`",
            );
        }
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
