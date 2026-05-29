use crate::{ServerRenderer, ServerRequest};
use anyhow::{Context, Result};
use std::io::{self, BufRead, Write};
use std::net::{TcpListener, TcpStream};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServeOptions {
    pub host: String,
    pub port: u16,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8124,
        }
    }
}

pub fn serve(renderer: ServerRenderer, options: ServeOptions) -> Result<()> {
    let listener = TcpListener::bind((options.host.as_str(), options.port))
        .with_context(|| format!("failed to bind {}:{}", options.host, options.port))?;
    println!(
        "Serving Fission server app at http://{}:{}/",
        options.host, options.port
    );
    println!("Press Ctrl+C to stop.");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_stream(stream, &renderer) {
                    eprintln!("request failed: {error}");
                }
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
    Ok(())
}

fn handle_stream(mut stream: TcpStream, renderer: &ServerRenderer) -> Result<()> {
    let request = parse_request(&stream)?;
    let response = renderer.handle(request)?;
    write!(
        stream,
        "HTTP/1.1 {} OK\r\ncontent-length: {}\r\n",
        response.status,
        response.body.len()
    )?;
    for (name, value) in response.headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "\r\n")?;
    stream.write_all(&response.body)?;
    Ok(())
}

fn parse_request(stream: &TcpStream) -> Result<ServerRequest> {
    let mut reader = io::BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let raw_path = parts.next().unwrap_or("/");
    let (path, query) = parse_path_and_query(raw_path);
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim_end().is_empty() {
            break;
        }
    }
    Ok(ServerRequest {
        method,
        path,
        query,
        headers: Default::default(),
    })
}

fn parse_path_and_query(raw: &str) -> (String, std::collections::BTreeMap<String, String>) {
    let Some((path, query)) = raw.split_once('?') else {
        return (raw.to_string(), Default::default());
    };
    let mut out = std::collections::BTreeMap::new();
    for part in query.split('&') {
        if part.is_empty() {
            continue;
        }
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        out.insert(key.to_string(), value.to_string());
    }
    (path.to_string(), out)
}
