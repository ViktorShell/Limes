use atoi::atoi;
use limes_macro::limes_run;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct RequestData {
    expression: String,
}

struct ParsedData {
    a: i32,
    op: char,
    b: i32,
}

fn parse_data(json_str: &str) -> Result<ParsedData, &'static str> {
    let data: RequestData =
        serde_json::from_str(json_str).map_err(|_| "Could not parse the expresison")?;

    let parts: Vec<&str> = data.expression.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(
            "Not a valid format, the expression must be of 2 values and one operation type",
        );
    }

    let a = parts[0]
        .parse::<i32>()
        .map_err(|_| "a is not a valid number")?;
    let op = parts[1].chars().next().ok_or("not a valid operator")?;
    let b = parts[2]
        .parse::<i32>()
        .map_err(|_| "b is not a valid number")?;
    Ok(ParsedData { a, op, b })
}

#[limes_run]
fn run(input: String) -> String {
    let parsed_data = match parse_data(&input) {
        Ok(data) => data,
        Err(e) => return e.into(),
    };

    let a = parsed_data.a;
    let op = parsed_data.op;
    let b = parsed_data.b;

    let exec = match op {
        '+' => a + b,
        '-' => a - b,
        '*' => a * b,
        '/' => a / b,
        _ => 0,
    };

    exec.to_string()
}
