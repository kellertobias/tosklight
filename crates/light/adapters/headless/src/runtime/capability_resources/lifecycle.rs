use super::*;

#[derive(Clone)]
pub(in crate::runtime) struct LifecycleResource {
    shutdown: CancellationToken,
    task_sender: tokio::sync::mpsc::Sender<OwnedRuntimeTask>,
    task_receiver: Arc<Mutex<Option<tokio::sync::mpsc::Receiver<OwnedRuntimeTask>>>>,
}

pub(super) const RUNTIME_TASK_QUEUE_CAPACITY: usize = 256;

pub(in crate::runtime) type OwnedRuntimeTask =
    std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'static>>;

impl LifecycleResource {
    pub(in crate::runtime) fn new(shutdown: CancellationToken) -> Self {
        let (task_sender, task_receiver) = tokio::sync::mpsc::channel(RUNTIME_TASK_QUEUE_CAPACITY);
        Self {
            shutdown,
            task_sender,
            task_receiver: Arc::new(Mutex::new(Some(task_receiver))),
        }
    }

    #[cfg(test)]
    pub(in crate::runtime) fn cancellation_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    pub(in crate::runtime) fn request_shutdown(&self) {
        self.shutdown.cancel();
    }

    pub(in crate::runtime) fn schedule(
        &self,
        task: impl std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
    ) -> Result<(), anyhow::Error> {
        self.task_sender
            .try_send(Box::pin(task))
            .map_err(|error| match error {
                tokio::sync::mpsc::error::TrySendError::Full(_) => {
                    anyhow::anyhow!("runtime task supervisor queue is full")
                }
                tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                    anyhow::anyhow!("runtime task supervisor is unavailable")
                }
            })
    }

    pub(in crate::runtime) fn take_task_receiver(
        &self,
    ) -> Option<tokio::sync::mpsc::Receiver<OwnedRuntimeTask>> {
        self.task_receiver.lock().take()
    }

    #[cfg(test)]
    pub(in crate::runtime) fn is_shutdown_requested(&self) -> bool {
        self.shutdown.is_cancelled()
    }
}
