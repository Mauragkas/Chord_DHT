use super::*;
use crate::node::node::CHORD_RING; // Add this import at the top
pub async fn handle_index(data: web::Data<Node>) -> impl Responder {
    let node_state = data.node_state.lock().await;
    let logs = data.logs.lock().await;
    let conn = data.db.lock().await;
    let mut stmt = conn
        .prepare("SELECT * FROM data ORDER BY hash ASC")
        .unwrap();

    let data_iter = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .unwrap();

    let mut data_vec = Vec::new();
    for data in data_iter {
        data_vec.push(data.unwrap());
    }
    // Read the HTML template
    let mut html = std::fs::read_to_string(HTML_PATH).expect("Failed to read HTML template");

    // Replace placeholders with actual data
    html = html.replace(
        "{{node_id}}",
        format!("{} [{}]", node_state.id, hash(&node_state.id)).as_str(),
    );
    html = html.replace("{HOME_URL}", &format!("http://{}", CHORD_RING.lock().await));
    html = html.replace(
        "{{predecessor}}",
        format!(
            // "{} ({})",
            "<a href=\"http://{0}\">{0} [{1}]</a>",
            node_state
                .predecessor
                .as_ref()
                .unwrap_or(&"None".to_string()),
            node_state
                .predecessor
                .as_ref()
                .map(|id| hash(id))
                .unwrap_or(0)
        )
        .as_str(),
    );
    html = html.replace(
        "{{successor}}",
        format!(
            // "{} ({})",
            "<a href=\"http://{0}\">{0} [{1}]</a>",
            node_state.successor.as_ref().unwrap_or(&"None".to_string()),
            node_state
                .successor
                .as_ref()
                .map(|id| hash(id))
                .unwrap_or(0)
        )
        .as_str(),
    );

    let log_html = logs
        .iter()
        .map(|log| format!("<li>{}</li>", log))
        .collect::<Vec<String>>()
        .join("");
    html = html.replace(
        "{logs}",
        if log_html.is_empty() {
            "<li>No logs</li>"
        } else {
            &log_html
        },
    );

    let finger_table_html = node_state
        .finger_table
        .entries
        .iter()
        .map(|entry| {
            format!(
                "<li>[{0}]: <span class=\"font-semibold\"><a href=\"http://{1}\">{1}</a> [{2}]</span></li>",
                entry.start,
                entry.id.as_ref().unwrap_or(&"None".to_string()),
                entry.id.as_ref().map(|id| hash(id)).unwrap_or_default()
            )
        })
        .collect::<Vec<String>>()
        .join("");
    html = html.replace(
        "{finger_table}",
        if finger_table_html.is_empty() {
            "<li>No entries</li>"
        } else {
            &finger_table_html
        },
    );

    let data_html = data_vec
        .iter()
        .map(|data| format!("<li>[{}] {}: {}</li>", data.0, data.1, data.2))
        .collect::<Vec<String>>()
        .join("");
    html = html.replace(
        "{node_data}",
        if data_html.is_empty() {
            "<li>No data</li>"
        } else {
            &data_html
        },
    );

    // Return the updated HTML content
    HttpResponse::Ok().content_type("text/html").body(html)
}
