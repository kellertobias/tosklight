use super::super::visualization_frame;
use super::*;

impl OutputResource {
    pub(in crate::runtime) fn latest_visualization_frame(
        &self,
    ) -> Option<Arc<visualization_frame::PublishedVisualizationFrame>> {
        self.visualization_frames.latest()
    }

    /// Pending (Preload) Position readouts come only from this slot, never from Live frames.
    pub(in crate::runtime) fn pending_position_readouts(
        &self,
    ) -> &crate::runtime::position_readout::PendingPositionReadoutSlot {
        &self.pending_position_readouts
    }

    pub(in crate::runtime) fn sampled_visualization_frame(
        &self,
    ) -> Option<Arc<visualization_frame::PublishedVisualizationFrame>> {
        self.visualization_frames.sampled()
    }

    pub(in crate::runtime) async fn wait_for_visualization_sample_after(
        &self,
        sequence: u64,
    ) -> Arc<visualization_frame::PublishedVisualizationFrame> {
        self.visualization_frames
            .wait_for_sample_after(sequence)
            .await
    }

    pub(in crate::runtime) fn visualization_frame_hub(
        &self,
    ) -> Arc<visualization_frame::VisualizationFrameHub> {
        Arc::clone(&self.visualization_frames)
    }

    pub(in crate::runtime) fn visualization_projection(
        &self,
        key: visualization_frame::VisualizationProjectionKey,
        source: &visualization_frame::PublishedVisualizationFrame,
        build: impl FnOnce(
            bool,
        )
            -> Result<light_wire::v2::visualization::VisualizationLaneSnapshot, ApiError>,
    ) -> Result<Arc<visualization_frame::ProjectedVisualizationFrame>, ApiError> {
        self.visualization_frames.projection(key, source, build)
    }

    pub(in crate::runtime) fn change_visualization_subscribers(
        &self,
        lane: light_wire::v2::visualization::VisualizationLane,
        delta: i8,
    ) {
        self.visualization_frames.change_subscribers(lane, delta);
    }

    pub(in crate::runtime) fn change_visualization_projection_claim(
        &self,
        key: visualization_frame::VisualizationProjectionKey,
        delta: i8,
    ) {
        self.visualization_frames
            .change_projection_claim(key, delta);
    }

    pub(in crate::runtime) fn visualization_metrics(
        &self,
    ) -> visualization_frame::VisualizationMetrics {
        self.visualization_frames.metrics()
    }

    pub(in crate::runtime) fn record_visualization_snapshot_route(
        &self,
        projection_duration: Duration,
        serialization_duration: Duration,
        payload_bytes: u64,
        source: Option<&visualization_frame::PublishedVisualizationFrame>,
    ) {
        self.visualization_frames.record_snapshot_route(
            projection_duration,
            serialization_duration,
            payload_bytes,
            source,
        );
    }

    pub(in crate::runtime) fn record_visualization_stream_serialization(
        &self,
        duration: Duration,
        payload_bytes: u64,
    ) {
        self.visualization_frames
            .record_stream_serialization(duration, payload_bytes);
    }

    pub(in crate::runtime) fn record_visualization_stream_queue_push(
        &self,
        replaced_pending: bool,
    ) {
        self.visualization_frames
            .record_stream_queue_push(replaced_pending);
    }

    pub(in crate::runtime) fn record_visualization_stream_queue_take(&self) {
        self.visualization_frames.record_stream_queue_take();
    }

    pub(in crate::runtime) fn record_visualization_stream_send(
        &self,
        duration: Duration,
        succeeded: bool,
    ) {
        self.visualization_frames
            .record_stream_send(duration, succeeded);
    }
}
