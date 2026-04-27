use anyhow::{Context, Result};
use log::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;

// --- Tool Definitions ---

#[derive(Serialize, Deserialize, Debug)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Parameters,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Parameters {
    #[serde(rename = "type", default = "default_param_type")]
    pub param_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    pub properties: HashMap<String, Property>,
}

fn default_param_type() -> String {
    "object".into()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Property {
    #[serde(rename = "type")]
    pub prop_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

pub trait LimesTool: Debug {
    fn name(&self) -> String;
    fn description(&self) -> String;

    // Provide the parameters and requirements as a raw JSON string
    fn parameters_json(&self) -> String;

    // Parses the JSON string into the Parameters struct automatically
    fn get_definition(&self) -> anyhow::Result<Tool> {
        let parameters: Parameters = serde_json::from_str(&self.parameters_json())?;

        Ok(Tool {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: self.name(),
                description: self.description(),
                parameters,
            },
        })
    }

    fn execute(&self, arguments: &str) -> anyhow::Result<String>;
}

// --- Message & API Models ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionCall {
    index: u32,
    name: String,
    arguments: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    #[serde(rename = "type", default = "default_tool_call_type")]
    tool_call_type: String,
    function: FunctionCall,
}

fn default_tool_call_type() -> String {
    "function".into()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "role")]
#[serde(rename_all = "lowercase")]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCall>>,
    },
    Tool {
        tool_name: String,
        content: String,
    },
}

// --- Ollama Response & Request Handlers ---

#[derive(Serialize, Debug)]
struct OllamaRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
    tools: &'a [Tool],
}

#[derive(Deserialize, Debug)]
pub struct OllamaResponse {
    pub message: Message,
    pub done: bool,
    // NOTE: Add other value...
}

// --- Agent Implementation ---
#[derive(Debug)]
pub struct LimesAgent {
    pub url: String,
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<Box<dyn LimesTool>>,
    pub tool_definitions: Vec<Tool>,
    client: reqwest::Client,
}

impl LimesAgent {
    pub async fn new(url: String, model: String, preamble: String) -> anyhow::Result<Self> {
        let compl_url = format!("{}/api/chat", url);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        Ok(Self {
            url: compl_url,
            model,
            messages: vec![Message::System { content: preamble }],
            tools: Vec::new(),
            tool_definitions: Vec::new(),
            client,
        })
    }

    pub fn add_tool(&mut self, tool: Box<dyn LimesTool>) -> anyhow::Result<()> {
        let def = tool.get_definition().map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse tool definition for '{}': {}",
                tool.name(),
                e
            )
        })?;

        self.tool_definitions.push(def);
        self.tools.push(tool);
        Ok(())
    }

    pub async fn invoke_agent(&mut self, prompt: &str) -> anyhow::Result<String> {
        self.messages.push(Message::User {
            content: prompt.to_string(),
        });

        loop {
            // 1. Prepare payload with Tool Definitions
            let payload = OllamaRequest {
                model: &self.model,
                messages: &self.messages,
                stream: false,
                tools: &self.tool_definitions,
            };

            // 2. Convert to string json
            let json_body = serde_json::to_string(&payload)?;

            // 3. Send the request
            let response: OllamaResponse = serde_json::from_str(
                &self
                    .client
                    .post(&self.url)
                    .header("Content-Type", "application/json")
                    .body(json_body)
                    .send()
                    .await?
                    .text()
                    .await?,
            )?;

            let assistant_msg = response.message.clone();
            self.messages.push(assistant_msg.clone());

            // Check if model wants to use tools
            if let Message::Assistant {
                tool_calls: Some(calls),
                ..
            } = assistant_msg
            {
                if calls.is_empty() {
                    return Ok(self.extract_content());
                }

                for call in calls {
                    let tool_name = &call.function.name;
                    let arguments = call.function.arguments.to_string();

                    let result =
                        if let Some(tool) = self.tools.iter().find(|t| t.name() == *tool_name) {
                            match tool.execute(&arguments) {
                                Ok(res) => res,
                                Err(e) => format!("Error executing tool: {}", e),
                            }
                        } else {
                            format!("Error: Tool '{}' not found", tool_name)
                        };

                    self.messages.push(Message::Tool {
                        tool_name: tool_name.clone(),
                        content: result,
                    });
                }

                // After executing tools and adding results, we loop back to let the LLM see the results
                continue;
            } else {
                // Final answer provided
                return Ok(self.extract_content());
            }
        }
    }

    fn extract_content(&self) -> String {
        for msg in self.messages.iter().rev() {
            if let Message::Assistant {
                content: Some(c), ..
            } = msg
            {
                return c.clone();
            }
        }
        "No final response content.".to_string()
    }
}
