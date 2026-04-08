use atoi::atoi;
use limes_macro::limes_run;

#[limes_run]
fn run(input: String) -> String {
    let mut iter = input.split_ascii_whitespace();
    let ascii_a = iter.next().unwrap();
    let ascii_op = iter.next().unwrap();
    let ascii_b = iter.next().unwrap();

    let a = atoi::<i32>(ascii_a.as_bytes()).unwrap();
    let b = atoi::<i32>(ascii_b.as_bytes()).unwrap();

    let exec = match ascii_op {
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        "/" => a / b,
        _ => 0,
    };

    exec.to_string()
}
