use anyhow::{Context, Result};
use rig::agent::Agent;
use rig::client::{CompletionClient, Nothing};
use rig::completion::ToolDefinition;
use rig::completion::{CompletionModel, Prompt};
use rig::providers::ollama;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use std::future::Future;
use std::pin::Pin;

use crate::runtime::types::*;
use crate::runtime::Runtime;

pub struct LimesAgent<M: CompletionModel> {
    agent: Agent<M>,
}

impl LimesAgent<ollama::CompletionModel> {
    pub async fn new_agent(host: &str, model_name: &str, user_id: UserId) -> Result<Self> {
        // Init the Agent Builder
        let client = ollama::Client::builder()
            .base_url(host)
            .api_key(Nothing)
            .build()
            .context("Failed the client build")?;

        let tool_list_user_funcs = UserDefinedFunctions::new(
            &user_id,
            r#"
This function give the list of all aviable functions offered by the user following the format: 
FunctionId: {value}
FunctionName: {value}
FunctionDescription: {value}
FunctionInputDescription: {value}
You can use the FunctionId value to call the user defined functions using the ExecuteUserDefinedFunction.
        "#
            .to_string(),
        )
        .await?;
        let tool_exec_user_funcs = ExecuteFunction::new(&user_id, r#"
This function allow the use of user defined tools, you must give the correct function_id and input value as described by the function input type properties.
        "#.to_string()).await;

        let agent = client.agent(model_name).preamble(
            r#"
You are a helpful assistant with access to a range of tool defined by the user by using the ListUserDefinedFunctions tool.
You must follow the how the input must be formatted by the functions specification and user specification to execute the functions by using the ExecuteUserDefinedFunction.
You Must use the tool only a single time if not defined in a different way by the user.
"#,
        )
            .tool(tool_exec_user_funcs)
            .tool(tool_list_user_funcs)
            .build();

        Ok(Self { agent })
    }

    pub async fn invoke_agent(&self, _prompt: String) -> anyhow::Result<String> {
        self.agent
            .prompt(_prompt)
            .await
            .with_context(|| "Agent: Failed to get the prompt")
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

// ====================================
// List user defined functions
// ====================================

type FunctionName = String;
type FunctionDescription = String;
type FunctionInputDescription = String;
pub struct UserDefinedFunctions {
    user_id: UserId,
    description: String,
    func_list: Vec<(
        FunctionId,
        FunctionName,
        FunctionDescription,
        FunctionInputDescription,
    )>,
}

impl UserDefinedFunctions {
    pub async fn new(user_id: &UserId, description: String) -> Result<Self> {
        let mut func_list: Vec<(
            FunctionId,
            FunctionName,
            FunctionDescription,
            FunctionInputDescription,
        )> = Vec::new();
        let rt = Runtime::get_runtime_ref()?;
        let user_functions = rt.get_user_functions(user_id).await?;
        for func_handl in user_functions.iter() {
            let func_metadata = (
                func_handl.function_id.clone(),
                func_handl.function_name.clone(),
                func_handl.description.clone(),
                func_handl.function_input_description.clone(),
            );
            func_list.push(func_metadata);
        }

        Ok(Self {
            user_id: user_id.clone(),
            description,
            func_list,
        })
    }
}

impl Tool for UserDefinedFunctions {
    const NAME: &'static str = "ListUserDefinedFunctions";
    type Error = std::io::Error;
    type Args = ();
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: self.description.clone(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(&self, _: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut output = String::new();
        for (func_id, func_name, func_desc, func_inp_desc) in self.func_list.iter() {
            output.push_str(&format!("FunctionId: {{{}}}\n FunctionName: {{{}}}\nFunctionDescription: {{{}}}\nFunctionInputDescription: {{{}}}\n\n",
                func_id,
                func_name,
                func_desc,
                func_inp_desc,
            ));
        }
        Ok(output)
    }
}

// ====================================
// Execute the selected function
// ====================================

#[derive(Serialize, Deserialize)]
pub struct ExecuteFunctionArgs {
    func_id: String,
    input: String,
}

pub struct ExecuteFunction {
    user_id: String,
    description: String,
}

impl ExecuteFunction {
    pub async fn new(user_id: &String, description: String) -> Self {
        Self {
            user_id: user_id.clone(),
            description,
        }
    }
}

impl Tool for ExecuteFunction {
    const NAME: &'static str = "ExecuteUserDefinedFunction";
    type Error = std::io::Error;
    type Args = ExecuteFunctionArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: self.description.clone(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "func_id": {
                        "type": "string",
                        "description": "The function id of the function defined by the ListUserDefinedFunctions"
                    },
                    "input": {
                        "type": "string",
                        "description": "The input string defined by the FunctionInputDescription of the tool ListUserDefinedFunctions",
                    }
                },
                "required": ["function_id", "input"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let user_id = self.user_id.clone();
        let func_id = args.func_id.clone();
        let func_closure = LimesAgent::get_func_future(user_id, func_id);
        let result = (func_closure)(args.input)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        result
    }
}

// ====================================
// ====================================

#[cfg(test)]
mod test {}
