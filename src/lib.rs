use std::{env, thread};
use std::io::{self, Read};
use std::process::{Command, Child};
use std::path::Path;
use std::time::Duration;
use std::sync::mpsc::{Receiver, RecvTimeoutError};

use thiserror::Error;
use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use structopt::StructOpt;

use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};

#[derive(Debug, Error)]
pub enum IoWatchError {
    #[error("no files or directories to watch")]
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
    /// Exit after the utility completes it's first execution
    #[structopt(short = "z")]
    exit_after: bool,
    /// The amount of seconds to wait until the command is executed if no events have been fired
    #[structopt(short = "t")]
    timeout: Option<u64>,
    /// The time delay in ms to apply before running the utility
    #[structopt(short = "d")]
    delay: Option<u64>,
    /// The kill signal to use, defaults to SIGTERM
    #[structopt(short = "k", default_value = "SIGTERM")]
    kill_signal: String,
    /// The utility to run when files change
    utility: Vec<String>,

    /// The currently running utility process
    #[structopt(skip)]
    utility_process: Option<Child>,
    /// Flag to track if is first execution
    #[structopt(skip)]
    first_run: bool,
}

impl IoWatch {
    /// Run the application
    pub fn run(
        mut self,
        rx: &Receiver<notify::Result<notify::Event>>,
        mut watcher: RecommendedWatcher,
    ) -> Result<(), anyhow::Error> {
        self.first_run = true;
        self.utility = if !self.use_shell {
            self.utility
        } else {
            let mut shell = IoWatch::get_shell_cmd();
            shell.append(&mut self.utility);
            shell
        };

        if !self.postpone {
            self.run_utility()?;
            if self.exit_after {
               return Ok(());
            }
        }

        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read files to watch")?;

        let files: Vec<&str> = buf.trim().split('\n').filter(|s| !s.is_empty()).collect();

        if files.is_empty() {
            Err(IoWatchError::NoFilesToWatch)?
        }

        let recursive_mode = if self.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        for f in files {
            watcher
                .watch(Path::new(f), recursive_mode)
                .with_context(|| format!("Failed to watch {}", f))?;
        }

        loop {
            match rx.recv_timeout(Duration::from_secs(self.timeout.unwrap_or(u64::MAX))) {
                Ok(result) => match result {
                    Ok(notify::event::Event { kind: notify::event::EventKind::Access(_), ..}) => continue,
                    Ok(notify::event::Event { kind: notify::event::EventKind::Other, ..}) => continue,
                    Ok(_) => self.run_utility()?,
                    Err(e) => Err(e).context("notify error")?,
                },
                Err(RecvTimeoutError::Timeout) => self.run_utility()?,
                Err(e) => Err(e).context("channel error")?,
            }

            if self.exit_after {
                break;
            }
        }

        Ok(())
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
    fn clear_term_screen(&self) -> Result<(), anyhow::Error> {
        Command::new("clear")
            .status()
            .or_else(|_| Command::new("cmd").args(&["/c", "cls"]).status())?;
        Ok(())
    }

    /// Kill the utility if still running
    fn kill_utility(&mut self) -> Result<(), anyhow::Error> {
        // TODO(mauri870): use the more generic approach below for windows
        // match &mut self.utility_process {
        //     Some(child) => child.kill().with_context(|| format!("failed to kill child process")),
        //     None => Ok(()),
        // }

        if let Some(child) = &mut self.utility_process {
            signal::kill(Pid::from_raw(child.id() as i32), self.kill_signal.parse::<Signal>()?).context("failed to kill child process")?;
        }

        self.utility_process = None;

        Ok(())
    }

    /// Wait for a delay in ms
    fn wait_delay(&self) -> Result<(), anyhow::Error> {
        thread::sleep(Duration::from_millis(self.delay.unwrap_or(0)));
        Ok(())
    }

    /// Run the provided utility
    fn run_utility(&mut self) -> Result<(), anyhow::Error> {
        if self.utility_process.is_some() {
            self.kill_utility()?;
        }

        if self.clear_term {
            self.clear_term_screen()
                .context("Failed to clear terminal screen")?;
        }

        // apply delay only on subsequent runs
        if !self.first_run && self.delay.is_some() {
            self.wait_delay()?;
        }

        self.utility_process = Some(Command::new(&self.utility[0])
            .args(&self.utility[1..])
            .spawn()
            .context(format!("failed to run the provided utility: {}", &self.utility[0]))?);

        self.first_run = false;

        Ok(())
    }
}
