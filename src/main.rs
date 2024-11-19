#![allow(dead_code)]
mod chord_server;
mod data_misc;
mod hash;
mod macros;
mod node;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chord_server::chrord::*;
use data_misc::*;
use node::node::*;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, thread::sleep, time::Duration};
use tokio::sync::{mpsc, Mutex};

lazy_static::lazy_static! {
    static ref M: usize = dotenv::var("M").unwrap().parse().unwrap();
    static ref PORT: u16 = {
        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1 {
            args[1].parse().unwrap()
        } else {
            dotenv::var("PORT").unwrap().parse().unwrap()
        }
    };
    static ref DEFAULT_CHANNEL_SIZE: usize = dotenv::var("DEFAULT_CHANNEL_SIZE").unwrap().parse().unwrap();
    static ref NUM_OF_NODES: usize = dotenv::var("NUM_OF_NODES").unwrap().parse().unwrap();
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let chord_server = Arc::new(ChordRing::new());
    let chord_server_clone = Arc::clone(&chord_server);

    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _result: (String, u16) = chord_server_clone.run().await.unwrap();
        });
    });

    let mut server_handles = vec![];
    let mut node_ids = vec![];

    for i in 1..=*NUM_OF_NODES {
        let port = *PORT + i as u16;
        let node = Node::new(Some(port));
        let node_clone = node.clone();
        let server_handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                run_server(node_clone).await.unwrap();
            });
        });
        let node_id = {
            let node_state = node.node_state.lock().await;
            node_state.id.clone()
        };
        node_ids.push(node_id);
        server_handles.push(server_handle);
    }

    for node_id in node_ids {
        chord_server.req_known_node(node_id).await;
        sleep(Duration::from_millis(1000));
    }

    for handle in server_handles {
        handle.join().unwrap();
    }
    handle.join().unwrap();

    Ok(())
}
