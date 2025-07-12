use anyhow::{Context, Result};
use clap::Parser;
use crossbeam_channel::{select, Receiver};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use log::{debug, info};
use nix::errno::Errno;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command};
use std::time::Duration;
use std::{env, thread};
use thiserror::Error;

#[cfg(unix)]
use {
    nix::sys::signal::{self, Signal},
    nix::sys::wait::{Id, WaitPidFlag},
    nix::unistd::Pid,
    std::os::unix::process::CommandExt,
};

#[cfg(unix)]
fn kill(child: &mut Child, sig: &str) -> Result<()> {
    let sig = sig.parse::<Signal>()?;
    let pgid = match nix::unistd::getpgid(Some(Pid::from_raw(child.id() as i32))) {
        Ok(pid) => pid,
        // Pid does not exist
        Err(Errno::ESRCH) => return Ok(()),
        Err(e) => Err(e)?,
    };

    signal::killpg(pgid, sig)?;

    // HACK: we use a custom nix crate to have waitid available on macos.
    // Not sure why they feature flagged macos, it definitely has a posix compliant waitid implementation.
    nix::sys::wait::waitid(Id::PGid(pgid), WaitPidFlag::all())
        .map(|_| ())
        .map_err(Into::into)
}

fn spawn(
    program: impl AsRef<str>,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<Child> {
    let mut cmd = Command::new(program.as_ref());
    cmd.args(args.into_iter());

    configure_command(&mut cmd);

    cmd.spawn().context(format!(
        "failed to spawn the provided utility: {}",
        program.as_ref()
    ))
}

#[cfg(unix)]
fn configure_command(cmd: &mut Command) {
    cmd.process_group(0);
}

#[cfg(windows)]
fn configure_command(_cmd: &mut Command) {
    // No-op on Windows
}

#[cfg(windows)]
fn kill(child: &mut Child, _sig: &str) -> Result<()> {
    child
        .kill()
        .with_context(|| format!("failed to kill child process"))?;
    child.wait().map(|_| ()).map_err(Into::into)
}

#[derive(Debug, Error)]
pub enum IoWatchError {
    #[error("no files or directories to watch")]
    NoFilesToWatch,
}

#[derive(Debug, Parser)]
#[command(name = "iowatch")]
#[command(about = "Cross platform way to run arbitrary commands when files change")]
#[command(author, version)]
pub struct Cli {
    /// File or directory to watch. To specify multiple files or directories, use standard input instead.
    #[arg(short = 'f')]
    input_file: Option<String>,
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
}

pub struct IoWatch {
    exit_after: bool,
    postpone: bool,
    recursive_mode: RecursiveMode,
    files: Vec<String>,
    timeout: Duration,
    delay: u64,
    clear_term: bool,
    kill_sig: String,
    utility_cmd: Vec<String>,
    utility_process: Option<Child>,
    first_run: bool,
}

impl IoWatch {
    /// Run the application
    pub fn run(mut self) -> Result<()> {
        debug!("Starting IoWatch with utility: {:?}", self.utility_cmd);
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut debouncer = new_debouncer(Duration::from_millis(25), None, tx)?;
        let watcher = debouncer.watcher();

        for f in &self.files {
            watcher
                .watch(f.as_ref(), self.recursive_mode)
                .with_context(|| format!("Failed to watch {}", f))?;
        }

        let ignore_matcher = self.get_ignore_matcher()?;

        let ctrlc_rx = self.ctrlc_events()?;

        debug!("exit after: {}", self.exit_after);

        if !self.postpone {
            debug!("Running utility immediately as postpone is false");
            self.run_utility()?;
            if self.exit_after {
                debug!("Exiting after first run as requested");
                return Ok(());
            }
        }

        loop {
            self.pump_events(rx.clone(), ctrlc_rx.clone(), &ignore_matcher)?;
            if self.exit_after {
                break;
            }
        }

        Ok(())
    }

    fn pump_events(
        &mut self,
        rx: Receiver<DebounceEventResult>,
        ctrlc_rx: Receiver<()>,
        ignore_matcher: &Gitignore,
    ) -> Result<()> {
        select! {
            // handle filesystem events
            recv(rx) -> res => {
                match res {
                    Ok(inner) => match inner {
                        Ok(events) => {
                            let ignore = events.iter()
                                .any(|e| ignore_matcher.matched_path_or_any_parents(&e.path, e.path.is_dir()).is_ignore());
                            if !ignore {
                                self.run_utility()?;
                            }
                        },
                        Err(errors) =>  errors.iter().for_each(|e| eprintln!("Error {:?}",e)),
                    },
                    Err(e) => Err(e)?
                }
            },
            // handle timeout case
            recv(crossbeam::channel::after(self.timeout)) -> _ => {
                debug!("Timeout reached, running utility");
                self.run_utility()?;
            },
            // handle ctrl+c
            recv(ctrlc_rx) -> _ => {
                info!("Ctrl+C received, exiting...");
                self.kill_utility()?;
                self.exit_after = true;
            }
        }

        Ok(())
    }

    /// Setup a handler and channel receiver for ctrl+c notifications
    fn ctrlc_events(&self) -> Result<Receiver<()>, ctrlc::Error> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        ctrlc::set_handler(move || {
            let _ = tx.send(());
        })?;

        Ok(rx)
    }

    /// Creates an ignore matcher from ignore files in dir
    fn get_ignore_matcher(&self) -> Result<Gitignore> {
        let root = env::current_dir()?;
        let gitignore_path = Path::new(&root).join(".gitignore");
        let ignore_path = Path::new(&root).join(".ignore");

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
            Some(ref mut child) => kill(child, &self.kill_sig),
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

        self.utility_process = Some(spawn(&self.utility_cmd[0], &self.utility_cmd[1..])?);

        self.first_run = false;

        Ok(())
    }
}

impl TryFrom<Cli> for IoWatch {
    type Error = anyhow::Error;
    fn try_from(cli: Cli) -> Result<Self> {
        let mut cli = cli;
        let utility = if !cli.use_shell {
            cli.utility
        } else {
            let mut shell = IoWatch::get_shell_cmd();
            shell.append(&mut cli.utility);
            shell
        };

        let files: Vec<String> = if let Some(file) = cli.input_file {
            debug!("Using input file: {}", file);
            vec![file]
        } else {
            debug!("Reading files from stdin");
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .context("Failed to read files from stdin")?;
            buf.trim()
                .lines()
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        };

        if files.is_empty() {
            Err(IoWatchError::NoFilesToWatch)?
        }

        let recursive = if cli.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        Ok(IoWatch {
            exit_after: cli.exit_after,
            postpone: cli.postpone,
            recursive_mode: recursive,
            first_run: true,
            utility_cmd: utility,
            files,
            delay: cli.delay,
            clear_term: cli.clear_term,
            timeout: Duration::from_secs(cli.timeout.unwrap_or(u64::MAX)),
            kill_sig: cli.kill_signal,
            utility_process: None,
        })
    }
}

impl Debug for IoWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IoWatch")
            .field("exit_after", &self.exit_after)
            .field("postpone", &self.postpone)
            .field("recursive_mode", &self.recursive_mode)
            .field("files", &self.files)
            .field("timeout", &self.timeout)
            .field("delay", &self.delay)
            .field("clear_term", &self.clear_term)
            .field("kill_sig", &self.kill_sig)
            .field("utility_cmd", &self.utility_cmd)
            .finish()
    }
}
