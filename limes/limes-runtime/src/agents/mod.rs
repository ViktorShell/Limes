use anyhow::{Context, Result};
use rig::agent::Agent;
use rig::client::{CompletionClient, Nothing};
use rig::completion::{CompletionModel, Prompt, ToolDefinition};
use rig::providers::ollama;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info};

use std::future::Future;
use std::pin::Pin;

use crate::runtime::{
    types::{FunctionId, UserId},
    Runtime,
};

// ─────────────────────────────────────────────────────────────────────────────
//  LimesAgent — LLM-backed agent with access to user-defined tools
// ─────────────────────────────────────────────────────────────────────────────

pub struct LimesAgent<M: CompletionModel> {
    agent: Agent<M>,
}

impl LimesAgent<ollama::CompletionModel> {
    /// Build an agent connected to Ollama with the user's loaded functions as tools.
    pub async fn new_agent(host: &str, model: &str, user_id: UserId) -> Result<Self> {
        let client = ollama::Client::builder()
            .base_url(host)
            .api_key(Nothing)
            .build()
            .context("LimesAgent: failed to build Ollama client")?;

        let list_tool = UserDefinedFunctions::build(&user_id).await?;
        let exec_tool = ExecuteFunction::new(&user_id);

        let agent = client
            .agent(model)
            .preamble(
                "You are a helpful assistant. You have access to user-defined tools. \
                 Use `ListUserDefinedFunctions` to discover available functions, \
                 then use `ExecuteUserDefinedFunction` to call them with the correct input. \
                 Follow the input format described in each function's description exactly. \
                 Use each tool at most once unless the user explicitly asks for repetition.",
            )
            .tool(list_tool)
            .tool(exec_tool)
            .build();

        info!(host, model, user_id, "LimesAgent initialized");
        Ok(Self { agent })
    }

    pub async fn invoke_agent(&self, prompt: String) -> Result<String> {
        debug!(prompt_len = prompt.len(), "Agent prompt received");
        self.agent
            .prompt(prompt)
            .await
            .context("LimesAgent: prompt call failed")
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

// ─────────────────────────────────────────────────────────────────────────────
//  Tool: ListUserDefinedFunctions
// ─────────────────────────────────────────────────────────────────────────────

/// Lists all functions loaded for a user so the LLM can discover them.
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
    const NAME: &'static str = "ListUserDefinedFunctions";
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
                    "FunctionId: {id}\nFunctionName: {name}\nDescription: {desc}\nInputDescription: {inp}\n"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(output)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tool: ExecuteUserDefinedFunction
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct ExecArgs {
    pub func_id: String,
    pub input: String,
}

pub struct ExecuteFunction {
    user_id: String,
}

impl ExecuteFunction {
    pub fn new(user_id: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
        }
    }
}

impl Tool for ExecuteFunction {
    const NAME: &'static str = "ExecuteUserDefinedFunction";
    type Error = std::io::Error;
    type Args = ExecArgs;
    type Output = String;

    async fn definition(&self, _: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Execute a user-defined function by its ID. \
                          The `input` field must follow the format described in the \
                          function's InputDescription."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "func_id": {
                        "type": "string",
                        "description": "The function ID from ListUserDefinedFunctions"
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
        let closure = LimesAgent::<ollama::CompletionModel>::make_exec_closure(
            self.user_id.clone(),
            args.func_id.clone(),
        );
        (closure)(args.input).await.map_err(|e| {
            error!(error = %e, func_id = args.func_id, "ExecuteFunction tool error");
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })
    }
}
