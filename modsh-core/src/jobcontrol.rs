//! Job control — Foreground/background execution

use std::collections::HashMap;

/// A job (pipeline or single command)
#[derive(Debug)]
pub struct Job {
    /// Job ID
    pub id: usize,
    /// Command string
    pub command: String,
    /// Process group ID
    pub pgid: Option<u32>,
    /// Status
    pub status: JobStatus,
    /// Processes in this job
    pub processes: Vec<ProcessInfo>,
}

/// Process information
#[derive(Debug)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Program name
    pub command: String,
    /// Exit status if completed
    pub status: Option<std::process::ExitStatus>,
}

/// Job status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JobStatus {
    /// Running
    Running,
    /// Stopped
    Stopped,
    /// Completed
    Completed,
    /// Killed
    Killed,
}

/// Job control manager
pub struct JobControl {
    jobs: HashMap<usize, Job>,
    next_id: usize,
    current_job: Option<usize>,
    previous_job: Option<usize>,
}

impl JobControl {
    /// Create a new job control manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            jobs: HashMap::new(),
            next_id: 1,
            current_job: None,
            previous_job: None,
        }
    }

    /// Add a new job
    pub fn add_job(&mut self, command: String, pgid: Option<u32>) -> usize {
        let id = self.next_id;
        self.next_id += 1;

        let job = Job {
            id,
            command,
            pgid,
            status: JobStatus::Running,
            processes: Vec::new(),
        };

        self.jobs.insert(id, job);
        self.update_current_job(id);
        id
    }

    /// Get a job by ID
    #[must_use]
    pub fn get_job(&self, id: usize) -> Option<&Job> {
        self.jobs.get(&id)
    }

    /// Get a mutable job by ID
    #[must_use]
    pub fn get_job_mut(&mut self, id: usize) -> Option<&mut Job> {
        self.jobs.get_mut(&id)
    }

    /// List all jobs
    #[must_use]
    pub fn list_jobs(&self) -> Vec<&Job> {
        self.jobs.values().collect()
    }

    /// Update job status
    pub fn update_status(&mut self, id: usize, status: JobStatus) {
        if let Some(job) = self.jobs.get_mut(&id) {
            job.status = status;
        }
    }

    /// Mark a job as completed
    pub fn mark_completed(&mut self, id: usize) {
        self.update_status(id, JobStatus::Completed);
        if self.current_job == Some(id) {
            self.current_job = self.previous_job;
            self.previous_job = None;
        }
    }

    /// Get the current job ID
    #[must_use]
    pub fn current_job(&self) -> Option<usize> {
        self.current_job
    }

    /// Get the previous job ID
    #[must_use]
    pub fn previous_job(&self) -> Option<usize> {
        self.previous_job
    }

    /// Update current and previous job references
    fn update_current_job(&mut self, id: usize) {
        self.previous_job = self.current_job;
        self.current_job = Some(id);
    }

    /// Bring a job to the foreground
    ///
    /// Gives terminal control to the job's process group, waits for it to complete
    /// or stop, then restores terminal control to the shell.
    ///
    /// # Errors
    /// Returns an error if the job ID is not found or terminal control fails
    #[cfg(unix)]
    pub fn foreground(&mut self, id: usize) -> Result<i32, String> {
        let job = self.jobs.get(&id).ok_or("No such job")?;
        let pgid = job.pgid.ok_or("Job has no process group")?;

        let shell_pgid = unsafe { libc::getpgrp() };
        let stdin_fd = libc::STDIN_FILENO;

        // Give terminal control to the job's process group
        unsafe {
            if libc::tcsetpgrp(stdin_fd, pgid as libc::pid_t) < 0 {
                return Err("tcsetpgrp failed".to_string());
            }
        }

        // If the job was stopped, continue it
        if job.status == JobStatus::Stopped {
            unsafe {
                libc::killpg(pgid as libc::pid_t, libc::SIGCONT);
            }
            if let Some(j) = self.jobs.get_mut(&id) {
                j.status = JobStatus::Running;
            }
        }

        self.update_current_job(id);

        // Wait for the job to complete or stop
        let mut status: libc::c_int = 0;
        let result = unsafe { libc::waitpid(-(pgid as libc::pid_t), &mut status, libc::WUNTRACED) };

        // Restore terminal control to the shell
        unsafe {
            let _ = libc::tcsetpgrp(stdin_fd, shell_pgid);
        }

        if result < 0 {
            return Err("waitpid failed".to_string());
        }

        // Update job status based on wait result
        let exit_code = if libc::WIFEXITED(status) {
            let code = libc::WEXITSTATUS(status);
            if let Some(j) = self.jobs.get_mut(&id) {
                j.status = JobStatus::Completed;
            }
            i32::from(code)
        } else if libc::WIFSIGNALED(status) {
            if let Some(j) = self.jobs.get_mut(&id) {
                j.status = JobStatus::Killed;
            }
            128 + libc::WTERMSIG(status)
        } else if libc::WIFSTOPPED(status) {
            if let Some(j) = self.jobs.get_mut(&id) {
                j.status = JobStatus::Stopped;
            }
            148 // 128 + SIGTSTP (20)
        } else {
            1
        };

        Ok(exit_code)
    }

    /// Non-Unix stub for foreground
    #[cfg(not(unix))]
    pub fn foreground(&mut self, _id: usize) -> Result<i32, String> {
        Err("Job control not supported on this platform".to_string())
    }

    /// Continue a job in the background
    ///
    /// Sends SIGCONT to the job's process group if it was stopped.
    ///
    /// # Errors
    /// Returns an error if the job ID is not found
    #[cfg(unix)]
    pub fn background(&mut self, id: usize) -> Result<(), String> {
        let job = self.jobs.get_mut(&id).ok_or("No such job")?;

        if job.status == JobStatus::Stopped {
            if let Some(pgid) = job.pgid {
                unsafe {
                    libc::killpg(pgid as libc::pid_t, libc::SIGCONT);
                }
            }
            job.status = JobStatus::Running;
        }

        println!("[{}] {}", id, job.command);
        Ok(())
    }

    /// Non-Unix stub for background
    #[cfg(not(unix))]
    pub fn background(&mut self, _id: usize) -> Result<(), String> {
        Err("Job control not supported on this platform".to_string())
    }

    /// Reap any children that have terminated (non-blocking)
    ///
    /// Call this periodically or after receiving SIGCHLD.
    #[cfg(unix)]
    pub fn reap_children(&mut self) {
        loop {
            let mut status: libc::c_int = 0;
            let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };

            if pid <= 0 {
                break;
            }

            // Find the job containing this process
            for job in self.jobs.values_mut() {
                if job.processes.iter().any(|p| p.pid == pid as u32) {
                    if libc::WIFEXITED(status) || libc::WIFSIGNALED(status) {
                        // Check if all processes in the job are done
                        // For now, mark the job as completed
                        job.status = JobStatus::Completed;
                    } else if libc::WIFSTOPPED(status) {
                        job.status = JobStatus::Stopped;
                    }
                    break;
                }
            }
        }
    }

    /// Non-Unix stub for reap_children
    #[cfg(not(unix))]
    pub fn reap_children(&mut self) {}

    /// Clean up completed jobs
    pub fn cleanup(&mut self) {
        self.jobs
            .retain(|_, job| !matches!(job.status, JobStatus::Completed | JobStatus::Killed));
    }
}

impl Default for JobControl {
    fn default() -> Self {
        Self::new()
    }
}

/// Signal handling for job control
pub mod signals {
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Flag set by SIGCHLD handler to indicate children need reaping
    static SIGCHLD_RECEIVED: AtomicBool = AtomicBool::new(false);

    /// Check if SIGCHLD was received since last check
    pub fn sigchld_pending() -> bool {
        SIGCHLD_RECEIVED.swap(false, Ordering::SeqCst)
    }

    /// SIGCHLD handler — async-signal-safe, just sets a flag
    extern "C" fn sigchld_handler(_sig: libc::c_int) {
        SIGCHLD_RECEIVED.store(true, Ordering::SeqCst);
    }

    /// Set up signal handlers for job control
    ///
    /// # Safety
    /// This function uses `sigaction` which is async-signal-safe.
    /// Should be called once during shell initialization.
    #[cfg(unix)]
    pub fn setup_handlers() {
        unsafe {
            // SIGCHLD — child process status changed
            let sa = libc::sigaction {
                sa_sigaction: sigchld_handler as *const () as usize,
                sa_mask: std::mem::zeroed(),
                sa_flags: libc::SA_RESTART,
                sa_restorer: None,
            };
            libc::sigaction(libc::SIGCHLD, &sa, std::ptr::null_mut());

            // SIGINT — shell ignores in interactive mode (sent to fg process group via terminal)
            let sa_int = libc::sigaction {
                sa_sigaction: libc::SIG_IGN,
                sa_mask: std::mem::zeroed(),
                sa_flags: 0,
                sa_restorer: None,
            };
            libc::sigaction(libc::SIGINT, &sa_int, std::ptr::null_mut());

            // SIGQUIT — shell ignores in interactive mode
            let sa_quit = libc::sigaction {
                sa_sigaction: libc::SIG_IGN,
                sa_mask: std::mem::zeroed(),
                sa_flags: 0,
                sa_restorer: None,
            };
            libc::sigaction(libc::SIGQUIT, &sa_quit, std::ptr::null_mut());
        }
    }

    /// Non-Unix stub
    #[cfg(not(unix))]
    pub fn setup_handlers() {
        // No-op on non-Unix platforms
    }
}
