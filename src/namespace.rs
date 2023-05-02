use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

use anyhow::{anyhow, Result};
use chatgpt::{
    prelude::{ChatGPT, ChatGPTEngine, Conversation, ModelConfigurationBuilder},
    types::{ChatMessage, CompletionResponse, Role},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ConversationData {
    model: String,
    history: Vec<ChatMessage>,
    context: Vec<ChatMessage>,
}

pub struct Namespace {
    client: ChatGPT,
    model: ChatGPTEngine,
    pub name: Option<String>,
    pub history: Vec<ChatMessage>,
    pub context: Vec<ChatMessage>,
}

impl Namespace {
    // store will save the conversation data in the config dir if self.name is not None
    pub fn store(self: &Self, path: &Path) -> Result<()> {
        let mut file = File::create(path)?;
        let conv = ConversationData {
            model: self.model.to_string(),
            history: self.history.clone(),
            context: self.context.clone(),
        };
        let contents = serde_json::to_string(&conv)?;
        file.write_all(contents.as_bytes())?;

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

        Ok(Namespace {
            client,
            model,
            name,
            history: conv.history,
            context: conv.context,
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

        // TODO support a first system message

        Ok(Namespace {
            client,
            model,
            name,
            history: vec![],
            context: vec![],
        })
    }

    // send_message sends the model the given message with the namespace's context.
    pub async fn send_message<S: Into<String>>(
        &mut self,
        message: S,
        keep: bool,
    ) -> Result<CompletionResponse> {
        let message = ChatMessage {
            content: message.into(),
            role: Role::User,
        };

        self.history.push(message.clone());
        self.context.push(message.clone());

        let response = self.client.send_history(&self.context).await?;

        self.history.push(response.message().clone());
        if !keep {
            self.context.pop();
        }

        Ok(response)
    }

    // function to create a ChatGPT conversation using context as the initial history.
    // To return history,
    pub fn create_conversation(&mut self) -> NamespaceConversation {
        NamespaceConversation {
            conversation: Conversation::new_with_history(self.client.clone(), self.context.clone()),
            space: self,
        }
    }
}

pub struct NamespaceConversation<'a> {
    conversation: Conversation,
    space: &'a mut Namespace,
}

impl<'a> NamespaceConversation<'a> {
    pub async fn send_message<S: Into<String>>(
        &mut self,
        message: S,
    ) -> Result<CompletionResponse> {
        let r = self.conversation.send_message(message).await?;
        Ok(r)
    }
}

impl<'a> Drop for NamespaceConversation<'a> {
    fn drop(&mut self) {
        let mut history = self.conversation.history.to_owned();

        for _ in 0..self.space.context.len() {
            history.remove(0);
        }

        self.space.history.append(&mut history);
    }
}

fn engine_from_str(s: &str) -> Result<ChatGPTEngine> {
    match s {
        "gpt-3.5-turbo" => Ok(ChatGPTEngine::Gpt35Turbo),
        "gpt-4" => Ok(ChatGPTEngine::Gpt4),
        "gpt-4-32k" => Ok(ChatGPTEngine::Gpt4_32k),
        "gpt-4-0314" => Ok(ChatGPTEngine::Gpt4_0314),
        "gpt-4-32k-0314" => Ok(ChatGPTEngine::Gpt4_32k_0314),

        custom => Err(anyhow!("Invalid model: {}", custom)),
        // custom => Ok(ChatGPTEngine::Custom(custom.clone())),
    }
}
