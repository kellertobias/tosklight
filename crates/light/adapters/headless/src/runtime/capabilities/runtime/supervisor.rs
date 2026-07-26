//! Capability-owned cancellation and join lifecycle for runtime background work.

use crate::runtime::{
    AppState, OwnedRuntimeTask, control_input_tasks, matter_bridge_sync, output_scheduler,
};
use anyhow::Context;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

type TaskOutcome = anyhow::Result<()>;

struct TaskGroup {
    name: &'static str,
    cancellation: CancellationToken,
    tasks: Vec<JoinHandle<TaskOutcome>>,
}

impl TaskGroup {
    fn new(name: &'static str, cancellation: CancellationToken) -> Self {
        Self {
            name,
            cancellation,
            tasks: Vec::new(),
        }
    }

    fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    fn track(&mut self, task: JoinHandle<TaskOutcome>) {
        self.tasks.push(task);
    }

    fn spawn(&mut self, task: impl std::future::Future<Output = TaskOutcome> + Send + 'static) {
        self.track(tokio::spawn(task));
    }

    async fn shutdown(self) -> anyhow::Result<()> {
        self.cancellation.cancel();
        let mut first_failure = None;
        for task in self.tasks {
            let result = task
                .await
                .with_context(|| format!("{} background task failed to join", self.name))
                .and_then(|result| {
                    result.with_context(|| format!("{} background task failed", self.name))
                });
            if first_failure.is_none() {
                first_failure = result.err();
            }
        }
        match first_failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

struct OutputTaskGroup {
    cancellation: CancellationToken,
    task: JoinHandle<TaskOutcome>,
}

impl OutputTaskGroup {
    async fn shutdown(self) -> anyhow::Result<()> {
        self.cancellation.cancel();
        self.task
            .await
            .context("output background task failed to join")?
            .context("output background task failed")
    }
}

pub(in crate::runtime) struct CapabilitySupervisors {
    root_cancellation: CancellationToken,
    output: OutputTaskGroup,
    control_inputs: TaskGroup,
    matter: TaskGroup,
    runtime_tasks: TaskGroup,
}

impl CapabilitySupervisors {
    pub(in crate::runtime) fn start(
        root_cancellation: CancellationToken,
        output_cancellation: CancellationToken,
        scheduler: output_scheduler::OutputScheduler,
        state: &AppState,
    ) -> Self {
        let mut matter = TaskGroup::new("Matter", root_cancellation.child_token());
        matter.spawn(matter_bridge_sync(state.clone(), matter.cancellation()));

        let mut control_inputs = TaskGroup::new("control-input", root_cancellation.child_token());
        for task in control_input_tasks(state, control_inputs.cancellation()) {
            control_inputs.spawn(task);
        }

        let mut runtime_tasks = TaskGroup::new("runtime-owned", root_cancellation.child_token());
        if let Some(receiver) = state.lifecycle.take_task_receiver() {
            runtime_tasks.spawn(drive_owned_tasks(runtime_tasks.cancellation(), receiver));
        }

        Self {
            root_cancellation,
            output: OutputTaskGroup {
                cancellation: output_cancellation,
                task: tokio::spawn(scheduler.into_task()),
            },
            control_inputs,
            matter,
            runtime_tasks,
        }
    }

    pub(in crate::runtime) fn cancellation(&self) -> CancellationToken {
        self.root_cancellation.clone()
    }

    pub(in crate::runtime) async fn shutdown(self) -> anyhow::Result<()> {
        self.root_cancellation.cancel();
        let (output, control_inputs, matter, runtime_tasks) = tokio::join!(
            self.output.shutdown(),
            self.control_inputs.shutdown(),
            self.matter.shutdown(),
            self.runtime_tasks.shutdown(),
        );
        output?;
        control_inputs?;
        matter?;
        runtime_tasks?;
        Ok(())
    }
}

pub(in crate::runtime) async fn drive_owned_tasks(
    cancellation: CancellationToken,
    mut receiver: tokio::sync::mpsc::Receiver<OwnedRuntimeTask>,
) -> anyhow::Result<()> {
    let mut tasks = tokio::task::JoinSet::<TaskOutcome>::new();
    loop {
        tokio::select! {
            biased;
            completed = tasks.join_next(), if !tasks.is_empty() => {
                completed
                    .expect("non-empty JoinSet returns one task")
                    .context("runtime-owned task failed to join")?
                    .context("runtime-owned task failed")?;
            }
            _ = cancellation.cancelled() => {
                tasks.abort_all();
                while let Some(result) = tasks.join_next().await {
                    if let Err(error) = result
                        && !error.is_cancelled()
                    {
                        return Err(anyhow::Error::new(error))
                            .context("runtime-owned task failed to join");
                    }
                }
                return Ok(());
            }
            task = receiver.recv() => match task {
                Some(task) => {
                    tasks.spawn(task);
                }
                None if tasks.is_empty() => return Ok(()),
                None => {
                    while let Some(result) = tasks.join_next().await {
                        result
                            .context("runtime-owned task failed to join")?
                            .context("runtime-owned task failed")?;
                    }
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[tokio::test]
    async fn task_group_cancels_and_awaits_owned_tasks() {
        let cancellation = CancellationToken::new();
        let stopped = Arc::new(AtomicBool::new(false));
        let mut group = TaskGroup::new("test", cancellation);
        let task_cancellation = group.cancellation();
        let task_stopped = Arc::clone(&stopped);
        group.track(tokio::spawn(async move {
            task_cancellation.cancelled().await;
            task_stopped.store(true, Ordering::SeqCst);
            Ok(())
        }));

        group.shutdown().await.unwrap();

        assert!(stopped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn task_group_surfaces_task_failure() {
        let mut group = TaskGroup::new("fallible", CancellationToken::new());
        group.track(tokio::spawn(async {
            Err(anyhow::anyhow!("intentional task failure"))
        }));

        let error = group.shutdown().await.unwrap_err();

        assert!(
            error
                .to_string()
                .contains("fallible background task failed")
        );
        assert!(
            format!("{error:#}").contains("intentional task failure"),
            "{error:#}"
        );
    }

    #[tokio::test]
    async fn task_group_surfaces_task_panic() {
        let mut group = TaskGroup::new("panicking", CancellationToken::new());
        group.track(tokio::spawn(async {
            panic!("intentional task panic");
            #[allow(unreachable_code)]
            Ok(())
        }));

        let error = group.shutdown().await.unwrap_err();

        assert!(
            error
                .to_string()
                .contains("panicking background task failed to join")
        );
    }

    #[tokio::test]
    async fn owned_task_driver_aborts_and_awaits_tasks_on_cancellation() {
        struct Dropped(Arc<AtomicBool>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let cancellation = CancellationToken::new();
        let (sender, receiver) = tokio::sync::mpsc::channel::<OwnedRuntimeTask>(1);
        let dropped = Arc::new(AtomicBool::new(false));
        let task_dropped = Arc::clone(&dropped);
        sender
            .try_send(Box::pin(async move {
                let _dropped = Dropped(task_dropped);
                std::future::pending::<()>().await;
                Ok(())
            }))
            .unwrap();
        let driver = tokio::spawn(drive_owned_tasks(cancellation.clone(), receiver));
        tokio::task::yield_now().await;

        cancellation.cancel();
        driver.await.unwrap().unwrap();

        assert!(dropped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn owned_task_driver_surfaces_task_failure() {
        let cancellation = CancellationToken::new();
        let (sender, receiver) = tokio::sync::mpsc::channel::<OwnedRuntimeTask>(1);
        sender
            .try_send(Box::pin(async {
                Err(anyhow::anyhow!("owned task failed intentionally"))
            }))
            .unwrap();

        let error = drive_owned_tasks(cancellation, receiver).await.unwrap_err();

        assert!(format!("{error:#}").contains("owned task failed intentionally"));
    }

    #[tokio::test]
    async fn owned_task_driver_surfaces_task_panic() {
        let cancellation = CancellationToken::new();
        let (sender, receiver) = tokio::sync::mpsc::channel::<OwnedRuntimeTask>(1);
        sender
            .try_send(Box::pin(async {
                panic!("owned task panicked intentionally");
                #[allow(unreachable_code)]
                Ok(())
            }))
            .unwrap();

        let error = drive_owned_tasks(cancellation, receiver).await.unwrap_err();

        assert!(
            error
                .to_string()
                .contains("runtime-owned task failed to join")
        );
    }
}
