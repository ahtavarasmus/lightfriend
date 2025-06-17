-- Your SQL goes here

CREATE TABLE message_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    role TEXT NOT NULL,  -- 'user', 'assistant', 'tool', or 'system'
    encrypted_content TEXT NOT NULL,
    tool_name TEXT,  -- Name of the tool if it's a tool response
    tool_call_id TEXT,  -- ID of the tool call if it's a tool response
    created_at INTEGER NOT NULL,  -- Unix timestamp
    conversation_id TEXT NOT NULL,  -- To group messages in the same conversation
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Create an index on user_id and conversation_id for faster lookups
CREATE INDEX idx_message_history_user_conversation 
ON message_history(user_id, conversation_id);

-- Create an index on created_at for chronological queries
CREATE INDEX idx_message_history_created_at 
ON message_history(created_at);
