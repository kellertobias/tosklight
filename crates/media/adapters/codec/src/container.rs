//! The `.toskclip` container.
//!
//! A flat header, the frame payloads back to back, then an index of every frame's offset, length,
//! and presentation timestamp. Random access is a lookup and a read — which is what makes reverse,
//! bounce, a speed change, and a frame-exact seek ordinary rather than the hardest part of
//! playback.
//!
//! The index sits at the end because a writer only knows each payload's size once it has written
//! it; the header carries its offset, patched when the clip is finished. A file whose header still
//! says the index is missing was never finished, and is rejected rather than half-read.

use std::io::{Read, Seek, SeekFrom, Write};

use media_domain::timeline::MediaTiming;

use crate::cache::ResidentClip;

/// Identifies the format, and catches a file that is something else entirely.
pub const MAGIC: [u8; 8] = *b"TOSKCLIP";

/// The format version this build writes. A reader refuses anything newer rather than guessing.
pub const VERSION: u16 = 1;

const HEADER_BYTES: u64 = 64;
const INDEX_ENTRY_BYTES: usize = 20;

/// Byte offsets within the header. Named so the writer and the reader cannot drift apart, which
/// is exactly the mistake these constants were introduced to fix.
mod field {
    pub const VERSION: usize = 8;
    pub const WIDTH: usize = 12;
    pub const HEIGHT: usize = 16;
    pub const FRAME_COUNT: usize = 20;
    pub const RATE_NUMERATOR: usize = 24;
    pub const RATE_DENOMINATOR: usize = 28;
    pub const INTRINSIC_BPM: usize = 32;
    pub const LAST_FRAME: usize = 40;
    pub const DURATION: usize = 48;
    pub const INDEX_OFFSET: usize = 56;
}

/// What a clip file says about itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipHeader {
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    /// Nominal frame rate, as a rational so 30000/1001 stays exact.
    pub frame_rate: (u32, u32),
    /// The tempo the asset was authored at, when it has one. Parsed at import from a filename
    /// token and stored here; runtime never re-infers it.
    pub intrinsic_bpm: Option<f64>,
}

impl ClipHeader {
    /// The timing the playback timeline needs.
    ///
    /// The last frame comes from the index rather than from arithmetic on the duration, so Once
    /// holds the frame that actually exists whatever the container's padding.
    pub fn timing(&self, last_frame_micros: u64, duration_micros: u64) -> MediaTiming {
        MediaTiming {
            duration: std::time::Duration::from_micros(duration_micros),
            last_frame: std::time::Duration::from_micros(last_frame_micros),
            intrinsic_bpm: self.intrinsic_bpm,
        }
    }

    fn frame_interval_micros(&self) -> u64 {
        let (numerator, denominator) = self.frame_rate;
        if numerator == 0 {
            return 0;
        }
        (1_000_000u64 * u64::from(denominator)) / u64::from(numerator)
    }
}

/// One frame's place in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameEntry {
    pub offset: u64,
    pub length: u32,
    pub presentation_micros: u64,
}

/// Why a clip file cannot be read.
#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("this is not a ToskLight clip file")]
    NotAClip,
    #[error("the clip is version {found}, but this build reads up to version {VERSION}")]
    FromTheFuture { found: u16 },
    #[error("the clip was never finished writing and has no frame index")]
    Unfinished,
    #[error("the clip's index claims frame {frame} runs past the end of the file")]
    IndexPastEnd { frame: u32 },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Writes a clip.
///
/// Frames are appended in presentation order; `finish` writes the index and patches the header.
/// Dropping a writer without finishing leaves a file that readers reject, which is the honest
/// outcome for an import that was interrupted.
#[derive(Debug)]
pub struct ClipWriter<W: Write + Seek> {
    sink: W,
    header: ClipHeader,
    index: Vec<FrameEntry>,
    position: u64,
}

impl<W: Write + Seek> ClipWriter<W> {
    pub fn new(mut sink: W, header: ClipHeader) -> Result<Self, ContainerError> {
        // A placeholder header, rewritten by `finish` once the counts are known.
        sink.write_all(&[0u8; HEADER_BYTES as usize])?;
        Ok(Self {
            sink,
            header,
            index: Vec::new(),
            position: HEADER_BYTES,
        })
    }

    /// Appends one encoded frame.
    pub fn write_frame(
        &mut self,
        payload: &[u8],
        presentation_micros: u64,
    ) -> Result<(), ContainerError> {
        self.sink.write_all(payload)?;
        self.index.push(FrameEntry {
            offset: self.position,
            length: payload.len() as u32,
            presentation_micros,
        });
        self.position += payload.len() as u64;
        Ok(())
    }

    /// Writes the index and the real header.
    pub fn finish(mut self) -> Result<W, ContainerError> {
        let index_offset = self.position;
        for entry in &self.index {
            self.sink.write_all(&entry.offset.to_le_bytes())?;
            self.sink.write_all(&entry.length.to_le_bytes())?;
            self.sink
                .write_all(&entry.presentation_micros.to_le_bytes())?;
        }

        let frame_count = self.index.len() as u32;
        let last_frame = self
            .index
            .last()
            .map_or(0, |entry| entry.presentation_micros);
        let duration = last_frame + self.header.frame_interval_micros();

        let mut header = Vec::with_capacity(HEADER_BYTES as usize);
        header.extend_from_slice(&MAGIC);
        header.extend_from_slice(&VERSION.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes()); // flags, reserved
        header.extend_from_slice(&self.header.width.to_le_bytes());
        header.extend_from_slice(&self.header.height.to_le_bytes());
        header.extend_from_slice(&frame_count.to_le_bytes());
        header.extend_from_slice(&self.header.frame_rate.0.to_le_bytes());
        header.extend_from_slice(&self.header.frame_rate.1.to_le_bytes());
        header.extend_from_slice(&self.header.intrinsic_bpm.unwrap_or(0.0).to_le_bytes());
        header.extend_from_slice(&last_frame.to_le_bytes());
        header.extend_from_slice(&duration.to_le_bytes());
        header.extend_from_slice(&index_offset.to_le_bytes());
        header.resize(HEADER_BYTES as usize, 0);

        self.sink.seek(SeekFrom::Start(0))?;
        self.sink.write_all(&header)?;
        self.sink.flush()?;
        Ok(self.sink)
    }
}

/// Reads a clip.
#[derive(Debug)]
pub struct ClipReader<R: Read + Seek> {
    source: R,
    header: ClipHeader,
    index: Vec<FrameEntry>,
    last_frame_micros: u64,
    duration_micros: u64,
}

impl<R: Read + Seek> ClipReader<R> {
    pub fn open(mut source: R) -> Result<Self, ContainerError> {
        let mut header = [0u8; HEADER_BYTES as usize];
        source.seek(SeekFrom::Start(0))?;
        source
            .read_exact(&mut header)
            .map_err(|_| ContainerError::NotAClip)?;

        if header[0..8] != MAGIC {
            return Err(ContainerError::NotAClip);
        }
        let version = u16::from_le_bytes([header[field::VERSION], header[field::VERSION + 1]]);
        if version > VERSION {
            return Err(ContainerError::FromTheFuture { found: version });
        }

        let read_u32 = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
        let read_u64 = |at: usize| u64::from_le_bytes(header[at..at + 8].try_into().unwrap());

        let frame_count = read_u32(field::FRAME_COUNT);
        let index_offset = read_u64(field::INDEX_OFFSET);
        // An unfinished writer never patched the header, so the index offset is still zero.
        if index_offset == 0 && frame_count > 0 {
            return Err(ContainerError::Unfinished);
        }

        let bpm = f64::from_le_bytes(
            header[field::INTRINSIC_BPM..field::INTRINSIC_BPM + 8]
                .try_into()
                .unwrap(),
        );
        let parsed = ClipHeader {
            width: read_u32(field::WIDTH),
            height: read_u32(field::HEIGHT),
            frame_count,
            frame_rate: (
                read_u32(field::RATE_NUMERATOR),
                read_u32(field::RATE_DENOMINATOR),
            ),
            intrinsic_bpm: (bpm > 0.0).then_some(bpm),
        };

        let mut raw = vec![0u8; frame_count as usize * INDEX_ENTRY_BYTES];
        source.seek(SeekFrom::Start(index_offset))?;
        source
            .read_exact(&mut raw)
            .map_err(|_| ContainerError::Unfinished)?;

        let (raw_entries, remainder) = raw.as_chunks::<INDEX_ENTRY_BYTES>();
        debug_assert!(remainder.is_empty());
        let index = raw_entries
            .iter()
            .map(|entry| FrameEntry {
                offset: u64::from_le_bytes(entry[0..8].try_into().unwrap()),
                length: u32::from_le_bytes(entry[8..12].try_into().unwrap()),
                presentation_micros: u64::from_le_bytes(entry[12..20].try_into().unwrap()),
            })
            .collect::<Vec<_>>();

        for (position, entry) in index.iter().enumerate() {
            if entry.offset + u64::from(entry.length) > index_offset {
                return Err(ContainerError::IndexPastEnd {
                    frame: position as u32,
                });
            }
        }

        Ok(Self {
            source,
            header: parsed,
            index,
            last_frame_micros: read_u64(field::LAST_FRAME),
            duration_micros: read_u64(field::DURATION),
        })
    }

    pub const fn header(&self) -> &ClipHeader {
        &self.header
    }

    pub fn index(&self) -> &[FrameEntry] {
        &self.index
    }

    /// The timing the playback timeline needs, taken from the index rather than from arithmetic.
    pub fn timing(&self) -> MediaTiming {
        self.header
            .timing(self.last_frame_micros, self.duration_micros)
    }

    /// Reads one frame's payload. Any frame, in any order, at the same cost.
    pub fn frame(&mut self, index: usize) -> Result<Option<Vec<u8>>, ContainerError> {
        let Some(entry) = self.index.get(index).copied() else {
            return Ok(None);
        };
        let mut payload = vec![0u8; entry.length as usize];
        self.source.seek(SeekFrom::Start(entry.offset))?;
        self.source.read_exact(&mut payload)?;
        Ok(Some(payload))
    }

    /// Reads every frame into memory, ready for the resident cache.
    pub fn read_resident(&mut self) -> Result<ResidentClip, ContainerError> {
        let mut frames = Vec::with_capacity(self.index.len());
        for position in 0..self.index.len() {
            let payload = self.frame(position)?.expect("the index bounds this loop");
            frames.push(std::sync::Arc::from(payload.into_boxed_slice()));
        }
        Ok(ResidentClip::new(frames))
    }

    /// The frame showing at a position in the clip: the last one whose presentation timestamp has
    /// arrived. This is the lookup every play mode ends in.
    pub fn frame_at(&self, position: std::time::Duration) -> Option<usize> {
        let micros = position.as_micros() as u64;
        match self
            .index
            .binary_search_by_key(&micros, |entry| entry.presentation_micros)
        {
            Ok(exact) => Some(exact),
            // Before the first frame there is nothing to show yet.
            Err(0) => None,
            Err(after) => Some(after - 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::Duration;

    use super::*;

    fn header() -> ClipHeader {
        ClipHeader {
            width: 64,
            height: 32,
            frame_count: 0,
            frame_rate: (10, 1),
            intrinsic_bpm: None,
        }
    }

    /// Ten frames at 10 fps, each payload distinguishable by its length and contents.
    fn write_clip(header: ClipHeader, frames: usize) -> Vec<u8> {
        let mut writer = ClipWriter::new(Cursor::new(Vec::new()), header).unwrap();
        for index in 0..frames {
            let payload = vec![index as u8; 8 + index];
            writer
                .write_frame(&payload, index as u64 * 100_000)
                .unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn a_clip_round_trips_through_the_container() {
        let bytes = write_clip(header(), 10);
        let mut reader = ClipReader::open(Cursor::new(bytes)).unwrap();

        assert_eq!(reader.header().width, 64);
        assert_eq!(reader.header().height, 32);
        assert_eq!(reader.header().frame_count, 10);
        assert_eq!(reader.header().frame_rate, (10, 1));

        for index in 0..10usize {
            let payload = reader.frame(index).unwrap().expect("frame exists");
            assert_eq!(payload.len(), 8 + index);
            assert!(
                payload.iter().all(|byte| *byte == index as u8),
                "frame {index} content"
            );
        }
        assert!(reader.frame(10).unwrap().is_none(), "past the end");
    }

    #[test]
    fn frames_read_in_any_order_which_is_what_reverse_and_bounce_need() {
        let mut reader = ClipReader::open(Cursor::new(write_clip(header(), 10))).unwrap();
        for index in [9usize, 0, 5, 9, 1, 8] {
            let payload = reader.frame(index).unwrap().unwrap();
            assert!(
                payload.iter().all(|byte| *byte == index as u8),
                "frame {index}"
            );
        }
    }

    #[test]
    fn timing_comes_from_the_index_rather_than_from_arithmetic() {
        let reader = ClipReader::open(Cursor::new(write_clip(header(), 10))).unwrap();
        let timing = reader.timing();
        assert_eq!(timing.last_frame, Duration::from_millis(900));
        assert_eq!(timing.duration, Duration::from_millis(1_000));
        assert!(
            timing.last_frame < timing.duration,
            "Once has a real frame to hold"
        );
    }

    #[test]
    fn a_non_integer_frame_rate_stays_exact() {
        let ntsc = ClipHeader {
            frame_rate: (30_000, 1_001),
            ..header()
        };
        let reader = ClipReader::open(Cursor::new(write_clip(ntsc, 3))).unwrap();
        assert_eq!(reader.header().frame_rate, (30_000, 1_001));
    }

    #[test]
    fn an_intrinsic_tempo_survives_and_its_absence_stays_absent() {
        let plain = ClipReader::open(Cursor::new(write_clip(header(), 2))).unwrap();
        assert_eq!(plain.timing().intrinsic_bpm, None);

        let authored = ClipHeader {
            intrinsic_bpm: Some(119.95),
            ..header()
        };
        let reader = ClipReader::open(Cursor::new(write_clip(authored, 2))).unwrap();
        assert_eq!(reader.timing().intrinsic_bpm, Some(119.95));
    }

    #[test]
    fn a_position_resolves_to_the_frame_that_is_showing() {
        let reader = ClipReader::open(Cursor::new(write_clip(header(), 10))).unwrap();
        assert_eq!(reader.frame_at(Duration::ZERO), Some(0));
        assert_eq!(
            reader.frame_at(Duration::from_millis(99)),
            Some(0),
            "still frame 0"
        );
        assert_eq!(reader.frame_at(Duration::from_millis(100)), Some(1));
        assert_eq!(reader.frame_at(Duration::from_millis(899)), Some(8));
        assert_eq!(reader.frame_at(Duration::from_millis(900)), Some(9));
        assert_eq!(
            reader.frame_at(Duration::from_secs(99)),
            Some(9),
            "clamps to the last frame"
        );
    }

    #[test]
    fn a_whole_clip_loads_into_the_resident_cache() {
        let mut reader = ClipReader::open(Cursor::new(write_clip(header(), 10))).unwrap();
        let resident = reader.read_resident().unwrap();
        assert_eq!(resident.frame_count(), 10);
        // 8, 9, 10 ... 17 bytes.
        assert_eq!(resident.bytes(), (8..18).sum::<usize>() as u64);
        assert_eq!(resident.frame(3).unwrap().len(), 11);
    }

    #[test]
    fn an_empty_clip_is_readable_rather_than_an_error() {
        let mut reader = ClipReader::open(Cursor::new(write_clip(header(), 0))).unwrap();
        assert_eq!(reader.header().frame_count, 0);
        assert!(reader.frame(0).unwrap().is_none());
        assert_eq!(reader.frame_at(Duration::ZERO), None);
        assert_eq!(reader.read_resident().unwrap().frame_count(), 0);
    }

    #[test]
    fn something_that_is_not_a_clip_is_refused() {
        let error =
            ClipReader::open(Cursor::new(b"not a clip at all, just bytes".to_vec())).unwrap_err();
        assert!(matches!(error, ContainerError::NotAClip), "{error}");

        let tiny = ClipReader::open(Cursor::new(vec![0u8; 4])).unwrap_err();
        assert!(matches!(tiny, ContainerError::NotAClip), "{tiny}");
    }

    #[test]
    fn a_newer_version_is_refused_rather_than_guessed_at() {
        let mut bytes = write_clip(header(), 2);
        bytes[super::field::VERSION..super::field::VERSION + 2]
            .copy_from_slice(&(VERSION + 1).to_le_bytes());
        let error = ClipReader::open(Cursor::new(bytes)).unwrap_err();
        assert!(matches!(error, ContainerError::FromTheFuture { found } if found == VERSION + 1));
    }

    #[test]
    fn an_interrupted_import_leaves_a_file_readers_reject() {
        // A writer that never finished: frames on disk, header still blank.
        let mut writer = ClipWriter::new(Cursor::new(Vec::new()), header()).unwrap();
        writer.write_frame(&[1, 2, 3], 0).unwrap();
        let bytes = writer.sink.into_inner();

        let error = ClipReader::open(Cursor::new(bytes)).unwrap_err();
        assert!(matches!(error, ContainerError::NotAClip), "{error}");
    }

    #[test]
    fn an_index_pointing_past_the_payloads_is_refused() {
        let mut bytes = write_clip(header(), 3);
        let index_offset = u64::from_le_bytes(
            bytes[super::field::INDEX_OFFSET..super::field::INDEX_OFFSET + 8]
                .try_into()
                .unwrap(),
        ) as usize;
        // Push the first entry's length past where the payloads end.
        bytes[index_offset + 8..index_offset + 12].copy_from_slice(&u32::MAX.to_le_bytes());

        let error = ClipReader::open(Cursor::new(bytes)).unwrap_err();
        assert!(
            matches!(error, ContainerError::IndexPastEnd { frame: 0 }),
            "{error}"
        );
    }
}
