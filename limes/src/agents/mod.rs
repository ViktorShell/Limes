use anyhow::{Context, Result};
use rig::agent::Agent;
use rig::client::{CompletionClient, Nothing};
use rig::completion::{CompletionModel, Prompt};
use rig::providers::ollama;
use rig::tool::Tool;
