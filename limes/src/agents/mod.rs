use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, LlamaModel},
    sampling::LlamaSampler,
};

use log::info;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};

pub struct AgentSession {}

pub struct AgentEngine {
    backend: LlamaBackend,
    model: LlamaModel,
    user_list: HashMap<String, (i32, mpsc::Receiver<String>)>,
}

pub struct AgentBuilder {}

// NOTE: Single message send by the user to the LlamaEngine
pub struct GenerationRequest {
    pub prompt: String,
    pub reply_channel: mpsc::Sender<String>,
}

// NOTE: State of a single chat
pub struct ActiveSequence {
    seq_id: i32,
    n_cur: i32,
    sampler: LlamaSampler,
    reply_channel: mpsc::Sender<String>,
}

pub struct AgentEngines {
    backend: Arc<LlamaBackend>,
    model: Arc<LlamaModel>,
}

impl AgentEngines {
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let backend = Arc::new(LlamaBackend::init()?);
        // backend params: like nthreads gpus ...
        let params = LlamaModelParams::default();
        if !path.as_ref().exists() {
            anyhow::bail!("Model path {} not found", path.as_ref().to_string_lossy());
        }
        let model = Arc::new(LlamaModel::load_from_file(&backend, path, &params)?);
        info!("AgentEngine initialized");
        Ok(Self { backend, model })
    }
}
