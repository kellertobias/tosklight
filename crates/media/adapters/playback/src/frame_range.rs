//! A layer's In and Out points resolved against one clip.

use std::time::Duration;

/// A layer's In and Out points resolved against one clip, as inclusive frame indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRange {
    pub first: usize,
    pub last: usize,
}

impl FrameRange {
    /// Resolves the wire In and Out points against a clip of `frame_count` frames.
    ///
    /// The In point counts frames from the clip's start and the Out point counts frames back from
    /// its end, so zero on both is the whole clip. An In point past the clip's last frame clamps to
    /// that frame. An Out point that would end the range before its In point, including one longer
    /// than the clip, plays through to the clip's end.
    pub fn resolve(in_point: u16, out_point: u16, frame_count: usize) -> Self {
        let end = frame_count.saturating_sub(1);
        let first = usize::from(in_point).min(end);
        let last = end
            .checked_sub(usize::from(out_point))
            .filter(|last| *last >= first)
            .unwrap_or(end);
        Self { first, last }
    }

    /// Resolves In and Out points counted at `frames_per_second` against a clip's own timeline.
    ///
    /// A point is a time: the In point is the frame showing `in_point / fps` seconds into the
    /// clip, and the range ends before the frame that would start `out_point / fps` seconds before
    /// the clip's end. A clip whose own rate equals the point rate resolves exactly as
    /// [`FrameRange::resolve`] does; any other clip still starts and stops at the same times.
    /// The clamping and play-through rules are the same as for frame counts.
    pub fn resolve_timed(
        in_point: u16,
        out_point: u16,
        frames_per_second: u8,
        presentation_micros: &[u64],
        duration: Duration,
    ) -> Self {
        // Absorbs integer rounding between the clip's timestamps and the point rate; far below
        // one frame at any accepted rate.
        const TOLERANCE_MICROS: u64 = 1_000;
        let fps = u64::from(frames_per_second.max(1));
        let micros = |frames: u16| u64::from(frames) * 1_000_000 / fps;
        let end = presentation_micros.len().saturating_sub(1);
        let in_micros = micros(in_point) + TOLERANCE_MICROS;
        let first = presentation_micros
            .partition_point(|&timestamp| timestamp <= in_micros)
            .saturating_sub(1)
            .min(end);
        let last = if out_point == 0 {
            end
        } else {
            let length = u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
            length
                .checked_sub(micros(out_point))
                .and_then(|stop| {
                    presentation_micros
                        .partition_point(|&timestamp| timestamp + TOLERANCE_MICROS < stop)
                        .checked_sub(1)
                })
                .filter(|last| *last >= first)
                .unwrap_or(end)
        };
        Self { first, last }
    }

    /// The whole clip.
    pub const fn full(frame_count: usize) -> Self {
        Self {
            first: 0,
            last: frame_count.saturating_sub(1),
        }
    }

    pub(crate) const fn clamp(self, frame: usize) -> usize {
        if frame < self.first {
            self.first
        } else if frame > self.last {
            self.last
        } else {
            frame
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_points_match_frame_counts_at_the_clips_own_rate() {
        let timings: Vec<u64> = (0..10).map(|index| index * 100_000).collect();
        let duration = Duration::from_secs(1);
        for in_point in 0..12 {
            for out_point in 0..12 {
                assert_eq!(
                    FrameRange::resolve_timed(in_point, out_point, 10, &timings, duration),
                    FrameRange::resolve(in_point, out_point, 10),
                    "in {in_point} out {out_point}"
                );
            }
        }
    }

    #[test]
    fn timed_points_keep_their_time_in_a_clip_of_another_rate() {
        // Two seconds at 10 fps, programmed at 25 fps: In 00:00.10 is 0.4 s, frame 4; Out
        // 00:00.25 before the end stops before 1.0 s, on frame 9.
        let timings: Vec<u64> = (0..20).map(|index| index * 100_000).collect();
        let duration = Duration::from_secs(2);
        assert_eq!(
            FrameRange::resolve_timed(10, 25, 25, &timings, duration),
            FrameRange { first: 4, last: 9 }
        );
        // A point between two frames starts on the frame already showing.
        assert_eq!(
            FrameRange::resolve_timed(6, 0, 25, &timings, duration).first,
            2
        );
        // An Out point longer than the clip plays through; an In point past it holds the end.
        assert_eq!(
            FrameRange::resolve_timed(0, 60_000, 25, &timings, duration),
            FrameRange::full(20)
        );
        assert_eq!(
            FrameRange::resolve_timed(60_000, 0, 25, &timings, duration).first,
            19
        );
    }
}
