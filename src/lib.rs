use std::env;
use std::io::{self, Read};
use std::process::Command;
use std::time::Duration;
use std::sync::mpsc::{Receiver, RecvTimeoutError};

use failure::{Fail, Error, ResultExt};
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use structopt::StructOpt;

#[derive(Debug, Fail)]
pub enum IoWatchError {
    #[fail(display = "No files or dirs to watch")]
    NoFilesToWatch,
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "iowatch",
    about = "Cross platform way to run arbitrary commands when files change"
)]
pub struct IoWatch {
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
    /// The amount of seconds to wait until the command is executed if no events have been fired
    #[structopt(short = "t")]
    timeout: Option<u64>,
    /// The utility to run when files change
    utility: Vec<String>,
}

impl IoWatch {
    /// Run the application
    pub fn run(
        mut self,
        rx: &Receiver<DebouncedEvent>,
        mut watcher: RecommendedWatcher,
    ) -> Result<(), Error> {
        self.utility = if !self.use_shell {
            self.utility
        } else {
            let mut shell = IoWatch::get_shell_cmd();
            shell.append(&mut self.utility);
            shell
        };

        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .with_context(|_| "Failed to read files to watch".to_string())?;

        let files: Vec<&str> = buf.trim().split('\n').filter(|s| !s.is_empty()).collect();

        if files.is_empty() {
            Err(IoWatchError::NoFilesToWatch)?
        }

        let recursive_mode = if self.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        for &f in &files {
            watcher
                .watch(f, recursive_mode)
                .with_context(|_| format!("Failed to watch {}", f))?;
        }

        // Running first iteration manually
        if !self.postpone {
            self.run_utility()?;
        }

        loop {
            match rx.recv_timeout(Duration::from_secs(self.timeout.unwrap_or(u64::MAX))) {
                // Discard initial notices
                Ok(DebouncedEvent::NoticeWrite(_)) => continue,
                Ok(DebouncedEvent::NoticeRemove(_)) => continue,
                Ok(DebouncedEvent::Chmod(_)) => continue,
                Ok(_) | Err(RecvTimeoutError::Timeout) => self.run_utility()?,
                Err(e) => Err(e).with_context(|_| "Error watching files".to_string())?,
            }
        }
    }

    /// Get the sytem's shell command string
    fn get_shell_cmd() -> Vec<String> {
        if cfg!(windows) {
            vec!["cmd".to_string(), "/c".to_string()]
        } else {
            // Assume GNU
            let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            vec![shell, "-c".to_string()]
        }
    }

    /// Clear the terminal screen
    fn clear_term_screen(&self) -> Result<(), Error> {
        Command::new("clear")
            .status()
            .or_else(|_| Command::new("cmd").args(&["/c", "cls"]).status())?;
        Ok(())
    }

    /// Run the provided utility
    fn run_utility(&self) -> Result<(), Error> {
        if self.clear_term {
            self.clear_term_screen()
                .with_context(|_| "Failed to clear terminal screen".to_string())?;
        }

        Command::new(&self.utility[0])
            .args(&self.utility[1..])
            .spawn()
            .with_context(|_| format!("{} Failed to run the provided utility", &self.utility[0]))?;

        Ok(())
    }
}
