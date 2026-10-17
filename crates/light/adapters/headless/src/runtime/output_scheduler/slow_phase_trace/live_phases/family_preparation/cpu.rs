//! Separate all-only CPU refinement; existing wall stages and old log fields stay unchanged.
use super::{ACTIVE, STAGES};
use light_engine::parallel::diagnostics::{CpuObservation, WorkerSummary};
use std::{marker::PhantomData, rc::Rc};
const SUBSTAGES: usize = 7;
const WORKERS: usize = 3;
#[derive(Clone, Copy)]
pub(in crate::runtime::output_scheduler) enum Substage {
    StaticDiscovery,
    Assembly,
    PrepareCurrent,
    PreparePrograms,
    ObserverFinish,
    SourceBindings,
    SourcesDrop,
}
#[derive(Clone, Copy)]
pub(in crate::runtime::output_scheduler) enum WorkerStage {
    CompleteDeferred,
    PrepareFamilies,
    OwnerCompose,
}
#[derive(Default)]
pub(super) struct Summary {
    stage_calls: [u64; STAGES],
    stage_valid: [u64; STAGES],
    stage_unavailable: [u64; STAGES],
    stage_invalid: [u64; STAGES],
    stage_cpu: [u64; STAGES],
    stage_read: [u64; STAGES],
    sub_calls: [u64; SUBSTAGES],
    sub_wall: [u64; SUBSTAGES],
    sub_valid: [u64; SUBSTAGES],
    sub_unavailable: [u64; SUBSTAGES],
    sub_invalid: [u64; SUBSTAGES],
    sub_cpu: [u64; SUBSTAGES],
    sub_read: [u64; SUBSTAGES],
    workers: [WorkerSummary; WORKERS],
}
fn add(target: &mut u64, value: u64) {
    *target = target.saturating_add(value);
}
impl Summary {
    pub(super) fn stage(&mut self, index: usize, sample: CpuObservation) {
        add(&mut self.stage_calls[index], 1);
        add(
            match sample.cpu_status {
                1 => &mut self.stage_valid[index],
                0 => &mut self.stage_unavailable[index],
                _ => &mut self.stage_invalid[index],
            },
            1,
        );
        add(&mut self.stage_cpu[index], sample.thread_cpu_nanos);
        add(&mut self.stage_read[index], sample.counter_read_nanos);
    }
    fn substage(&mut self, index: usize, sample: CpuObservation) {
        add(&mut self.sub_calls[index], 1);
        add(&mut self.sub_wall[index], sample.wall_nanos);
        add(
            match sample.cpu_status {
                1 => &mut self.sub_valid[index],
                0 => &mut self.sub_unavailable[index],
                _ => &mut self.sub_invalid[index],
            },
            1,
        );
        add(&mut self.sub_cpu[index], sample.thread_cpu_nanos);
        add(&mut self.sub_read[index], sample.counter_read_nanos);
    }
    fn worker(&mut self, index: usize, sample: WorkerSummary) {
        let target = &mut self.workers[index];
        target.pool_timing.merge(sample.pool_timing);
        add(&mut target.calls, sample.calls);
        add(&mut target.jobs, sample.jobs);
        add(&mut target.cpu_valid_jobs, sample.cpu_valid_jobs);
        add(
            &mut target.cpu_unavailable_jobs,
            sample.cpu_unavailable_jobs,
        );
        add(&mut target.cpu_invalid_jobs, sample.cpu_invalid_jobs);
        add(&mut target.cpu_nanos, sample.cpu_nanos);
        add(&mut target.counter_read_nanos, sample.counter_read_nanos);
        add(&mut target.job_wall_nanos, sample.job_wall_nanos);
        target.job_wall_max_nanos = target.job_wall_max_nanos.max(sample.job_wall_max_nanos);
        target.section_to_job_max_nanos = target
            .section_to_job_max_nanos
            .max(sample.section_to_job_max_nanos);
        add(
            &mut target.uncovered_sort_calls,
            sample.uncovered_sort_calls,
        );
    }
    pub(super) fn emit(&self) {
        let values = |f: fn(&WorkerSummary) -> u64| self.workers.each_ref().map(f);
        tracing::info!(schema_version=1u8,
            stage_calls=?self.stage_calls,stage_cpu_valid_calls=?self.stage_valid,stage_cpu_unavailable_calls=?self.stage_unavailable,stage_cpu_invalid_calls=?self.stage_invalid,stage_cpu_nanos=?self.stage_cpu,stage_counter_read_nanos=?self.stage_read,
            substage_calls=?self.sub_calls,substage_wall_nanos=?self.sub_wall,substage_cpu_valid_calls=?self.sub_valid,substage_cpu_unavailable_calls=?self.sub_unavailable,substage_cpu_invalid_calls=?self.sub_invalid,substage_cpu_nanos=?self.sub_cpu,substage_counter_read_nanos=?self.sub_read,
            worker_calls=?values(|w|w.calls),worker_jobs=?values(|w|w.jobs),worker_cpu_valid_jobs=?values(|w|w.cpu_valid_jobs),worker_cpu_unavailable_jobs=?values(|w|w.cpu_unavailable_jobs),worker_cpu_invalid_jobs=?values(|w|w.cpu_invalid_jobs),worker_cpu_nanos=?values(|w|w.cpu_nanos),worker_counter_read_nanos=?values(|w|w.counter_read_nanos),worker_job_wall_nanos=?values(|w|w.job_wall_nanos),worker_job_wall_max_nanos=?values(|w|w.job_wall_max_nanos),worker_section_to_job_max_nanos=?values(|w|w.section_to_job_max_nanos),worker_uncovered_sort_calls=?values(|w|w.uncovered_sort_calls),
            "output preparation cpu summary");
        let pool = |f: fn(&light_engine::parallel::diagnostics::PoolTimingSummary) -> u64| {
            self.workers.each_ref().map(|w| f(&w.pool_timing))
        };
        tracing::info!(schema_version=1u8,
            install_calls=?pool(|p|p.install_calls),install_jobs=?pool(|p|p.install_jobs),
            install_empty_calls=?pool(|p|p.install_empty_calls),install_invalid_calls=?pool(|p|p.install_invalid_calls),
            install_wall_nanos=?pool(|p|p.install_wall_nanos),submit_to_first_entry_nanos=?pool(|p|p.submit_to_first_entry_nanos),
            submit_to_entry_max_nanos=?pool(|p|p.submit_to_entry_max_nanos),last_exit_to_return_nanos=?pool(|p|p.last_exit_to_return_nanos),
            last_exit_to_return_max_nanos=?pool(|p|p.last_exit_to_return_max_nanos),scratch_lock_calls=?pool(|p|p.scratch_lock_calls),
            scratch_lock_wall_nanos=?pool(|p|p.scratch_lock_wall_nanos),scratch_lock_wall_max_nanos=?pool(|p|p.scratch_lock_wall_max_nanos),
            "output preparation pool timing");
    }
}
/// !Send and synchronous: never carry a CPU sample across a future poll boundary.
pub(in crate::runtime::output_scheduler) struct Subspan {
    kind: Substage,
    active: Option<(u64, light_engine::parallel::diagnostics::CpuTimer)>,
    _same_thread: PhantomData<Rc<()>>,
}
impl Subspan {
    pub(in crate::runtime::output_scheduler) fn begin(kind: Substage) -> Self {
        let active = super::super::full_frequency_enabled()
            .then(|| {
                ACTIVE.with(|frame| {
                    frame.borrow().as_ref().and_then(|frame| {
                        frame.family.cpu.as_ref().map(|_| {
                            (
                                frame.family.epoch,
                                light_engine::parallel::diagnostics::CpuTimer::start_if(true)
                                    .expect("enabled CPU frame"),
                            )
                        })
                    })
                })
            })
            .flatten();
        Self {
            kind,
            active,
            _same_thread: PhantomData,
        }
    }
}
impl Drop for Subspan {
    fn drop(&mut self) {
        if let Some((epoch, timer)) = self.active.take() {
            let sample = timer.finish();
            ACTIVE.with(|frame| {
                if let Some(frame) = frame.borrow_mut().as_mut()
                    && frame.family.epoch == epoch
                    && let Some(cpu) = frame.family.cpu.as_mut()
                {
                    cpu.substage(self.kind as usize, sample);
                }
            });
        }
    }
}
pub(in crate::runtime::output_scheduler) fn covered<T>(
    kind: WorkerStage,
    work: impl FnOnce() -> T,
) -> T {
    if !super::super::full_frequency_enabled() {
        return work();
    }
    let epoch = ACTIVE.with(|frame| {
        frame
            .borrow()
            .as_ref()
            .and_then(|frame| frame.family.cpu.as_ref().map(|_| frame.family.epoch))
    });
    let (result, summary) = light_engine::parallel::diagnostics::covered(epoch.is_some(), work);
    if let (Some(epoch), Some(summary)) = (epoch, summary) {
        ACTIVE.with(|frame| {
            if let Some(frame) = frame.borrow_mut().as_mut()
                && frame.family.epoch == epoch
                && let Some(cpu) = frame.family.cpu.as_mut()
            {
                cpu.worker(kind as usize, summary);
            }
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_statuses_and_worker_maxima_keep_coverage_truthful() {
        let mut summary = Summary::default();
        for status in [0, 1, 2] {
            let sample = CpuObservation {
                wall_nanos: 10,
                thread_cpu_nanos: u64::from(status == 1) * 4,
                cpu_status: status,
                counter_read_nanos: 2,
            };
            summary.stage(9, sample);
            summary.substage(1, sample);
        }
        assert_eq!(summary.stage_calls[9], 3);
        assert_eq!(
            (
                summary.stage_valid[9],
                summary.stage_unavailable[9],
                summary.stage_invalid[9]
            ),
            (1, 1, 1)
        );
        assert_eq!(summary.stage_cpu[9], 4);
        assert_eq!(summary.sub_calls[1], 3);
        assert_eq!(summary.sub_wall[1], 30);
        let worker = WorkerSummary {
            calls: 1,
            jobs: 2,
            cpu_valid_jobs: 2,
            cpu_nanos: 8,
            counter_read_nanos: 4,
            job_wall_nanos: 30,
            job_wall_max_nanos: 20,
            section_to_job_max_nanos: 7,
            uncovered_sort_calls: 1,
            pool_timing: light_engine::parallel::diagnostics::PoolTimingSummary {
                install_calls: 1,
                install_jobs: 2,
                install_wall_nanos: 31,
                submit_to_first_entry_nanos: 5,
                submit_to_entry_max_nanos: 8,
                last_exit_to_return_nanos: 3,
                last_exit_to_return_max_nanos: 3,
                scratch_lock_calls: 2,
                scratch_lock_wall_nanos: 4,
                scratch_lock_wall_max_nanos: 3,
                ..Default::default()
            },
            ..WorkerSummary::default()
        };
        summary.worker(0, worker);
        summary.worker(0, worker);
        assert_eq!(summary.workers[0].calls, 2);
        assert_eq!(summary.workers[0].jobs, 4);
        assert_eq!(summary.workers[0].job_wall_nanos, 60);
        assert_eq!(summary.workers[0].job_wall_max_nanos, 20);
        assert_eq!(summary.workers[0].section_to_job_max_nanos, 7);
        assert_eq!(summary.workers[0].uncovered_sort_calls, 2);
        assert_eq!(summary.workers[1].calls, 0);
        let pool = summary.workers[0].pool_timing;
        assert_eq!(pool.install_calls, 2);
        assert_eq!(pool.install_jobs, 4);
        assert_eq!(pool.install_wall_nanos, 62);
        assert_eq!(pool.submit_to_first_entry_nanos, 10);
        assert_eq!(pool.submit_to_entry_max_nanos, 8);
        assert_eq!(pool.last_exit_to_return_nanos, 6);
        assert_eq!(pool.last_exit_to_return_max_nanos, 3);
        assert_eq!(pool.scratch_lock_calls, 4);
        assert_eq!(pool.scratch_lock_wall_nanos, 8);
        assert_eq!(pool.scratch_lock_wall_max_nanos, 3);
    }
    #[test]
    fn absent_frame_has_no_cpu_scope_and_preserves_result() {
        assert!(Subspan::begin(Substage::Assembly).active.is_none());
        assert_eq!(covered(WorkerStage::CompleteDeferred, || 27), 27);
    }
}
