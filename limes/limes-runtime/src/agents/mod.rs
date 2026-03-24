use anyhow::{Context, Result};
use log::*;
use rig::agent::Agent;
use rig::client::{CompletionClient, Nothing};
use rig::completion::{CompletionModel, Prompt, ToolDefinition};
use rig::providers::ollama;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use std::future::Future;
use std::pin::Pin;

use crate::runtime::{
    types::{FunctionId, UserId},
    Runtime,
};

// Limes Agent

pub struct LimesAgent<M: CompletionModel> {
    agent: Agent<M>,
}

impl LimesAgent<ollama::CompletionModel> {
    pub async fn new_agent(host: &str, model: &str, user_id: UserId) -> Result<Self> {
        let client = ollama::Client::builder()
            .base_url(host)
            .api_key(Nothing)
            .build()
            .context("LimesAgent: failed to build Ollama client")?;

        let list_tool = UserDefinedFunctions::build(&user_id).await?;
        let exec_tool = ExecuteUserDefinedFunction::new(&user_id);

        let system_prompt = r#"
You are a task assistant, the first thing you MUST DO is to call the UserDefinedFunctions which gives you the list, as a string, of all usable functions, those functions can be called only by calling the ExecuteUserDefinedFunction and giving it the Function Id of the wanted function and the actual input for the function.
So once you have called the UserDefinedFunctions tool you can assist the user with the tasks he requires.
        "#;

        //         let system_prompt = r#"
        // Assistant Task: Execute user-defined tools precisely.
        // 1. You MUST Use `UserDefinedFunctions` to discover tools.
        // 2. You MUST Use `ExecuteUserDefinedFunction` for calls, strictly following the specified input schema.
        // Only use these tools.
        // "#;

        let force_tool = rig::completion::message::ToolChoice::Specific {
            function_names: vec![
                "UserDefinedFunctions".to_string(),
                "ExecuteUserDefinedFunction".to_string(),
            ],
        };

        let agent = client
            .agent(model)
            .preamble(system_prompt)
            .tool(list_tool)
            .tool(exec_tool)
            .tool_choice(force_tool)
            .build();

        info!(
            r#"
LimesAgent initialized:
    host: {host}
    model: {model}
    user_id: {user_id}
        "#
        );
        Ok(Self { agent })
    }

    pub async fn invoke_agent(&self, prompt: String) -> Result<String> {
        debug!(
            r#"
Agent prompt received:
    prompt: {prompt}
"#
        );

        let answer = self
            .agent
            .prompt(prompt)
            .await
            .context("LimesAgent: prompt call failed");

        // FIX: MUST REMOVE
        let answer = answer.unwrap();
        println!("\n\n\n{}\n\n\n", answer.clone());

        Ok(answer)
    }

    /// Returns a boxed closure that executes a specific user function through
    /// the global runtime singleton. Used by the `ExecuteFunction` tool.
    pub fn make_exec_closure(user_id: UserId, function_id: FunctionId) -> ExecClosure {
        Box::new(move |input: String| {
            let uid = user_id.clone();
            let fid = function_id.clone();
            Box::pin(async move {
                let rt = Runtime::get_runtime_ref().map_err(|e| anyhow::anyhow!("{e}"))?;
                let funcs = rt.get_user_functions(&uid).await?;
                let handler = funcs
                    .iter()
                    .find(|f| f.function_id == fid)
                    .ok_or_else(|| anyhow::anyhow!("Agent: function {fid} not found"))?;
                handler.lambda.run(&input).await
            })
        })
    }
}

type PinnedFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type ExecClosure = Box<dyn Fn(String) -> PinnedFuture<Result<String>> + Send + Sync>;

// Gives the list of all user defined tools to the LLM
pub struct UserDefinedFunctions {
    /// (id, name, description, input_description)
    entries: Vec<(FunctionId, String, String, String)>,
}

impl UserDefinedFunctions {
    pub async fn build(user_id: &UserId) -> Result<Self> {
        let rt = Runtime::get_runtime_ref().map_err(|e| anyhow::anyhow!("{e}"))?;
        let funcs = rt.get_user_functions(user_id).await?;

        let entries = funcs
            .iter()
            .map(|f| {
                (
                    f.function_id.clone(),
                    f.function_name.clone(),
                    f.description.clone(),
                    f.function_input_description.clone(),
                )
            })
            .collect();

        Ok(Self { entries })
    }
}

impl Tool for UserDefinedFunctions {
    const NAME: &'static str = "UserDefinedFunctions";
    type Error = std::io::Error;
    type Args = ();
    type Output = String;

    async fn definition(&self, _: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Returns a list of all user-defined functions with their IDs, \
                          names, descriptions, and expected input formats."
                .to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
        }
    }

    async fn call(&self, _: Self::Args) -> std::result::Result<Self::Output, Self::Error> {
        let output = self
            .entries
            .iter()
            .map(|(id, name, desc, inp)| {
                format!(
                    "FunctionName: {name}\nFunctionId: {id}\nDescription: {desc}\nInputDescription: {inp}\n"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        info!("USER_DEFINED_FUNCTIONS: {output}");
        Ok(output)
    }
}

// Execute the user defined tool by the LLM invoketion
#[derive(Serialize, Deserialize)]
pub struct ExecArgs {
    pub func_id: String,
    pub input: String,
}

pub struct ExecuteUserDefinedFunction {
    user_id: String,
}

impl ExecuteUserDefinedFunction {
    pub fn new(user_id: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
        }
    }
}

impl Tool for ExecuteUserDefinedFunction {
    const NAME: &'static str = "ExecuteUserDefinedFunction";
    type Error = std::io::Error;
    type Args = ExecArgs;
    type Output = String;

    async fn definition(&self, _: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Execute a user-defined function by its ID. \
                          The `input` field must follow the format described in the \
                          function's InputDescription result of the tool UserDefinedFunctions."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "func_id": {
                        "type": "string",
                        "description": "The function ID from UserDefinedFunctions from the parameter FunctionId, you MUST give only the ID."
                    },
                    "input": {
                        "type": "string",
                        "description": "The input string matching the function's InputDescription"
                    }
                },
                "required": ["func_id", "input"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> std::result::Result<Self::Output, Self::Error> {
        let uid = self.user_id.clone();
        let fid = args.func_id;
        let input = args.input;

        let rt = Runtime::get_runtime_ref().unwrap();
        let funcs = rt.get_user_functions(&uid).await.unwrap();

        let mut does_contains = false;
        for func_handl in funcs.iter() {
            if func_handl.function_id == fid {
                does_contains = true;
                break;
            }
        }

        if !does_contains {
            return Ok("ERROR: Access Denied. You MUST call `UserDefinedFunctions` first to discover valid function IDs.".to_string());
        }

        let handler = funcs.iter().find(|f| f.function_id == fid).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Agent: function {fid} not found".to_string(),
            )
        })?;

        handler.lambda.run(&input).await.map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Agent: function {fid} not found".to_string(),
            )
        })
    }

    // async fn call(&self, args: Self::Args) -> std::result::Result<Self::Output, Self::Error> {
    //     let closure = LimesAgent::<ollama::CompletionModel>::make_exec_closure(
    //         self.user_id.clone(),
    //         args.func_id.clone(),
    //     );
    //     (closure)(args.input).await.map_err(|e| {
    //         error!("ExecuteFunction tool error:\nfunc_id = {}", args.func_id);
    //         std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    //     })
    // }
}
