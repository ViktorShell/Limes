use limes_macro::limes_run;

#[limes_run]
fn run(input: String) -> String {
    let input_json: serde_json::Value = serde_json::from_str(&input).unwrap();
    let a = input_json["a"].as_u64().unwrap_or(0);
    let b = input_json["b"].as_u64().unwrap_or(0);
    let op = input_json["op"].as_str().unwrap_or("+");

    let exec = match op {
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        "/" => a / b,
        _ => 0,
    }
    .to_string();

    format!(r#"{{"content": {} }}"#, exec)
}
