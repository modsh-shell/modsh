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
    /// # Errors
    /// Returns an error if the job ID is not found
    pub fn foreground(&mut self, id: usize) -> Result<(), String> {
        let _job = self.jobs.get(&id).ok_or("No such job")?;

        // TODO: Implement proper terminal control with tcsetpgrp
        // This requires unsafe libc calls on Unix

        self.update_current_job(id);
        Ok(())
    }

    /// Continue a job in the background
    ///
    /// # Errors
    /// Returns an error if the job ID is not found
    pub fn background(&mut self, id: usize) -> Result<(), String> {
        let job = self.jobs.get_mut(&id).ok_or("No such job")?;

        if job.status == JobStatus::Stopped {
            job.status = JobStatus::Running;
            // TODO: Send SIGCONT to process group
        }

        println!("[{}] {}", id, job.command);
        Ok(())
    }

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
    /// Set up signal handlers for job control
    pub fn setup_handlers() {
        // TODO: Install signal handlers for:
        // - SIGINT (Ctrl+C) - interrupt foreground job
        // - SIGTSTP (Ctrl+Z) - stop foreground job
        // - SIGCHLD - child process status changed
        // - SIGHUP - terminal disconnected

        // This requires platform-specific unsafe code
    }
}
