#![allow(unused_must_use)] // this is for the macros to ignore the Result type
mod chord_server;
mod data_misc;
pub mod hash;
mod macros;
mod node;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chord_server::chord::*;
use data_misc::*;
use node::{helper, node::*};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

lazy_static::lazy_static! {
    static ref M: usize = dotenv::var("M").unwrap().parse().unwrap();
    static ref N: usize = dotenv::var("N").unwrap().parse().unwrap();
    static ref IP: String = get_tailscale_ip().unwrap_or_else(|_| String::from("0.0.0.0"));
    static ref PORT: u16 = {
        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1 {
            args[1].parse().unwrap()
        } else {
            dotenv::var("PORT").unwrap().parse().unwrap()
        }
    };
    static ref DEFAULT_CHANNEL_SIZE: usize = dotenv::var("DEFAULT_CHANNEL_SIZE").unwrap().parse().unwrap();
}

fn get_tailscale_ip() -> std::io::Result<String> {
    use std::process::Command;

    let output = Command::new("tailscale").arg("ip").output()?;

    if output.status.success() {
        // Split the output by newlines and take the first line (IPv4 address)
        let ip = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("0.0.0.0")
            .trim()
            .to_string();
        Ok(ip)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to get Tailscale IP",
        ))
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(2).map(|s| s.as_str()) {
        Some("chord") => {
            let chord_server = Arc::new(ChordRing::new());
            let chord_server_clone = Arc::clone(&chord_server);

            let handle = std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let _result: (String, u16) = chord_server_clone.run().await.unwrap();
                });
            });

            handle.join().unwrap();
        }
        Some("node") => {
            let node_port = args[3].parse::<u16>().unwrap_or(*PORT);
            let node = Node::new(Some(node_port));
            let node_clone = node.clone();

            let server_handle = std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    helper::run_server(node_clone).await.unwrap();
                    // println!("Starting node {}:{}", *IP, node_port);
                });
            });

            let mut retries = 5;
            while retries > 0 {
                // match node.req_known_node(format!("{}:{}", *IP, *PORT)).await {
                match node.req_known_node(format!("{}:{}", args[4], *PORT)).await {
                    Ok(()) => break,
                    Err(_) => {
                        retries -= 1;
                        if retries > 0 {
                            log_message!(node, "Retrying to connect to known node");
                            std::thread::sleep(std::time::Duration::from_secs(1));
                        } else {
                            println!("Failed to connect to known node");
                            std::process::exit(1);
                        }
                    }
                }
            }

            server_handle.join().unwrap();
        }
        _ => {
            println!("Please specify either 'chord' or 'node' as the second argument");
            std::process::exit(1);
        }
    }

    Ok(())
}
