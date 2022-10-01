use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command};
use std::time::Duration;
use std::{env, thread};

use anyhow::{Context, Result};
use clap::Parser;
use crossbeam_channel::{select, Receiver};
use nix::sys::wait::{Id, WaitPidFlag};
use notify_debouncer_mini::new_debouncer;
use notify_debouncer_mini::notify::RecursiveMode;
use thiserror::Error;

use ignore::gitignore::{Gitignore, GitignoreBuilder};

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
    #[arg(short = 'd', default_value = "100")]
    delay: u64,
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
    pub fn run(mut self) -> Result<()> {
        let (tx, rx) = crossbeam_channel::unbounded();

        let mut debouncer = new_debouncer(Duration::from_millis(25), None, tx)?;

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

        let watcher = debouncer.watcher();

        for f in files {
            watcher
                .watch(Path::new(f), recursive_mode)
                .with_context(|| format!("Failed to watch {}", f))?;
        }

        let ignore_dir = env::current_dir()?;
        let ignore_matcher = self.get_ignore_matcher(ignore_dir)?;

        let ctrlc_rx = self.ctrlc_events()?;

        // Handle timeout case in select to also run the utility
        loop {
            select! {
                recv(rx) -> res => {
                    match res {
                        Ok(inner) => match inner {
                            Ok(events) => {
                                let ignore = events.iter()
                                    .any(|e| ignore_matcher.matched_path_or_any_parents(&e.path, e.path.is_dir()).is_ignore());
                                if !ignore {
                                    self.run_utility()?;

                                    if self.exit_after {
                                        break;
                                    }
                                }
                            },
                            Err(errors) =>  errors.iter().for_each(|e| eprintln!("Error {:?}",e)),
                        },
                        Err(e) => Err(e)?
                    }
                },
                recv(ctrlc_rx) -> _ => {
                    self.kill_utility()?;
                    break;
                }
            }
            // match rx.recv_timeout(Duration::from_secs(self.timeout.unwrap_or(u64::MAX))) {
            //     // Discard initial notices
            //     Ok(DebouncedEvent::NoticeWrite(_)) => continue,
            //     Ok(DebouncedEvent::NoticeRemove(_)) => continue,
            //     Ok(DebouncedEvent::Chmod(_)) => continue,
            //     Ok(DebouncedEvent::Rescan) => continue,
            //     Ok(DebouncedEvent::Remove(_)) => continue,
            //     Ok(DebouncedEvent::Error(e, _)) => Err(e)?,
            //     Ok(
            //         DebouncedEvent::Create(p)
            //         | DebouncedEvent::Write(p)
            //         | DebouncedEvent::Rename(_, p),
            //     ) => {
            //         if let Match::None = ignore_matcher.matched_path_or_any_parents(&p, p.is_dir())
            //         {
            //             self.run_utility()?;

            // if self.exit_after {
            //     break;
            // }
            //         }
            //     }
            //     Err(RecvTimeoutError::Timeout) => self.run_utility()?,
            //     Err(e) => Err(e).context("channel error")?,
            // }
        }

        Ok(())
    }

    fn ctrlc_events(&self) -> Result<Receiver<()>, ctrlc::Error> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        ctrlc::set_handler(move || {
            let _ = tx.send(());
        })?;

        Ok(rx)
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
        match self.utility_process {
            Some(ref mut child) => {
                if cfg!(unix) {
                    let sig = self.kill_signal.parse::<Signal>()?;
                    let pgid = nix::unistd::getpgid(Some(Pid::from_raw(child.id() as i32)))?;

                    signal::killpg(pgid, sig)?;

                    nix::sys::wait::waitid(Id::PGid(pgid), WaitPidFlag::all())
                        .map(|_| ())
                        .map_err(Into::into)
                } else {
                    child
                        .kill()
                        .with_context(|| format!("failed to kill child process"))?;
                    child.wait().map(|_| ()).map_err(Into::into)
                }
            }
            None => Ok(()),
        }
    }

    /// Wait for a delay in ms
    fn wait_delay(&self) -> Result<()> {
        thread::sleep(Duration::from_millis(self.delay));
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
        if !self.first_run && self.delay > 0 {
            self.wait_delay()?;
        }

        self.utility_process = Some(
            Command::new(&self.utility[0])
                .args(&self.utility[1..])
                .process_group(0)
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
