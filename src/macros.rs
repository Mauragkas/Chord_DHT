#[macro_export]
macro_rules! log_message {
    ($app_state:expr, $msg:expr $(, $arg:expr)*) => {{
        let now = {
            #[cfg(debug_assertions)]
            {
                chrono::Utc::now()
                    .with_timezone(&chrono::FixedOffset::east_opt(2 * 3600).unwrap())
                    .format("%Y-%m-%d %H:%M:%S%.6f")
            }
            #[cfg(not(debug_assertions))]
            {
                chrono::Utc::now()
                    .with_timezone(&chrono::FixedOffset::east_opt(2 * 3600).unwrap())
                    .format("%Y-%m-%d %H:%M:%S")
            }
        };

        #[cfg(debug_assertions)]
        println!("[{}] {}", now, format!($msg $(, $arg)*));

        $app_state.logs.lock().await.push(format!(
            "[{}] <span class=\"font-semibold\">{}</span>",
            now,
            format!($msg $(, $arg)*)
        ))
    }};
}

#[macro_export]
macro_rules! send_post_request {
    ($url:expr, $message:expr) => {
        send_post_request!($url, $message, 3) // Default to 3 retries
    };
    ($url:expr, $message:expr, $max_retries:expr) => {{
        let client = reqwest::Client::new();
        let json_body = serde_json::to_string(&$message).unwrap();
        let mut attempts = 0;
        let result = loop {
            match client
                .post($url)
                .header("Content-Type", "application/json")
                .body(json_body.clone())
                .send()
                .await
            {
                Ok(response) => {
                    break Ok(response);
                }
                Err(e) => {
                    attempts += 1;
                    if attempts < $max_retries {
                        // Exponential backoff: wait 2^attempts * 100ms
                        tokio::time::sleep(std::time::Duration::from_millis(
                            100 * (2_u64.pow(attempts as u32)),
                        ))
                        .await;
                        continue;
                    } else {
                        break Err(e);
                    }
                }
            }
        };
        result
    }};
}

#[macro_export]
macro_rules! send_get_request {
    ($url:expr) => {
        send_get_request!($url, 3) // Default to 3 retries
    };
    ($url:expr, $max_retries:expr) => {{
        let client = reqwest::Client::new();
        let mut attempts = 0;
        let result = loop {
            match client.get($url).send().await {
                Ok(response) => {
                    break Ok(response);
                }
                Err(e) => {
                    attempts += 1;
                    if attempts < $max_retries {
                        // Exponential backoff: wait 2^attempts * 100ms
                        tokio::time::sleep(std::time::Duration::from_millis(
                            100 * (2_u64.pow(attempts as u32)),
                        ))
                        .await;
                        continue;
                    } else {
                        break Err(e);
                    }
                }
            }
        };
        result
    }};
}
