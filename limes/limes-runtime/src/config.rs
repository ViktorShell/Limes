use std::{
    sync::{OnceLock, RwLock},
    time::Duration,
};

pub static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();

pub struct Config {
    pub ollama_url: String,
    pub model: String,
    pub request_timeout: Duration,
}

impl Config {
    pub fn init(config: Config) {
        if CONFIG.set(RwLock::new(config)).is_err() {
            eprintln!("Config was not initialized")
        }
    }

    pub fn get() -> &'static RwLock<Config> {
        CONFIG
            .get()
            .expect("You must initialize the config with .init() before use")
    }

    pub fn get_ollama_url(&self) -> String {
        self.ollama_url.clone()
    }

    pub fn get_model(&self) -> String {
        self.model.clone()
    }
}
