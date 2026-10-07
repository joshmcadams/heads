use crate::repo::{Output, RunError, Runner};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

#[cfg(not(unix))]
compile_error!("heads currently supports Linux and macOS (Unix process groups required)");

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

// Unix signal/kill constants are shared by Linux and macOS. The handler only
// writes an atomic; all process cleanup and reporting happens in normal code.
extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
    fn kill(pid: i32, signal: i32) -> i32;
}

extern "C" fn interrupt_handler(_: i32) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

pub fn install_interrupt_handler() -> std::io::Result<()> {
    // SAFETY: the function has the C signal-handler ABI and remains valid for
    // the process lifetime. The handler performs only an atomic store.
    let previous = unsafe { signal(2, interrupt_handler as *const () as usize) };
    if previous == usize::MAX {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst) || INTERRUPTED.load(Ordering::SeqCst)
    }
}

struct GroupChild(Child);

impl Drop for GroupChild {
    fn drop(&mut self) {
        // SAFETY: each child starts a new process group whose ID is its PID.
        // A negative PID targets that group, including helpers and SSH children.
        unsafe { kill(-(self.0.id() as i32), 9) };
        let _ = self.0.wait();
    }
}

pub struct GitRunner {
    path: PathBuf,
    deadline: Instant,
    timeout: Duration,
    cancellation: Cancellation,
}

impl GitRunner {
    pub fn new(path: &Path, timeout: Duration, cancellation: Cancellation) -> Self {
        Self {
            path: path.to_path_buf(),
            deadline: Instant::now() + timeout,
            timeout,
            cancellation,
        }
    }

    fn check(&self) -> Result<(), RunError> {
        if self.cancellation.is_cancelled() {
            Err(RunError::Cancelled)
        } else if Instant::now() >= self.deadline {
            Err(RunError::Failed(format!(
                "timed out after {}s",
                self.timeout.as_secs_f64()
            )))
        } else {
            Ok(())
        }
    }
}

fn capture(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = pipe.read_to_end(&mut bytes).map(|_| bytes);
        let _ = sender.send(result);
    });
    receiver
}

impl Runner for GitRunner {
    fn run(&mut self, args: &[&str]) -> Result<Output, RunError> {
        use std::os::unix::process::CommandExt;
        self.check()?;
        let label = format!("git {}", args.join(" "));
        let failure = |error| RunError::Failed(format!("{label}: {error}"));
        let mut command = Command::new("git");
        command
            .current_dir(&self.path)
            .arg("--no-optional-locks")
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        if std::env::var_os("GIT_SSH_COMMAND").is_none() {
            command.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
        }
        let mut child = GroupChild(command.spawn().map_err(failure)?);
        let stdout = capture(child.0.stdout.take().expect("piped stdout"));
        let stderr = capture(child.0.stderr.take().expect("piped stderr"));
        let mut status = None;
        let mut out = None;
        let mut err = None;
        loop {
            self.check().map_err(|error| match error {
                RunError::Failed(message) => RunError::Failed(format!("{message} ({label})")),
                other => other,
            })?;
            if status.is_none() {
                status = child.0.try_wait().map_err(failure)?;
            }
            for (receiver, value) in [(&stdout, &mut out), (&stderr, &mut err)] {
                if value.is_none() {
                    match receiver.try_recv() {
                        Ok(bytes) => *value = Some(bytes.map_err(failure)?),
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => {
                            return Err(RunError::Failed(format!(
                                "{label}: output reader disconnected"
                            )));
                        }
                    }
                }
            }
            if let (Some(status), Some(out), Some(err)) = (status, out.as_ref(), err.as_ref()) {
                return Ok(Output {
                    code: status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(out).into_owned(),
                    stderr: String::from_utf8_lossy(err).into_owned(),
                });
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
