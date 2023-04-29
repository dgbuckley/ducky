# TODO

## Active Task

- repl like mode
- launch with -r|--repl

## Short term goals

- If no prompt given and no stdin, open up EDITOR for setting the prompt
  - if stdin and a prompt, send stdin as "code".
- have more conversational styled mode where repeated questions and answers can be given without rerunning command
- Format the recieved markdown text in the terminal

# Ideas

## Select model by conversation

Possibly have a command to create conversations. We would need to store the history manually
conversations can be repo specific or global


## Longer term goals

- Beutify cli
  - color formatting code
  - paging when output is larger than the terminal
  - better selecting, maybe with: https://docs.rs/dialoguer/latest/dialoguer/
- Spawn a per-conversation daemon
- have api for usage as library
  - Possibly like kakoune, where commands can be sent to a "session" with a certain flag
- ability to look through history of conversation. Pretty print into a pager?
- Add conversation creation option asking for initial promt
  - Options for default, a user created template, or custom prompt
  - If template selected give opprotunity to edit and customize it for conversation