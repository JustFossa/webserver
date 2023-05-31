use std::collections::HashMap;
use std::net::TcpListener;
use std::net::TcpStream;
use std::io::prelude::*;
use std::fs;
use rayon::ThreadPoolBuilder;
use reqwest;
use serde_yaml;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = fs::read_to_string("config.yml").unwrap();
    let values: HashMap<&str, &str> = serde_yaml::from_str(&config as &str).unwrap();
    let thread_pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let listener = TcpListener::bind(values["ip"].to_owned() + ":" + values["port"] as &str).unwrap();

    println!("[SERVER]: Running on port {}", values["port"]);


    match fs::read("404.html") {
        Ok(_) => {
            println!("[LOGGING]: 404.html has been found!")
        }
        Err(_) => {
            println!("[LOGGING]: 404.html not found, creating one for you!");
            let res = reqwest::get("https://raw.githubusercontent.com/mhill426/free404/gh-pages/lights_off/404.html").await.unwrap().text().await;
            let clean_contents = res.unwrap().replace("\n", "").replace("\\","");
            let mut file = fs::File::create("404.html").unwrap();
            file.write(clean_contents.as_bytes())?;
        }
    }

    for stream_result in listener.incoming() {
        match stream_result {
            Ok(stream) => {
                thread_pool.spawn(|| {
                    handle_connection(stream).unwrap();
                })
            }
            Err(error) => {
                println!("Error accepting incoming connection: {}", error);
            }
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> std::io::Result<()> {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();

    let req_string = String::from_utf8_lossy(&buffer[..]);
    let vector = req_string.split(" ").collect::<Vec<&str>>();
    //let method = vector[0];
    let path = vector[1];
    let mut file = path.split("/").collect::<Vec<&str>>()[1];

    if path == "/" {
        file = "index.html";
    }

    match fs::read_to_string(file) {
        Ok(contents) => {
            let response = format!("HTTP/1.1 200 OK\r\n Content-Length: {}\r\n\r\n{}", contents.len(), contents);
            stream.write(response.as_bytes()).unwrap();
        }
        Err(_) => {
            let contents = fs::read_to_string("404.html").unwrap();
            let response = format!("HTTP/1.1 404 Not found\r\n Content-Length: {}\r\n\r\n{}", contents.len(), contents);
            stream.write(response.as_bytes()).unwrap();
        }
    }
    Ok(())
}
