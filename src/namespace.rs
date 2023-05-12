use std::{
    cmp::Ordering,
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
pub struct ConversationData {
    pub model: String,
    pub history: Vec<ChatMessage>,
    pub context: Vec<ChatMessage>,
    pub includes: usize,
    pub session_len: usize,
}

pub struct Namespace {
    client: ChatGPT,
    pub name: Option<String>,
    pub data: ConversationData,
}

impl Namespace {
    // store will save the conversation data in the config dir if self.name is not None
    pub fn store(self: &Self, path: &Path) -> Result<()> {
        let mut file = File::create(path)?;
        let contents = serde_json::to_string(&self.data)?;
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
            name,
            data: conv,
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
        // TODO have a way to change the number of includes
        Ok(Namespace {
            client,
            name,
            data: ConversationData {
                model: engine.to_string(),
                history: vec![],
                context: vec![],
                includes: 2,
                session_len: 0,
            },
        })
    }

    async fn send_message_as<S: Into<String>>(
        &mut self,
        role: Role,
        message: S,
        keep: bool,
        extend_session: bool,
    ) -> Result<CompletionResponse> {
        let message = ChatMessage {
            content: message.into(),
            role,
        };

        if !extend_session {
            self.data.session_len = 0;
        }

        // Include both the assistant's response and the user's message for each "includes".
        let includes = (self.data.includes + self.data.session_len) * 2;

        self.data.history.push(message.clone());

        let history_len = if self.data.history.len() <= includes + 1 {
            0
        } else {
            self.data.history.len() - 1 - includes
        };
        let context_len = self.data.context.len();

        self.data
            .context
            .extend_from_slice(&mut self.data.history[history_len..]);

        let response = self.client.send_history(&self.data.context).await?;

        self.data.history.push(response.message().clone());
        let last_user = self.data.context.pop().unwrap();
        self.data.context.truncate(context_len);
        if keep || role == Role::System {
            self.data.context.push(last_user);

            if role == Role::System {
                // Keep system messages at the start
                // TODO configure this to optionally keep system messages at the end. OpenAI says this may be better for 3.5 turbo
                self.data.context.sort_by(|a, b| {
                    if a.role == Role::System {
                        return Ordering::Less;
                    } else if b.role == Role::System {
                        return Ordering::Greater;
                    } else {
                        return Ordering::Equal;
                    }
                });
            }
        }

        if extend_session {
            self.data.session_len += 1;
        }

        Ok(response)
    }

    // send_system_message sends a message as a system message and keeps the system
    // message in the context.
    pub async fn send_system_message<S: Into<String>>(
        &mut self,
        message: S,
        keep: bool,
        extend_session: bool,
    ) -> Result<CompletionResponse> {
        return self
            .send_message_as(Role::System, message, keep, extend_session)
            .await;
    }

    // send_message sends the model the given message with the namespace's context.
    pub async fn send_message<S: Into<String>>(
        &mut self,
        message: S,
        keep: bool,
        extend_session: bool,
    ) -> Result<CompletionResponse> {
        return self
            .send_message_as(Role::User, message, keep, extend_session)
            .await;
    }

    // function to create a ChatGPT conversation using context as the initial history.
    // To return history,
    pub fn create_conversation(&mut self) -> NamespaceConversation {
        NamespaceConversation {
            conversation: Conversation::new_with_history(
                self.client.clone(),
                self.data.context.clone(),
            ),
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

        for _ in 0..self.space.data.context.len() {
            history.remove(0);
        }

        self.space.data.history.append(&mut history);
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
