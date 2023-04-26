use std::env::args;

use chatgpt::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Getting the API key here
    let mut arg_it = args();
    let key = arg_it.nth(1).unwrap();
    let prompt = arg_it.collect::<Vec<String>>().join(" ");

    // Creating a new ChatGPT client.
    // Note that it requires an API key, and uses
    // tokens from your OpenAI API account balance.
    let client = ChatGPT::new(key)?;

    // // Sending a message and getting the completion
    let response = client
        .send_message(prompt)
        .await?;

    println!("Response: {}", response.message().content);

    Ok(())
}