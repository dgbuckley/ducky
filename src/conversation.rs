use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use chatgpt::prelude::{ChatGPT, ChatGPTEngine, Conversation, ModelConfigurationBuilder};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ConversationData {
    model: String,
    history: Vec<chatgpt::types::ChatMessage>,
}

pub struct State {
    pub conversation: Conversation,
    model: ChatGPTEngine,
    name: Option<String>,
}

impl State {
    // store will save the conversation data in the config dir if self.name is not None
    pub fn store(self: &Self) -> Result<()> {
        println!("{:?}", self.name);
        match &self.name {
            Some(name) => {
                let path = config_path(name)?;

                let mut file = File::create(path)?;
                let conv = ConversationData {
                    model: self.model.to_string(),
                    history: self.conversation.history.clone(),
                };
                let contents = serde_json::to_string(&conv)?;
                file.write_all(contents.as_bytes())?;
            }
            _ => (),
        }
        Ok(())
    }

    // load will read in an existing conversation
    pub fn load_from(path: &Path, name: Option<String>, key: &str) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let conv: ConversationData = serde_json::from_str(&contents)?;

        let model = engine_from_str(&conv.model)?;
        let client = ChatGPT::new_with_config(
            key,
            ModelConfigurationBuilder::default()
                .engine(model)
                .build()
                .unwrap(),
        )?;

        let conversation = Conversation::new_with_history(client, conv.history);

        Ok(State {
            conversation,
            model,
            name,
        })
    }

    // create a new state and initialize the gpt client
    pub fn create(name: Option<String>, engine: &str, key: &str) -> Result<Self> {
        let model = engine_from_str(engine)?;
        let client = ChatGPT::new_with_config(
            key,
            ModelConfigurationBuilder::default()
                .engine(model)
                .build()
                .unwrap(),
        )?;

        // TODO support a first message
        let conversation = client.new_conversation();
        // let conversation = Conversation::new(client, first_message);

        Ok(State {
            conversation,
            model,
            name,
        })
    }
}

fn engine_from_str(s: &str) -> Result<ChatGPTEngine> {
    match s {
        "gpt-3.5-turbo" => Ok(ChatGPTEngine::Gpt35Turbo),
        "gpt-3.5-turbo-0301" => Ok(ChatGPTEngine::Gpt35Turbo_0301),
        "gpt-4" => Ok(ChatGPTEngine::Gpt4),
        "gpt-4-32k" => Ok(ChatGPTEngine::Gpt4_32k),
        "gpt-4-0314" => Ok(ChatGPTEngine::Gpt4_0314),
        "gpt-4-32k-0314" => Ok(ChatGPTEngine::Gpt4_32k_0314),
        custom => Err(anyhow!("Invalid model: {}", custom)),
        // custom => Ok(ChatGPTEngine::Custom(custom.clone())),
    }
}

/// Returns the config path and ensures the config
/// directory exists.
pub fn config_path(name: &str) -> Result<PathBuf> {
    let mut config_file_path = match BaseDirs::new() {
        Some(base_dirs) => {
            let config_dir_base = base_dirs.config_dir();
            let mut config_dir = PathBuf::from(config_dir_base);
            config_dir.push("ducky");

            if !config_dir.exists() {
                match std::fs::create_dir_all(&config_dir) {
                    Ok(_) => config_dir,
                    Err(e) => return Err(anyhow!("{}", e)),
                }
            } else {
                config_dir
            }
        }
        None => return Err(anyhow!("Unable to get config directory")),
    };

    config_file_path.push(name.to_owned());
    config_file_path.set_extension("json");

    Ok(config_file_path)
}
