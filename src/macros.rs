#[macro_export]
macro_rules! log_message {
    ($app_state:expr, $msg:expr $(, $arg:expr)*) => {
        #[cfg(debug_assertions)]
        println!($msg $(, $arg)*);

        let now = chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(2 * 3600).unwrap()).format("%Y-%m-%d %H:%M:%S");

        $app_state.logs.lock().await.push(format!(
            "[{}] <span class=\"font-semibold\">{}</span>",
            now,
            format!($msg $(, $arg)*)
        ));
    };
}

#[macro_export]
macro_rules! send_post_request {
    ($url:expr, $message:expr) => {
        reqwest::Client::new()
            .post($url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&$message).unwrap())
            .send()
            .await
            .unwrap();
    };
}
