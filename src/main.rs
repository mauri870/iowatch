extern crate notify;
#[macro_use]
extern crate clap;

use clap::{Arg, App};
use notify::{RecommendedWatcher, Watcher, RecursiveMode, DebouncedEvent};
use std::sync::mpsc;
use std::time::Duration;
use std::io::{self, Read, ErrorKind};
use std::env;
use std::process::{Command, ExitStatus};

fn main() {
    let matches = App::new("entr")
        .version(&crate_version!()[..])
        .author("Mauri de Souza nunes <mauri870@gmail.com>")
        .about(
            "Cross platform way to run arbitrary commands when files change",
        )
        .arg(Arg::with_name("clear").short("c").help(
            "Clear the screen before invoking the utility",
        ))
        .arg(Arg::with_name("postpone").short("p").help(
            "Postpone the first execution of the utility until a file is modified",
        ))
        .arg(Arg::with_name("recursive").short("R").help(
            "Watch for changes in directories recursively",
        ))
        .arg(Arg::with_name("shell").short("s").help(
            "Evaluate the first argument using the default interpreter",
        ))
        .arg(Arg::with_name("utility").multiple(true).help(
            "The utility to run when files change",
        ))
        .get_matches();

    let clear_term = matches.is_present("clear");
    let postpone = matches.is_present("postpone");
    let recursive = matches.is_present("recursive");
    let use_shell = matches.is_present("shell");

    let utility = match matches.values_of_lossy("utility") {
        Some(mut val) => {
            if !use_shell {
                val
            } else {
                let mut shell = get_shell_cmd();
                shell.append(&mut val);
                shell
            }
        }
        None => panic!("No utility provided"),
    };

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

    let recursive_mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    for f in &files {
        if let Err(err) = watcher.watch(f, recursive_mode) {
            panic!("Failed to watch {} - {}", f, err);
        }
    }

    // Running first iteration manually
    if !postpone {
        run_command(&utility, clear_term);
    }

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

fn get_shell_cmd() -> Vec<String> {
    if cfg!(windows) {
        vec!["cmd".to_string(), "/c".to_string()]
    } else {
        // Assume GNU
        let shell = env::var("SHELL").unwrap_or("/bin/sh".to_string());
        vec![shell, "-c".to_string()]
    }
}

fn clear_term_screen() -> io::Result<ExitStatus> {
    let clear_cmd = if cfg!(windows) {
        "cls"
    } else {
        // Assume POSIX
        "clear"
    };
    Command::new(clear_cmd).status()
}

fn run_command(utility: &Vec<String>, clear_term: bool) {
    if clear_term {
        clear_term_screen().expect("Failed to clear terminal screen");
    }

    match Command::new(&utility[0]).args(&utility[1..]).spawn() {
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
