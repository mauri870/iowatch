extern crate notify;
#[macro_use]
extern crate clap;

use clap::{Arg, App};
use notify::{RecommendedWatcher, Watcher, RecursiveMode, DebouncedEvent};
use std::sync::mpsc;
use std::time::Duration;
use std::io::{self, Read, ErrorKind};
use std::process::{Command, Output, Stdio};

// TODO: Add a verbose flag
fn main() {
    let matches = App::new("entr")
        .version(&crate_version!()[..])
        .author("Mauri de Souza nunes <mauri870@gmail.com>")
        .about(
            "Cross platform way to run arbitrary commands when files change.",
        )
        .arg(Arg::with_name("clear").short("c").help(
            "Clear the screen before invoking the utility",
        ))
        .arg(Arg::with_name("utility").multiple(true))
        .get_matches();

    let clear_term = matches.is_present("clear");
    let utility = matches.values_of_lossy("utility").unwrap();

    if utility.is_empty() {
        panic!("No utility provided");
    }

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
    run_command(&utility, clear_term);

    loop {
        match rx.recv() {
            // Discard initial notices
            Ok(DebouncedEvent::NoticeWrite(_)) => continue,
            Ok(DebouncedEvent::NoticeRemove(_)) => continue,
            Ok(DebouncedEvent::Chmod(_)) => continue,
            Ok(_) => run_command(&utility, clear_term),
            Err(e) => panic!("watch error: {}", e),
        }
    }
}

fn clear_term_screen() -> io::Result<Output> {
    let clear_cmd = if cfg!(windows) {
        "cls"
    } else {
        // Assume POSIX
        "clear"
    };
    Command::new(clear_cmd)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
}

fn run_command(utility: &Vec<String>, clear_term: bool) {
    if clear_term {
        clear_term_screen().expect("Failed to clear terminal screen");
    }

    match Command::new(&utility[0])
        .args(&utility[1..])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output() {
        Err(e) => {
            if let ErrorKind::NotFound = e.kind() {
                panic!(
                    "{} was not found! Check your PATH or the provided utility!",
                    &utility[0]
                );
            } else {
                panic!("Error running the specified utility: {}", e);
            }
        }
        _ => {}
    }
}
