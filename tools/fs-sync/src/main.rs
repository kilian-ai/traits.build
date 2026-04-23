//! fs-sync — tiny 9P2000.L server exposing a local folder over TCP.
//!
//! Usage:
//!     fs-sync --dir <path> [--host 127.0.0.1] [--port 5640] [--readonly]
//!
//! Mount from a 9P client (Linux kernel v9fs):
//!     mount -t 9p -o trans=tcp,port=5640,version=9p2000.L,aname=/ \
//!           <server_ip> /mnt/host
//!
//! No authentication. Bind to 127.0.0.1 unless you know what you're doing.

mod fs;
mod proto;
mod server;

use std::net::{TcpListener, ToSocketAddrs};
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;

use server::Conn;

struct Args {
    dir: PathBuf,
    host: String,
    port: u16,
    readonly: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut dir: Option<PathBuf> = None;
    let mut host = "127.0.0.1".to_string();
    let mut port: u16 = 5640;
    let mut readonly = false;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dir" | "-d" => {
                dir = it.next().map(PathBuf::from);
            }
            "--host" | "-H" => {
                host = it.next().ok_or("--host needs a value")?;
            }
            "--port" | "-p" => {
                let v = it.next().ok_or("--port needs a value")?;
                port = v.parse().map_err(|_| "invalid port")?;
            }
            "--readonly" | "-r" => {
                readonly = true;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown arg: {other}")),
        }
    }
    let dir = dir.ok_or("--dir <path> is required")?;
    Ok(Args {
        dir,
        host,
        port,
        readonly,
    })
}

fn print_help() {
    println!(
        "fs-sync — minimal 9P2000.L file server\n\n\
         USAGE:\n    \
         fs-sync --dir <path> [--host 127.0.0.1] [--port 5640] [--readonly]\n\n\
         Mount (Linux):\n    \
         mount -t 9p -o trans=tcp,port=5640,version=9p2000.L,aname=/ \\\n           \
         <server_ip> /mnt/host"
    );
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("fs-sync: {e}");
            print_help();
            return ExitCode::from(2);
        }
    };

    let root = match std::fs::canonicalize(&args.dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("fs-sync: cannot open {}: {e}", args.dir.display());
            return ExitCode::FAILURE;
        }
    };
    if !root.is_dir() {
        eprintln!("fs-sync: {} is not a directory", root.display());
        return ExitCode::FAILURE;
    }

    let bind = format!("{}:{}", args.host, args.port);
    let addrs: Vec<_> = match bind.to_socket_addrs() {
        Ok(it) => it.collect(),
        Err(e) => {
            eprintln!("fs-sync: bad bind address {bind}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let listener = match TcpListener::bind(addrs.as_slice()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("fs-sync: bind {bind} failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    eprintln!(
        "fs-sync: serving {} on {}{} (9P2000.L)",
        root.display(),
        bind,
        if args.readonly { " [readonly]" } else { "" }
    );

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("fs-sync: accept error: {e}");
                continue;
            }
        };
        let peer = stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "?".into());
        let root = root.clone();
        let readonly = args.readonly;
        thread::spawn(move || {
            eprintln!("fs-sync: + client {peer}");
            let conn = Conn::new(root, readonly);
            if let Err(e) = conn.serve(stream) {
                eprintln!("fs-sync: client {peer} ended: {e}");
            } else {
                eprintln!("fs-sync: - client {peer}");
            }
        });
    }
    ExitCode::SUCCESS
}
