use limes_macro::limes_run;
use serde_json::Value;

#[limes_run]
fn run(prompt: String) -> String {
    let task_json: Value = serde_json::from_str(&prompt).unwrap();
    let task = task_json["task"]
        .as_str()
        .unwrap_or("Failed to give the task");
    invoke_agent(task)
}
