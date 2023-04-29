# TODO

## Active Task

- getting conversations to a usable place
  - easily access and create repo-specific and global conversations
  - correct detection of the root of a project

## Short term goals

- read in flags with the clap library
  - use clap to read in a -c | --conversation flag
- only save the conversation if the -c flag was called
- then we can focus on git repository checking
  - Initially impemented, needs polishing
- If no prompt given and no stdin, open up EDITOR for setting the prompt

# Ideas

## Select model by conversation

Possibly have a command to create conversations. We would need to store the history manually
conversations can be repo specific or global


## Longer term goals

- have more conversational styled mode where repeated questions and answers can be given without rerunning comman
- Spawn a per-conversation daemon
- have api for usage as library
  - Possibly like kakoune, where commands can be sent to a "session" with a certain flag
- ability to look through history of conversation. Pretty print into a pager?
- Add conversation creation option asking for initial promt
  - Options for default, a user created template, or custom prompt
  - If template selected give opprotunity to edit and customize it for conversation