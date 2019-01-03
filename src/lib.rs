extern crate notify;
extern crate structopt;
#[macro_use]
extern crate failure;
extern crate exitfailure;

use std::sync::mpsc::Receiver;
use std::io::{self, Read};
use std::env;
use std::process::Command;

use notify::{RecommendedWatcher, Watcher, RecursiveMode, DebouncedEvent};
use structopt::StructOpt;
use failure::{Error, ResultExt};
use exitfailure::ExitFailure;

#[derive(Debug, Fail)]
pub enum EntrError {
    #[fail(display = "No files or dirs to watch")]
    NoFilesToWatch,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "entr",
            about = "Cross platform way to run arbitrary commands when files change")]
pub struct Entr {
    /// Clear the screen before invoking the utility
    #[structopt(short = "c")]
    clear_term: bool,
    /// Postpone the first execution of the utility until a file is modified
    #[structopt(short = "p")]
    postpone: bool,
    /// Watch for changes in directories recursively
    #[structopt(short = "R")]
    recursive: bool,
    /// Evaluate the first argument using the default interpreter
    #[structopt(short = "s")]
    use_shell: bool,
    /// The utility to run when files change
    utility: Vec<String>,
}

impl Entr {
    /// Run the application
    pub fn run(
        mut self,
        rx: Receiver<DebouncedEvent>,
        mut watcher: RecommendedWatcher,
    ) -> Result<(), ExitFailure> {
        self.utility = if !self.use_shell {
            self.utility
        } else {
            let mut shell = Entr::get_shell_cmd();
            shell.append(&mut self.utility);
            shell
        };

        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).with_context(|_| {
            format!("Failed to read files to watch")
        })?;

        let files: Vec<&str> = buf.trim().split("\n").filter(|s| !s.is_empty()).collect();

        if files.is_empty() {
            Err(EntrError::NoFilesToWatch)?
        }

        let recursive_mode = if self.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        for f in &files {
            watcher.watch(f, recursive_mode).with_context(|_| {
                format!("Failed to watch {}", f)
            })?;
        }

        // Running first iteration manually
        if !self.postpone {
            self.run_utility()?;
        }

        loop {
            match rx.recv() {
                // Discard initial notices
                Ok(DebouncedEvent::NoticeWrite(_)) => continue,
                Ok(DebouncedEvent::NoticeRemove(_)) => continue,
                Ok(DebouncedEvent::Chmod(_)) => continue,
                Ok(_) => self.run_utility()?,
                Err(e) => Err(e).with_context(|_| format!("Error watching files"))?,
            }
        }
    }

    /// Get the sytem's shell command string
    fn get_shell_cmd() -> Vec<String> {
        if cfg!(windows) {
            vec!["cmd".to_string(), "/c".to_string()]
        } else {
            // Assume GNU
            let shell = env::var("SHELL").unwrap_or("/bin/sh".to_string());
            vec![shell, "-c".to_string()]
        }
    }

    /// Clear the terminal screen
    fn clear_term_screen(&self) -> Result<(), Error> {
        let clear_cmd = if cfg!(windows) {
            "cls"
        } else {
            // Assume POSIX
            "clear"
        };
        Command::new(clear_cmd).status()?;

        Ok(())
    }

    /// Run the provided utility
    fn run_utility(&self) -> Result<(), Error> {
        if self.clear_term {
            self.clear_term_screen().with_context(|_| {
                format!("Failed to clear terminal screen")
            })?;
        }

        Command::new(&self.utility[0])
            .args(&self.utility[1..])
            .spawn()
            .with_context(|_| {
                format!("{} Failed to run the provided utility", &self.utility[0])
            })?;

        Ok(())
    }
}
