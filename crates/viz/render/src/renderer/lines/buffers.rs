//! Changed ranges only: retained GPU instance/guide buffers survive camera-only frames.
use crate::buffers::DynamicBuffer;
use bytemuck::Pod;

pub(super) fn upload_changed<T: Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &mut DynamicBuffer,
    previous: &mut Vec<u8>,
    data: &[T],
) -> bool {
    let bytes = bytemuck::cast_slice(data);
    let grew = buffer.ensure(device, bytes.len() as u64);
    buffer.length = data.len() as u32;
    if grew || previous.len() != bytes.len() {
        if !bytes.is_empty() {
            queue.write_buffer(&buffer.buffer, 0, bytes);
        }
    } else {
        for range in changed_ranges(previous, bytes, size_of::<T>()) {
            queue.write_buffer(&buffer.buffer, range.start as u64, &bytes[range]);
        }
    }
    previous.clear();
    previous.extend_from_slice(bytes);
    grew
}

fn changed_ranges(previous: &[u8], next: &[u8], stride: usize) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, (a, b)) in previous
        .chunks_exact(stride)
        .zip(next.chunks_exact(stride))
        .enumerate()
    {
        if a != b {
            start.get_or_insert(index * stride);
        } else if let Some(from) = start.take() {
            ranges.push(from..index * stride);
        }
    }
    if let Some(from) = start {
        ranges.push(from..next.len());
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::changed_ranges;
    #[test]
    fn only_contiguous_changed_instances_are_uploaded() {
        assert!(changed_ranges(&[0; 16], &[0; 16], 4).is_empty());
        assert_eq!(
            changed_ranges(
                &[0; 16],
                &[1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0],
                4
            ),
            vec![0..8, 12..16]
        );
    }
}
