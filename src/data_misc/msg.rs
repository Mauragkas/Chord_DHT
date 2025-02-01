use super::*;
use data::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Message {
    IAmYourSuccessor {
        node_id: String,
    },
    IAmYourPredecessor {
        node_id: String,
    },
    ReqKnownNode {
        node_id: String,
    },
    ResKnownNode {
        node_id: String,
    },
    Data {
        from: String,
        data: Vec<Data>,
    },
    ReqJoin {
        node_id: String,
    },
    ResJoin {
        node_id: String,
        sender_id: String,
    },
    RingIsFull,
    Success {
        message: String,
    },
    ErrorMessage {
        error: String,
    },
    LookupReq {
        key: String,
        hops: usize,
    },
    LookupRes {
        key: String,
        hops: usize,
        data: Option<Vec<Data>>,
    },
    NodeExists,
    Notify {
        node_id: String,
    },
    Leave {
        node_id: String,
    },
    Joined {
        node_id: String,
    },
    ReqFinger {
        from: String,
        index: usize,
    },
    ResFinger {
        node_id: String,
        index: usize,
    },
    CheckNode {
        node_id: String,
    },
    ReqSuccessor {
        from: String,
    },
    ResSuccessor {
        from: String,
        successor: String,
    },
    Ping,
    Pong,
    Kys,
}
