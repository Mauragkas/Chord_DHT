use super::*;
use data::*;
use node_state::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Message {
    IAmYourSuccessor { node_id: String },
    IAmYourPredecessor { node_id: String },
    ReqKnownNode { node_id: String },
    ResKnownNode { node_id: String },
    Data { from: String, data: Vec<Data> },
    UpdateNodeState { new_state: NodeState },
    ReqJoin { node_id: String },
    ResJoin { node_id: String, sender_id: String },
    RingIsFull,
    Success { message: String },
    ErrorMessage { error: String },
    Lookup { key: String },
    NodeExists,
    Notify { node_id: String },
    Leave { node_id: String },
    Ping,
    Pong,
}
