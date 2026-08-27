//! Cuelist-clip translation between the Timecode wire contract and the playback domain.

use light_playback::TimecodeFrame;
use light_wire::v2::timecode as wire;

pub(super) fn domain_cue_clip(
    clip: wire::TimecodeCueListClip,
) -> light_playback::TimecodeCueListClip {
    use light_playback as domain;
    domain::TimecodeCueListClip {
        id: domain::TimecodeClipId(clip.id),
        start_frame: TimecodeFrame(clip.start_frame),
        end_frame: TimecodeFrame(clip.end_frame),
        start_cue_id: clip.start_cue_id,
        end_cue_id: clip.end_cue_id,
        start_behavior: match clip.start_behavior {
            wire::TimecodeClipStart::State => domain::TimecodeClipStart::State,
            wire::TimecodeClipStart::Cue => domain::TimecodeClipStart::Cue,
        },
        end_behavior: match clip.end_behavior {
            wire::TimecodeClipEnd::Release => domain::TimecodeClipEnd::Release,
            wire::TimecodeClipEnd::Hold => domain::TimecodeClipEnd::Hold,
        },
        cue_starts: clip
            .cue_starts
            .into_iter()
            .map(|placed| domain::TimecodeCueStart {
                cue_id: placed.cue_id,
                offset_frame: TimecodeFrame(placed.offset_frame),
            })
            .collect(),
        in_fade_frames: clip.in_fade_frames,
        out_fade_frames: clip.out_fade_frames,
    }
}

pub(super) fn wire_cue_clip(
    clip: light_playback::TimecodeCueListClip,
) -> wire::TimecodeCueListClip {
    use light_playback as domain;
    wire::TimecodeCueListClip {
        id: clip.id.0,
        start_frame: clip.start_frame.0,
        end_frame: clip.end_frame.0,
        start_cue_id: clip.start_cue_id,
        end_cue_id: clip.end_cue_id,
        start_behavior: match clip.start_behavior {
            domain::TimecodeClipStart::State => wire::TimecodeClipStart::State,
            domain::TimecodeClipStart::Cue => wire::TimecodeClipStart::Cue,
        },
        end_behavior: match clip.end_behavior {
            domain::TimecodeClipEnd::Release => wire::TimecodeClipEnd::Release,
            domain::TimecodeClipEnd::Hold => wire::TimecodeClipEnd::Hold,
        },
        cue_starts: clip
            .cue_starts
            .into_iter()
            .map(|placed| wire::TimecodeCueStart {
                cue_id: placed.cue_id,
                offset_frame: placed.offset_frame.0,
            })
            .collect(),
        in_fade_frames: clip.in_fade_frames,
        out_fade_frames: clip.out_fade_frames,
    }
}
