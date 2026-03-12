use anyhow::{Context, Result};
use rig::agent::Agent;
use rig::client::{CompletionClient, Nothing};
use rig::completion::ToolDefinition;
use rig::completion::{CompletionModel, Prompt};
use rig::providers::ollama;
use rig::tool::{Tool, ToolDyn, ToolError};
use rig::wasm_compat::WasmBoxedFuture;
use serde::Deserialize;
use serde_json::json;

use std::future::Future;
use std::pin::Pin;

use crate::runtime::types::*;
use crate::runtime::Runtime;

// TODO: Implement a Factory Pattern for the different usable agents

pub enum AviableModels {
    Llama,
}

impl AviableModels {
    fn value(&self) -> String {
        match *self {
            AviableModels::Llama => String::from("llama3.2:3b"),
        }
    }
}

pub struct LimesAgent<M: CompletionModel> {
    model_name: AviableModels,
    agent: Agent<M>,
    // user_id: UserId,
    // function_id: FunctionId,
}

impl LimesAgent<ollama::CompletionModel> {
    pub async fn new_agent(
        host: &str,
        model_name: &str,
        user_id: UserId,
        func_id: FunctionId,
    ) -> Result<Self> {
        let client = ollama::Client::builder()
            .base_url(host)
            .api_key(Nothing)
            .build()
            .context("Failed the client build")?;

        let mut agent_builder = client.agent(model_name).preamble(
            "You are a helpful assistant with access to a range of tool defined by the user. You must follow the how the input must be formatted by the functions specification and user specification",
        );

        let rt = Runtime::get_runtime_ref().unwrap();
        let func_vec = rt.get_user_functions(&user_id).await?;

        for t in func_vec.iter() {
            let f_tool = t;
            if func_id == f_tool.function_id {
                continue;
            }
            let tool_func = LimesAgent::get_func_future(user_id, func_id);

            let lf_tool = LimesFunctionTool {
                tool_name: f_tool.function_name.clone(),
                tool_description: f_tool.description.clone(),
                tool_input_description: f_tool.function_input_description.clone(),
                tool_func,
            };

            agent_builder = agent_builder.tool(lf_tool);
        }
        let agent = agent_builder.build();

        todo!();
    }

    pub fn get_func_future(user_id: UserId, function_id: FunctionId) -> FunctionClosure {
        Box::new(move |args_input: String| {
            let u_id = user_id.clone();
            let f_id = function_id.clone();

            Box::pin(async move {
                let rt = Runtime::get_runtime_ref()?;
                let user_functions = rt.get_user_functions(&u_id).await?;
                let func_wrap = user_functions
                    .iter()
                    .find(|f| f.function_id == f_id)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Agent: Failed to find the function with id: {}", f_id)
                    })?;
                let result = func_wrap.lambda.run(&args_input).await?;
                Ok(result)
            })
        })
    }
}

type PinnedFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type FunctionClosure = Box<dyn Fn(String) -> PinnedFuture<anyhow::Result<String>> + Send + Sync>;

pub struct LimesFunctionTool {
    tool_name: String,
    tool_description: String,
    tool_input_description: String,
    tool_func: FunctionClosure,
}

impl LimesFunctionTool {
    pub fn new(
        tool_name: String,
        tool_description: String,
        tool_input_description: String,
        tool_func: FunctionClosure,
    ) -> Self {
        Self {
            tool_name,
            tool_description,
            tool_input_description,
            tool_func,
        }
    }
}

#[derive(Deserialize)]
pub struct InputField {
    input: String,
}

impl Tool for LimesFunctionTool {
    const NAME: &'static str = "LimesFunction";
    type Error = std::io::Error;
    type Args = InputField;
    type Output = String;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.tool_name.clone(),
            description: self.tool_description.clone(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "definition": self.tool_input_description
                    }
                },
                "required": ["input"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Use the lambda tool
        let input = args.input;
        let result = (self.tool_func)(input)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        Ok(result)
    }
}
