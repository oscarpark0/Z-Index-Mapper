use actix_web::{web, App, HttpResponse, HttpServer, Responder, HttpRequest, Error};
use actix_web_actors::ws;
use actix::{Actor, StreamHandler};
use std::sync::{Arc, Mutex};
use std::collections::BTreeMap;
use regex::Regex;
use walkdir::WalkDir;
use serde::Serialize;
use serde_json;
use std::time::Duration;
use notify::{Watcher, RecursiveMode, watcher, DebouncedEvent};
use std::sync::mpsc::channel;
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Serialize, Clone)]
struct ZIndexEntry {
    z_index: i64,
    occurrences: Vec<(String, usize)>, // (file_path, line_number)
}

struct MyWebSocket {
    data: Arc<Mutex<BTreeMap<i64, ZIndexEntry>>>,
}

impl MyWebSocket {
    fn new(data: Arc<Mutex<BTreeMap<i64, ZIndexEntry>>>) -> Self {
        MyWebSocket { data }
    }
}

impl Actor for MyWebSocket {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                if text == "refresh" {
                    let data = self.data.lock().unwrap();
                    let json = serde_json::to_string(&*data).unwrap();
                    ctx.text(json);
                }
            }
            _ => (),
        }
    }
}

async fn index() -> impl Responder {
    HttpResponse::Ok().body(include_str!("index.html"))
}

async fn ws_index(req: HttpRequest, stream: web::Payload, data: web::Data<Arc<Mutex<BTreeMap<i64, ZIndexEntry>>>>) -> Result<HttpResponse, Error> {
    let res = ws::start(MyWebSocket::new(data.get_ref().clone()), &req, stream)?;
    Ok(res)
}

// Add this new function to handle the /component endpoint
async fn get_component(query: web::Query<ComponentQuery>) -> impl Responder {
    let file_path = PathBuf::from(&query.file);
    let line_number: usize = query.line.parse().unwrap_or(1);

    match fs::read_to_string(&file_path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = line_number.saturating_sub(3);
            let end = (line_number + 3).min(lines.len());
            let relevant_lines = lines[start..end].join("\n");
            
            let component_data = ComponentData {
                content: relevant_lines,
                z_index_line: line_number - start,
            };
            
            HttpResponse::Ok().json(component_data)
        }
        Err(_) => HttpResponse::NotFound().body("File not found or couldn't be read")
    }
}

// Add this struct to parse query parameters
#[derive(serde::Deserialize)]
struct ComponentQuery {
    file: String,
    line: String,
}

// Add this struct to represent the component data
#[derive(Serialize)]
struct ComponentData {
    content: String,
    z_index_line: usize,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let data = Arc::new(Mutex::new(BTreeMap::<i64, ZIndexEntry>::new()));

    // Set the correct path to scan
    let scan_path = PathBuf::from(r"C:\Users\tb\Desktop\tarotmancer-frontend");

    // Set up file system watcher
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(2)).unwrap();
    watcher.watch(&scan_path, RecursiveMode::Recursive).unwrap();

    // Exclude directories from watching
    for dir in &["node_modules", "build", ".git"] {
        let path = scan_path.join(dir);
        if path.exists() {
            match watcher.watch(&path, RecursiveMode::Recursive) {
                Ok(_) => println!("Warning: {} is being watched. This may slow down the scanning process.", dir),
                Err(_) => println!("Successfully excluded {} from watching.", dir),
            }
        }
    }

    // Spawn a thread to handle file system events
    let event_data = data.clone();
    let event_scan_path = scan_path.clone();
    std::thread::spawn(move || {
        loop {
            match rx.recv() {
                Ok(event) => {
                    match event {
                        DebouncedEvent::Create(_) | DebouncedEvent::Write(_) | DebouncedEvent::Remove(_) | DebouncedEvent::Rename(_, _) => {
                            // File change detected, update data
                            let mut map = event_data.lock().unwrap();
                            *map = scan_files(&event_scan_path);
                        }
                        _ => (), // Ignore other events
                    }
                }
                Err(e) => println!("Watch error: {:?}", e),
            }
            // Add a small delay to prevent excessive CPU usage
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    // Perform an initial scan
    let mut initial_data = data.lock().unwrap();
    *initial_data = scan_files(&scan_path);
    drop(initial_data);  // Release the lock before moving `data`

    let app_data = web::Data::new(data.clone());
    HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .route("/", web::get().to(index))
            .route("/ws", web::get().to(ws_index))
            .route("/component", web::get().to(get_component))  // Add this line
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

fn scan_files(project_root: &PathBuf) -> BTreeMap<i64, ZIndexEntry> {
    let mut map = BTreeMap::<i64, ZIndexEntry>::new();
    let re = Regex::new(r"(?:z-index|zIndex):\s*(-?\d+)").unwrap();

    println!("Scanning directory: {:?}", project_root);

    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| !should_ignore(e))
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.path().extension().map_or(false, |ext| 
            ["js", "jsx", "ts", "tsx", "css", "scss"].contains(&ext.to_str().unwrap())) {
            println!("Checking file: {:?}", entry.path());
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                for (line_number, line) in content.lines().enumerate() {
                    println!("Checking line {}: {}", line_number + 1, line);  // Add this line
                    for cap in re.captures_iter(line) {
                        if let Ok(z_index) = cap[1].parse::<i64>() {
                            println!("Found z-index: {} in {:?} at line {}", z_index, entry.path(), line_number + 1);
                            map.entry(z_index).or_insert_with(|| ZIndexEntry {
                                z_index,
                                occurrences: Vec::new(),
                            }).occurrences.push((
                                entry.path().to_str().unwrap().to_string(),
                                line_number + 1,
                            ));
                        }
                    }
                }
            } else {
                eprintln!("Error reading file: {:?}", entry.path());
            }
        }
    }

    println!("Scanned files. Found {} unique z-index values.", map.len());
    map
}

fn should_ignore(entry: &walkdir::DirEntry) -> bool {
    let path = entry.path();
    let file_name = entry.file_name().to_str().unwrap_or("");

    // Ignore node_modules
    if file_name == "node_modules" {
        return true;
    }

    // Ignore build directory
    if path.components().any(|c| c.as_os_str() == "build") {
        return true;
    }

    // Ignore .git directory
    if file_name == ".git" {
        return true;
    }

    false
}