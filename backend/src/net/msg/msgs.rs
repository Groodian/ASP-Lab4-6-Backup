use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GlobalChatMessage {
    pub message: String,
}
