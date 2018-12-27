extern crate notify;

use notify::{RecommendedWatcher, Watcher, RecursiveMode, DebouncedEvent};
use std::sync::mpsc;
use std::time::Duration;
use std::io::{self, Read, ErrorKind};
use std::process::{Command, Stdio};

// TODO: Add a verbose flag
fn main() {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).expect(
        "Failed to read files to watch",
    );

    let files: Vec<String> = buf.trim()
        .split("\n")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect();

    if files.is_empty() {
        panic!("No files or dirs to watch");
    }

    let command: Vec<_> = std::env::args().skip(1).collect();
    if command.is_empty() {
        panic!("No command provided");
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(0)).unwrap();

    for f in &files {
        // TODO: Add a recursive flag
        match watcher.watch(f, RecursiveMode::Recursive) {
            Err(_) => panic!("Failed to watch {}", f),
            _ => {}
        }
    }

    // Running first iteration manually
    run_command(&command);

    loop {
        match rx.recv() {
            // Discard initial notices
            Ok(DebouncedEvent::NoticeWrite(_)) => {}
            Ok(DebouncedEvent::NoticeRemove(_)) => {}
            Ok(_) => run_command(&command),
            Err(e) => panic!("watch error: {}", e),
        }
    }
}

fn run_command(command: &Vec<String>) {
    match Command::new(&command[0])
        .args(&command[1..])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output() {
        Err(e) => {
            if let ErrorKind::NotFound = e.kind() {
                panic!(
                    "{} was not found! Check your PATH or the provided command!",
                    &command[0]
                );
            } else {
                panic!("Error running the specified command: {}", e);
            }
        }
        _ => {}
    }
}
