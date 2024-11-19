#![allow(unused)]
use super::*;
use data::*;
use finger_table::*;
use hash::hash;
use msg::*;
use node_state::*;

pub mod finger_table;
pub mod node;

const HTML_PATH: &str = "./src/node/client/template.html";

macro_rules! log_and_push {
    ($logs:expr, $fmt:expr, $($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        println!($fmt, $($arg)*);
        $logs.lock().await.push(format!(
            "{}: <span class=\"font-semibold\">{}</span>",
            chrono::Utc::now(),
            format!($fmt, $($arg)*)
        ));
    }};
}
