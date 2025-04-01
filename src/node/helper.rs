use super::*;
pub fn is_between(start: u32, key: u32, end: u32) -> bool {
    if start < end {
        key > start && key <= end
    } else {
        key > start || key <= end
    }
}

pub fn in_mem_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();

    #[cfg(debug_assertions)]
    println!("Database connection established.");

    let schema = std::fs::read_to_string("./misc/schema.sql").expect("Failed to read schema.sql");
    conn.execute(&schema, []).expect("Failed to execute schema");

    #[cfg(debug_assertions)]
    {
        println!("Table 'data' created.");
        let mut stmt = conn.prepare("PRAGMA table_info(data)").unwrap();
        let table_info = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })
            .unwrap();

        println!("Schema of 'data' table:");
        for info in table_info {
            let (column_name, column_type) = info.unwrap();
            println!("{}: {}", column_name, column_type);
        }
    }

    conn
}

pub async fn get_data(data: web::Data<Node>) -> impl Responder {
    let conn = data.db.lock().await;
    let mut stmt = conn.prepare("SELECT * FROM data").unwrap();
    let data_iter = stmt
        .query_map([], |row| {
            // println!("Row: {:?}", row);
            Ok(Data::new(
                row.get::<_, String>(1).unwrap().as_str(),
                row.get::<_, String>(2).unwrap().as_str(),
            ))
        })
        .unwrap();

    let mut data_vec = Vec::new();
    for data in data_iter {
        data_vec.push(data.unwrap());
    }

    HttpResponse::Ok().json(data_vec)
}

pub async fn run_server(app_state: web::Data<Node>) -> std::io::Result<()> {
    let id = app_state.node_state.lock().await.id.clone();
    #[cfg(debug_assertions)]
    println!("(Node)Server running at http://{}", id.clone());

    app_state.node_state.lock().await.predecessor = Some(id.clone());
    app_state
        .node_state
        .lock()
        .await
        .successor
        .insert_first(id.clone());

    let bind_address = {
        let node_state = app_state.node_state.lock().await;
        node_state.id.clone()
    };
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/", web::get().to(handle_index))
            .route("/data", web::get().to(get_data))
            .route("/leave", web::post().to(handle_leave))
            .route("/join", web::post().to(handle_join))
            .route("/insert", web::post().to(handle_insert))
            .route("/successors", web::get().to(handle_successors))
            .route("/predecessor", web::get().to(handle_predecessor))
            .route(
                "/msg",
                web::post().to(move |data: web::Data<Node>, message: web::Json<Message>| {
                    handle_message(data, message)
                }),
            )
    })
    .bind(bind_address.clone())?
    .run()
    .await
}
