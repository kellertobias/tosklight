//! The import job model.
//!
//! Import is long-running, so it is a job with an identity, typed progress, a terminal outcome,
//! and a cancellation path — not a request that blocks until it finishes. Concurrency is bounded,
//! because a dropped folder of fifty clips must not start fifty transcodes and starve the render
//! thread of CPU.

use std::collections::VecDeque;

use media_domain::MediaAddress;
use uuid::Uuid;

/// How many imports may run at once by default.
///
/// Two leaves headroom for the render thread and the decoders on a modest machine, which matters
/// more than finishing an import a few seconds sooner.
pub const DEFAULT_CONCURRENCY: usize = 2;

/// A job's identity, which a client polls or subscribes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(Uuid);

impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Where a job is.
#[derive(Debug, Clone, PartialEq)]
pub enum JobState {
    Queued,
    Running {
        frames_done: u32,
        frames_total: Option<u32>,
    },
    /// Finished and published. The library now holds it.
    Succeeded {
        frames: u32,
    },
    /// Finished without publishing. Nothing was left behind.
    Failed {
        reason: String,
    },
    Cancelled,
}

impl JobState {
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Succeeded { .. } | Self::Failed { .. } | Self::Cancelled
        )
    }

    /// How far along, when that can be computed. Nothing invents a percentage.
    pub fn fraction(&self) -> Option<f32> {
        match self {
            Self::Queued => Some(0.0),
            Self::Running {
                frames_done,
                frames_total: Some(total),
            } if *total > 0 => Some((*frames_done as f32 / *total as f32).clamp(0.0, 1.0)),
            Self::Succeeded { .. } => Some(1.0),
            _ => None,
        }
    }
}

/// One import.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: JobId,
    pub source: std::path::PathBuf,
    pub destination: MediaAddress,
    pub state: JobState,
}

/// A bounded queue of imports.
///
/// The queue itself runs nothing; it decides what should run and records what happened. Keeping
/// the policy separate from the work is what makes admission, ordering, cancellation, and
/// retention testable without transcoding anything.
#[derive(Debug)]
pub struct ImportQueue {
    concurrency: usize,
    waiting: VecDeque<JobId>,
    jobs: Vec<Job>,
    running: Vec<JobId>,
}

impl ImportQueue {
    pub fn new(concurrency: usize) -> Self {
        Self {
            concurrency: concurrency.max(1),
            waiting: VecDeque::new(),
            jobs: Vec::new(),
            running: Vec::new(),
        }
    }

    pub const fn concurrency(&self) -> usize {
        self.concurrency
    }

    /// Queues an import and returns its identity.
    pub fn submit(&mut self, source: std::path::PathBuf, destination: MediaAddress) -> JobId {
        let id = JobId::new();
        self.jobs.push(Job {
            id,
            source,
            destination,
            state: JobState::Queued,
        });
        self.waiting.push_back(id);
        id
    }

    pub fn job(&self, id: JobId) -> Option<&Job> {
        self.jobs.iter().find(|job| job.id == id)
    }

    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    pub fn running(&self) -> usize {
        self.running.len()
    }

    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    /// The oldest runnable job, if there is room. Imports to one address are serialized because
    /// they publish the same clip and thumbnail; other addresses can still use the free workers.
    pub fn next_to_start(&mut self) -> Option<Job> {
        if self.running.len() >= self.concurrency {
            return None;
        }
        let position = self.waiting.iter().position(|id| {
            let Some(waiting) = self.job(*id) else {
                return false;
            };
            !self.running.iter().any(|running| {
                self.job(*running)
                    .is_some_and(|job| job.destination == waiting.destination)
            })
        })?;
        let id = self.waiting.remove(position)?;
        self.running.push(id);
        self.set(
            id,
            JobState::Running {
                frames_done: 0,
                frames_total: None,
            },
        );
        self.job(id).cloned()
    }

    /// Records progress on a running job.
    pub fn progress(&mut self, id: JobId, frames_done: u32, frames_total: Option<u32>) {
        if self.running.contains(&id)
            && self
                .job(id)
                .is_some_and(|job| matches!(job.state, JobState::Running { .. }))
        {
            self.set(
                id,
                JobState::Running {
                    frames_done,
                    frames_total,
                },
            );
        }
    }

    /// Records a terminal outcome and frees the slot.
    pub fn finish(&mut self, id: JobId, state: JobState) {
        debug_assert!(state.is_terminal());
        self.waiting.retain(|waiting| *waiting != id);
        self.running.retain(|running| *running != id);
        self.set(id, state);
    }

    /// Cancels a job. One still queued never starts; one already running is marked so the worker
    /// stops at its next frame and leaves nothing behind.
    pub fn cancel(&mut self, id: JobId) -> bool {
        let Some(job) = self.jobs.iter().find(|job| job.id == id) else {
            return false;
        };
        if job.state.is_terminal() {
            return false;
        }
        self.waiting.retain(|waiting| *waiting != id);
        self.set(id, JobState::Cancelled);
        true
    }

    /// Drops finished jobs, keeping the most recent so a client that asks late still sees what
    /// happened. Failures are kept: the whole point of a failed job is that somebody reads it.
    pub fn forget_completed(&mut self, keep: usize) {
        let mut succeeded: Vec<JobId> = self
            .jobs
            .iter()
            .filter(|job| {
                matches!(job.state, JobState::Succeeded { .. } | JobState::Cancelled)
                    && !self.running.contains(&job.id)
            })
            .map(|job| job.id)
            .collect();
        if succeeded.len() <= keep {
            return;
        }
        succeeded.truncate(succeeded.len() - keep);
        self.jobs.retain(|job| !succeeded.contains(&job.id));
    }

    fn set(&mut self, id: JobId, state: JobState) {
        if let Some(job) = self.jobs.iter_mut().find(|job| job.id == id) {
            job.state = state;
        }
    }
}

impl Default for ImportQueue {
    fn default() -> Self {
        Self::new(DEFAULT_CONCURRENCY)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn queue() -> ImportQueue {
        ImportQueue::new(2)
    }

    fn submit(queue: &mut ImportQueue, name: &str, file: u8) -> JobId {
        queue.submit(PathBuf::from(name), MediaAddress::new(1, file))
    }

    #[test]
    fn a_job_starts_queued_and_reports_its_source_and_destination() {
        let mut queue = queue();
        let id = submit(&mut queue, "clip.mp4", 4);
        let job = queue.job(id).unwrap();
        assert_eq!(job.state, JobState::Queued);
        assert_eq!(job.destination, MediaAddress::new(1, 4));
        assert_eq!(job.state.fraction(), Some(0.0));
    }

    #[test]
    fn no_more_than_the_concurrency_runs_at_once() {
        let mut queue = queue();
        for file in 1..=5u8 {
            submit(&mut queue, "clip.mp4", file);
        }

        assert!(queue.next_to_start().is_some());
        assert!(queue.next_to_start().is_some());
        assert!(
            queue.next_to_start().is_none(),
            "a third would starve the render thread"
        );
        assert_eq!(queue.running(), 2);
        assert_eq!(queue.waiting(), 3);
    }

    #[test]
    fn a_finished_job_frees_its_slot_for_the_next_one() {
        let mut queue = queue();
        for file in 1..=3u8 {
            submit(&mut queue, "clip.mp4", file);
        }
        let first = queue.next_to_start().unwrap();
        queue.next_to_start().unwrap();
        assert!(queue.next_to_start().is_none());

        queue.finish(first.id, JobState::Succeeded { frames: 100 });
        assert!(
            queue.next_to_start().is_some(),
            "the third starts once a slot frees"
        );
    }

    #[test]
    fn the_queue_drains_in_the_order_things_were_dropped_in() {
        let mut queue = ImportQueue::new(1);
        let ids: Vec<JobId> = (1..=3u8)
            .map(|file| submit(&mut queue, "clip.mp4", file))
            .collect();

        for expected in ids {
            let started = queue.next_to_start().unwrap();
            assert_eq!(started.id, expected);
            queue.finish(started.id, JobState::Succeeded { frames: 1 });
        }
    }

    #[test]
    fn progress_is_reported_only_where_it_can_be_computed() {
        let mut queue = queue();
        let id = submit(&mut queue, "clip.mp4", 1);
        queue.next_to_start();

        queue.progress(id, 25, Some(100));
        assert_eq!(queue.job(id).unwrap().state.fraction(), Some(0.25));

        queue.progress(id, 25, None);
        assert_eq!(
            queue.job(id).unwrap().state.fraction(),
            None,
            "a source with no frame count gets no invented percentage"
        );
    }

    #[test]
    fn a_queued_job_can_be_cancelled_before_it_starts() {
        let mut queue = queue();
        let id = submit(&mut queue, "clip.mp4", 1);
        assert!(queue.cancel(id));
        assert_eq!(queue.job(id).unwrap().state, JobState::Cancelled);
        assert!(
            queue.next_to_start().is_none(),
            "a cancelled job never starts"
        );
    }

    #[test]
    fn a_running_job_keeps_its_slot_until_cancellation_is_acknowledged() {
        let mut queue = ImportQueue::new(1);
        let first = submit(&mut queue, "clip.mp4", 1);
        submit(&mut queue, "clip.mp4", 2);
        queue.next_to_start();

        assert!(queue.cancel(first));
        assert_eq!(queue.running(), 1);
        assert!(queue.next_to_start().is_none());
        queue.progress(first, 1, Some(10));
        assert_eq!(queue.job(first).unwrap().state, JobState::Cancelled);
        queue.forget_completed(0);
        assert!(
            queue.job(first).is_some(),
            "the active address reservation survives retention"
        );
        queue.finish(first, JobState::Cancelled);
        assert!(queue.next_to_start().is_some());
    }

    #[test]
    fn same_address_jobs_wait_while_other_addresses_can_start() {
        let mut queue = queue();
        let first = submit(&mut queue, "first.mp4", 1);
        let replacement = submit(&mut queue, "replacement.mp4", 1);
        let other = submit(&mut queue, "other.mp4", 2);
        assert_eq!(queue.next_to_start().unwrap().id, first);
        assert_eq!(queue.next_to_start().unwrap().id, other);
        queue.finish(other, JobState::Succeeded { frames: 1 });
        assert!(queue.next_to_start().is_none());
        queue.cancel(first);
        assert!(queue.next_to_start().is_none());
        queue.finish(first, JobState::Cancelled);
        assert_eq!(queue.next_to_start().unwrap().id, replacement);
    }

    #[test]
    fn a_finished_job_cannot_be_cancelled_after_the_fact() {
        let mut queue = queue();
        let id = submit(&mut queue, "clip.mp4", 1);
        queue.next_to_start();
        queue.finish(id, JobState::Succeeded { frames: 10 });

        assert!(!queue.cancel(id));
        assert_eq!(
            queue.job(id).unwrap().state,
            JobState::Succeeded { frames: 10 }
        );
    }

    #[test]
    fn a_failure_is_retained_with_its_reason() {
        let mut queue = queue();
        let id = submit(&mut queue, "broken.mp4", 1);
        queue.next_to_start();
        queue.finish(
            id,
            JobState::Failed {
                reason: "no video stream".to_owned(),
            },
        );

        queue.forget_completed(0);
        let job = queue
            .job(id)
            .expect("a failure is kept for somebody to read");
        assert_eq!(
            job.state,
            JobState::Failed {
                reason: "no video stream".to_owned()
            }
        );
        assert_eq!(job.state.fraction(), None);
    }

    #[test]
    fn completed_jobs_are_forgotten_newest_first() {
        let mut queue = ImportQueue::new(4);
        let ids: Vec<JobId> = (1..=4u8)
            .map(|file| submit(&mut queue, "clip.mp4", file))
            .collect();
        for id in &ids {
            queue.next_to_start();
            queue.finish(*id, JobState::Succeeded { frames: 1 });
        }

        queue.forget_completed(2);
        assert!(queue.job(ids[0]).is_none(), "the oldest went");
        assert!(queue.job(ids[2]).is_some(), "the two most recent stayed");
        assert!(queue.job(ids[3]).is_some());
    }

    #[test]
    fn a_concurrency_of_zero_is_treated_as_one_rather_than_deadlocking() {
        let mut queue = ImportQueue::new(0);
        assert_eq!(queue.concurrency(), 1);
        submit(&mut queue, "clip.mp4", 1);
        assert!(queue.next_to_start().is_some());
    }
}
