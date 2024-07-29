use actix_web::{web, App, HttpResponse, HttpServer, Responder, HttpRequest, Error};
use actix_web_actors::ws;
use actix::{Actor, StreamHandler};
use std::sync::{Arc, RwLock};
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
    data: Arc<RwLock<BTreeMap<i64, ZIndexEntry>>>,
    scan_path: Arc<RwLock<PathBuf>>,
}

impl MyWebSocket {
    fn new(data: Arc<RwLock<BTreeMap<i64, ZIndexEntry>>>, scan_path: Arc<RwLock<PathBuf>>) -> Self {
        MyWebSocket { data, scan_path }
    }
}

impl Actor for MyWebSocket {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                let message: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                if let Some(action) = message.get("action") {
                    match action.as_str() {
                        Some("refresh") => {
                            if let Some(directory) = message.get("directory") {
                                if let Some(dir_str) = directory.as_str() {
                                    let mut scan_path = self.scan_path.write().unwrap();
                                    *scan_path = PathBuf::from(dir_str);
                                    drop(scan_path);  // Release the write lock

                                    let mut data = self.data.write().unwrap();
                                    *data = scan_files(&*self.scan_path.read().unwrap());
                                    drop(data);  // Release the write lock
                                }
                            }
                            let data = self.data.read().unwrap();
                            let json = serde_json::to_string(&*data).unwrap();
                            ctx.text(json);
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }
}

async fn index() -> impl Responder {
    HttpResponse::Ok().body(include_str!("index.html"))
}

async fn ws_index(req: HttpRequest, stream: web::Payload, data: web::Data<Arc<RwLock<BTreeMap<i64, ZIndexEntry>>>>, scan_path: web::Data<Arc<RwLock<PathBuf>>>) -> Result<HttpResponse, Error> {
    let res = ws::start(MyWebSocket::new(data.get_ref().clone(), scan_path.get_ref().clone()), &req, stream)?;
    Ok(res)
}

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

#[derive(serde::Deserialize)]
struct ComponentQuery {
    file: String,
    line: String,
}

#[derive(Serialize)]
struct ComponentData {
    content: String,
    z_index_line: usize,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let data = Arc::new(RwLock::new(BTreeMap::<i64, ZIndexEntry>::new()));
    let scan_path = Arc::new(RwLock::new(PathBuf::from(r"C:\Users\tb\Desktop\tarotmancer-frontend")));

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(2)).unwrap();
    
    // Dereference the RwLockReadGuard
    watcher.watch(&*scan_path.read().unwrap(), RecursiveMode::Recursive).unwrap();

    for dir in &["node_modules", "build", ".git"] {
        let path = scan_path.read().unwrap().join(dir);
        if path.exists() {
            match watcher.watch(&path, RecursiveMode::Recursive) {
                Ok(_) => println!("Warning: {} is being watched. This may slow down the scanning process.", dir),
                Err(_) => println!("Successfully excluded {} from watching.", dir),
            }
        }
    }

    let data_clone = data.clone();
    let scan_path_clone = scan_path.clone();

    std::thread::spawn(move || {
        loop {
            match rx.recv() {
                Ok(event) => {
                    match event {
                        DebouncedEvent::Create(_) | DebouncedEvent::Write(_) | DebouncedEvent::Remove(_) | DebouncedEvent::Rename(_, _) => {
                            let mut map = data_clone.write().unwrap();
                            *map = scan_files(&*scan_path_clone.read().unwrap());
                        }
                        _ => (), // Ignore other events
                    }
                }
                Err(e) => println!("Watch error: {:?}", e),
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    let mut initial_data = data.write().unwrap();
    *initial_data = scan_files(&*scan_path.read().unwrap());
    drop(initial_data);  // Release the write lock

    let app_data = web::Data::new(data.clone());
    let app_scan_path = web::Data::new(scan_path.clone());
    HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .app_data(app_scan_path.clone())
            .route("/", web::get().to(index))
            .route("/ws", web::get().to(ws_index))
            .route("/component", web::get().to(get_component))
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
                    println!("Checking line {}: {}", line_number + 1, line);
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

    if file_name == "node_modules" {
        return true;
    }

    if path.components().any(|c| c.as_os_str() == "build") {
        return true;
    }

    if file_name == ".git" {
        return true;
    }

    false
}