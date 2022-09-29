use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;
use std::{env, thread};

use anyhow::{Context, Result};
use clap::Parser;
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::Match;

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

#[derive(Debug, Error)]
pub enum IoWatchError {
    #[error("no files or directories to watch")]
    NoFilesToWatch,
}

#[derive(Debug, Parser)]
#[command(name = "iowatch")]
#[command(about = "Cross platform way to run arbitrary commands when files change")]
pub struct IoWatch {
    /// Clear the screen before invoking the utility
    #[arg(short = 'c')]
    clear_term: bool,
    /// Postpone the first execution of the utility until a file is modified
    #[arg(short = 'p')]
    postpone: bool,
    /// Watch for changes in directories recursively
    #[arg(short = 'R')]
    recursive: bool,
    /// Evaluate the first argument using the default interpreter
    #[arg(short = 's')]
    use_shell: bool,
    /// Exit after the utility completes it's first execution
    #[arg(short = 'z')]
    exit_after: bool,
    /// The amount of seconds to wait until the command is executed if no events have been fired
    #[arg(short = 't')]
    timeout: Option<u64>,
    /// The time delay in ms to apply before running the utility
    #[arg(short = 'd')]
    delay: Option<u64>,
    /// The kill signal to use, defaults to SIGTERM
    #[arg(short = 'k', default_value = "SIGTERM")]
    kill_signal: String,
    /// The utility to run when files change
    utility: Vec<String>,

    /// The currently running utility process
    #[arg(skip)]
    utility_process: Option<Child>,
    /// Flag to track if is first execution
    #[arg(skip)]
    first_run: bool,
}

impl IoWatch {
    /// Run the application
    pub fn run(
        mut self,
        rx: &Receiver<DebouncedEvent>,
        mut watcher: RecommendedWatcher,
    ) -> Result<()> {
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

        for f in &files {
            watcher
                .watch(f, recursive_mode)
                .with_context(|| format!("Failed to watch {}", f))?;
        }

        let ignore_dir = env::current_dir()?;
        let ignore_matcher = self.get_ignore_matcher(ignore_dir)?;

        loop {
            match rx.recv_timeout(Duration::from_secs(self.timeout.unwrap_or(u64::MAX))) {
                // Discard initial notices
                Ok(DebouncedEvent::NoticeWrite(_)) => continue,
                Ok(DebouncedEvent::NoticeRemove(_)) => continue,
                Ok(DebouncedEvent::Chmod(_)) => continue,
                Ok(DebouncedEvent::Rescan) => continue,
                Ok(DebouncedEvent::Remove(_)) => continue,
                Ok(DebouncedEvent::Error(e, _)) => Err(e)?,
                Ok(
                    DebouncedEvent::Create(p)
                    | DebouncedEvent::Write(p)
                    | DebouncedEvent::Rename(_, p),
                ) => {
                    if let Match::None = ignore_matcher.matched_path_or_any_parents(&p, p.is_dir())
                    {
                        self.run_utility()?;
                    }
                }
                Err(RecvTimeoutError::Timeout) => self.run_utility()?,
                Err(e) => Err(e).context("channel error")?,
            }

            if self.exit_after {
                break;
            }
        }

        Ok(())
    }

    /// Creates an ignore matcher from ignore files in dir
    fn get_ignore_matcher(&self, root: impl AsRef<Path>) -> Result<Gitignore> {
        let gitignore_path = Path::new(root.as_ref()).join(".gitignore");
        let ignore_path = Path::new(root.as_ref()).join(".ignore");

        let mut builder = GitignoreBuilder::new(root);
        if gitignore_path.exists() {
            builder.add(".gitignore");
        }

        if ignore_path.exists() {
            builder.add(".ignore");
        }

        let matcher = builder.build()?;
        Ok(matcher)
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
    fn clear_term_screen(&self) -> Result<()> {
        Command::new("clear")
            .status()
            .or_else(|_| Command::new("cmd").args(&["/c", "cls"]).status())?;
        Ok(())
    }

    /// Kill the utility if still running
    fn kill_utility(&mut self) -> Result<()> {
        // TODO(mauri870): use the more generic approach below for windows
        // match &mut self.utility_process {
        //     Some(child) => child.kill().with_context(|| format!("failed to kill child process")),
        //     None => Ok(()),
        // }

        if let Some(child) = &mut self.utility_process {
            signal::kill(
                Pid::from_raw(child.id() as i32),
                self.kill_signal.parse::<Signal>()?,
            )
            .context("failed to kill child process")?;

            child.wait()?;
        }

        self.utility_process = None;

        Ok(())
    }

    /// Wait for a delay in ms
    fn wait_delay(&self) -> Result<()> {
        thread::sleep(Duration::from_millis(self.delay.unwrap_or(0)));
        Ok(())
    }

    /// Run the provided utility
    fn run_utility(&mut self) -> Result<()> {
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

        self.utility_process = Some(
            Command::new(&self.utility[0])
                .args(&self.utility[1..])
                .spawn()
                .context(format!(
                    "failed to run the provided utility: {}",
                    &self.utility[0]
                ))?,
        );

        self.first_run = false;

        Ok(())
    }
}
