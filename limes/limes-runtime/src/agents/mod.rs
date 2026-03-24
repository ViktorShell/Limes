use anyhow::{Context, Result};
use log::*;
use rig::agent::Agent;
use rig::client::{CompletionClient, Nothing};
use rig::completion::{CompletionModel, Prompt, ToolDefinition};
use rig::providers::ollama;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use std::future::Future;
use std::pin::Pin;

use crate::runtime::{
    types::{FunctionId, UserId},
    Runtime,
};


/// Limes Agent
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

        let list_tool = ListFunctions::build(&user_id).await?;
        let exec_tool = ExecuteFunction::new(&user_id);

        let system_prompt = 
            "You are a helpful assistant with access to a dynamic function registry. \
             When asked to perform a calculation or run a function:\n\
             1. ALWAYS call list_functions first to see what is available and note the exact function_id values.\n\
             2. Then call execute_function using the exact function_id from step 1 and an input object \
                matching that function's input_structure.\n\
             3. Return the result clearly to the user.\n\
             Never guess function_id values — always retrieve them from list_functions.";

        let agent = client
            .agent(model)
            .preamble(system_prompt)
            .tool(list_tool)
            .tool(exec_tool)
            .build();

        info!(
            r#"
LimesAgent initialized:
>> host: {host}
>> model: {model}
>> user_id: {user_id}
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
            .context("LimesAgent: prompt call failed")?;

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

/// Metadata for the model to rappresent the functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionMeta {
    pub function_name: String,
    pub function_id: String,
    pub function_description: String,
    // NOTE: Foundamental for the model understandin or it will allucinate
    pub input_structure: Value,
}

/// Input for the list_functions
#[derive(Serialize, Deserialize)]
pub struct ListFunctionsInput {}

/// List of the user defined functions
#[derive(Serialize, Deserialize)]
pub struct ListFunctionsOutput {
    functions: Vec<FunctionMeta>,
}

// Gives the list of all user defined tools to the LLM
pub struct ListFunctions {
    functions: Vec<FunctionMeta>,
}

impl ListFunctions {
    pub async fn build(user_id: &UserId) -> Result<Self> {
        let rt = Runtime::get_runtime_ref().map_err(|e| anyhow::anyhow!("{e}"))?;
        let funcs = rt.get_user_functions(user_id).await?;

        let functions = funcs
            .iter()
            .map(|f| FunctionMeta {
                function_name: f.function_name.clone(),
                function_id: f.function_id.clone(),
                function_description: f.description.clone(),
                // NOTE: Foundamental that the input structure is described as a json
                input_structure: json!(f.function_input_description.clone()),
            })
            .collect();

        Ok(Self { functions })
    }
}

impl Tool for ListFunctions {
    const NAME: &'static str = "list_functions";
    type Error = std::io::Error;
    type Args = ListFunctionsInput;
    type Output = ListFunctionsOutput;

    async fn definition(&self, _: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Returns the list of all available user-defined functions. \
                          Each entry contains function_name, function_id, and input_structure (JSON Schema). \
                          You MUST call this tool first to discover which functions exist and what their \
                          function_id values are before calling execute_function.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": [],
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let functions = self.functions.clone();
        Ok(ListFunctionsOutput { functions })
    }
}

/// Tool Executor
pub struct ExecuteFunction {
    user_id: String,
}

// Execute the user defined tool by the LLM invoketion
#[derive(Serialize, Deserialize)]
pub struct ExecuteFunctionInput {
    pub function_id: String,
    /// NOTE: Important, MUST match the input_structure schema for that function
    pub input: Value,
}

#[derive(Serialize, Deserialize)]
pub struct ExecuteFunctionOutput {
    result: Value,
}

impl ExecuteFunction {
    pub fn new(user_id: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
        }
    }
}

impl Tool for ExecuteFunction {
    const NAME: &'static str = "execute_function";
    type Error = std::io::Error;
    type Args = ExecuteFunctionInput;
    type Output = ExecuteFunctionOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Executes a user-defined function. \
                          You must provide the exact function_id returned by list_functions \
                          and an input object matching that function's input_structure schema. \
                          Returns the result of the function."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "function_id": {
                        "type": "string",
                        "description": "The function_id exactly as returned by list_functions"
                    },
                    "input": {
                        "type": "object",
                        "description": "Input parameters matching the function's input_structure schema"
                    }
                },
                "required": ["function_id", "input"]
            }),
        }
    }

    /// NOTE: There are a bit of conversions from String to JSON and to so on.
    /// But for now is not a big deal.
    async fn call(&self, args: Self::Args) -> std::result::Result<Self::Output, Self::Error> {
        let uid = self.user_id.clone();
        let fid = args.function_id;
        let input = args.input;

        let rt = Runtime::get_runtime_ref().unwrap();
        let funcs = rt.get_user_functions(&uid).await.unwrap();

        let handler = funcs.iter().find(|f| f.function_id == fid).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Agent: function {fid} not found".to_string(),
            )
        })?;

        let input_serialized = input.to_string();
        let result = handler.lambda.run(&input_serialized).await.map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Agent: function {fid} not found".to_string(),
            )
        })?;

        Ok(ExecuteFunctionOutput {
            result: json!(result),
        })
    }
}
