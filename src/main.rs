#![allow(unused_variables)]

use std::collections::HashMap;
use std::net::TcpListener;
use std::net::TcpStream;
use std::io::prelude::*;
use std::fs;
use std::fs::File;
use std::sync::{Arc, Mutex};
use rayon::ThreadPoolBuilder;
use reqwest;
use serde_yaml;
use chrono;
use chrono::Utc;
use lazy_static::lazy_static;

lazy_static! {
    static ref FILE_CACHE: Mutex<HashMap<String, Vec<u8>>> = {
        Mutex::new(HashMap::new())
    };
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = tokio::fs::read_to_string("config.yml").await?;
    let config_str = config.as_str();
    let values: HashMap<String, String> = serde_yaml::from_str(&config_str).unwrap();
    let thread_pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let listener = TcpListener::bind(format!("{}:{}", &values["ip"], &values["port"])).unwrap();


    match tokio::fs::remove_file(format!("{}/last.log.old", &values["logs_folder"])).await {
        Ok(_) => {
            println!("[SERVER]: last.log.old deleted successfully");
        },
        Err(_) => {
            println!("[ERROR]: last.log.old was not found");
        }
    }

    match tokio::fs::rename(format!("{}/last.log", &values["logs_folder"]), format!("{}/last.log.old", &values["logs_folder"])).await {
        Ok(_) => {
            println!("[SERVER]: last.log created successfully");
        },
        Err(err) => {
            println!("[ERROR]: {}", err.to_string());
        }
    }
    let log_file = File::create(format!("{}/last.log", &values["logs_folder"])).unwrap();

    println!("[SERVER]: Listening at {}:{}", &values["ip"], &values["port"]);

    match fs::read(format!("{}/{}", &values["server_root"], &values["not_found_file"])) {
        Ok(_) => {
            println!("[LOGGING]: 404.html has been found!")
        }
        Err(_) => {
            println!("[LOGGING]: 404.html not found, creating one for you");
            let res = reqwest::get("https://raw.githubusercontent.com/JustFossa/webserver/main/404.html").await.unwrap().text().await;
            let clean_contents = res.unwrap().replace("\n", "").replace("\\","");
            let mut file = File::create("404.html").unwrap();
            file.write(clean_contents.as_bytes())?;
        }
    }

    let values= Arc::new(Mutex::new(values));

    for stream_result in listener.incoming() {

        match stream_result {
            Ok(stream) => {
                let values_clone = Arc::clone(&values);
                let log_clone = log_file.try_clone().unwrap();
                thread_pool.spawn(move || {
                        handle_connection(stream, &values_clone, &log_clone).expect("Handling connection failed");
                })
            }
            Err(error) => {
                println!("Error accepting incoming connection: {}", error);
            }
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, config: &Arc<Mutex<HashMap<String,String>>>, mut log_file: &File) -> std::io::Result<()> {
    let mut buffer = [0; 1024];
    let mut cache = FILE_CACHE.lock().unwrap();

    stream.read(&mut buffer).unwrap();

    let req_string = String::from_utf8_lossy(&buffer[..]);
    let vector = req_string.split(" ").collect::<Vec<&str>>();
    let method = vector[0];
    let path = vector[1];
    let file = if path == "/" {"index"} else {path.split("/").collect::<Vec<&str>>()[1].split(".").collect::<Vec<&str>>()[0]};
    let config = &*config.lock().unwrap();

    let file_path = format!("{}/{}.html", config["server_root"], file);

    log_file.write(format!("[{}]: {} \n", Utc::now().format("%Y-%m-%d %H:%M:%S"), req_string.split("\r\n").collect::<Vec<&str>>()[0]).as_bytes()).unwrap();

    if cache.get(file).is_none() {
        match fs::read_to_string(file_path) {
            Ok(contents) => {
                let response = format!("HTTP/1.1 200 OK\r\n Content-Length: {}\r\n\r\n{}", contents.len(), contents);
                stream.write(response.as_bytes()).unwrap();
                cache.insert(String::from(file), Vec::from(contents.as_bytes()));
            }
            Err(_) => {
                let contents = fs::read_to_string(format!("{}/404.html", config["server_root"])).unwrap();
                let response = format!("HTTP/1.1 404 Not found\r\n Content-Length: {}\r\n\r\n{}", contents.len(), contents);
                stream.write(response.as_bytes()).unwrap();
            }
        }
    } else {
        let contents = String::from_utf8_lossy(cache.get(file).unwrap());
        let response = format!("HTTP/1.1 200 OK\r\n Content-Length: {}\r\n\r\n{}",  contents.len(), contents);
        stream.write(response.as_bytes()).unwrap();
    }
    Ok(())
}
