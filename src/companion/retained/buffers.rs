//! Persistent GPU frame and capture buffers owned by the retained host.

use super::{GpuPrimitive, RetainedResourceCounters};

/// A byte range within one logical persistent mirror buffer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ByteSpan {
    pub(super) offset: usize,
    pub(super) len: usize,
}

#[allow(dead_code)] // The retained delta uploader consumes byte spans in the next checkpoint.
impl ByteSpan {
    pub(super) const fn slots<T>(first: usize, count: usize) -> Self {
        Self {
            offset: first * std::mem::size_of::<T>(),
            len: count * std::mem::size_of::<T>(),
        }
    }

    const fn end(self) -> usize {
        self.offset + self.len
    }
}

/// Fixed dirty-range scratch for one logical GPU buffer.
///
/// More than 64 fragmented writes remain valid: the set conservatively bridges
/// an unchanged gap instead of growing or rejecting a same-generation delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DirtySpanSet {
    spans: [ByteSpan; Self::CAPACITY],
    len: usize,
}

impl Default for DirtySpanSet {
    fn default() -> Self {
        Self {
            spans: [ByteSpan::default(); Self::CAPACITY],
            len: 0,
        }
    }
}

#[allow(dead_code)] // The retained delta uploader consumes dirty spans in the next checkpoint.
impl DirtySpanSet {
    pub(super) const CAPACITY: usize = 64;

    #[allow(dead_code)] // Read by the retained delta uploader in the next checkpoint.
    pub(super) fn as_slice(&self) -> &[ByteSpan] {
        &self.spans[..self.len]
    }

    pub(super) fn insert(&mut self, span: ByteSpan) {
        if span.len == 0 {
            return;
        }

        let mut start = span.offset;
        let mut end = span.end();
        let mut first = 0;
        while first < self.len && self.spans[first].end() < start {
            first += 1;
        }
        let mut after = first;
        while after < self.len && self.spans[after].offset <= end {
            start = start.min(self.spans[after].offset);
            end = end.max(self.spans[after].end());
            after += 1;
        }

        if first < after {
            self.spans[first] = ByteSpan { offset: start, len: end - start };
            self.spans.copy_within(after..self.len, first + 1);
            self.len -= after - first - 1;
            return;
        }

        if self.len < Self::CAPACITY {
            self.spans.copy_within(first..self.len, first + 1);
            self.spans[first] = span;
            self.len += 1;
            return;
        }

        // The valid write would create a 65th fragment. Bridge it to the
        // nearest existing range, then absorb any ranges the bridge touches.
        let previous_gap = first
            .checked_sub(1)
            .map(|index| start.saturating_sub(self.spans[index].end()));
        let next_gap = (first < self.len).then(|| self.spans[first].offset.saturating_sub(end));
        let bridge = match (previous_gap, next_gap) {
            (Some(previous), Some(next)) if previous <= next => first - 1,
            (Some(_), Some(_)) => first,
            (Some(_), None) => first - 1,
            (None, Some(_)) => first,
            (None, None) => unreachable!("full span set has a neighbor"),
        };
        let bridged = ByteSpan {
            offset: start.min(self.spans[bridge].offset),
            len: end.max(self.spans[bridge].end()) - start.min(self.spans[bridge].offset),
        };
        self.spans[bridge] = bridged;
        self.coalesce_around(bridge);
    }

    fn coalesce_around(&mut self, mut index: usize) {
        if index > 0 && self.spans[index - 1].end() >= self.spans[index].offset {
            let start = self.spans[index - 1].offset;
            let end = self.spans[index - 1].end().max(self.spans[index].end());
            self.spans[index - 1] = ByteSpan { offset: start, len: end - start };
            self.spans.copy_within(index + 1..self.len, index);
            self.len -= 1;
            index -= 1;
        }
        while index + 1 < self.len && self.spans[index].end() >= self.spans[index + 1].offset {
            let end = self.spans[index].end().max(self.spans[index + 1].end());
            self.spans[index].len = end - self.spans[index].offset;
            self.spans.copy_within(index + 2..self.len, index + 1);
            self.len -= 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // The retained delta uploader reports mirror overflow in the next checkpoint.
pub(super) enum MirrorError {
    CapacityExceeded,
}

/// Fixed-capacity POD storage for one logical persistent GPU buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FixedPodMirror<T, const N: usize> {
    values: [T; N],
}

impl<T: bytemuck::Pod + bytemuck::Zeroable + Copy, const N: usize> FixedPodMirror<T, N> {
    pub(super) fn zeroed() -> Self {
        Self { values: [T::zeroed(); N] }
    }

    pub(super) const fn from_array(values: [T; N]) -> Self {
        Self { values }
    }

    #[allow(dead_code)] // Capacity assertions precede retained-host integration.
    pub(super) const fn capacity(&self) -> usize {
        N
    }

    #[allow(dead_code)] // Read by the retained delta uploader in the next checkpoint.
    pub(super) fn as_slice(&self) -> &[T] {
        &self.values
    }

    pub(super) fn set_fixed(&mut self, slot: usize, value: T) {
        debug_assert!(slot < N);
        self.values[slot] = value;
    }

    #[allow(dead_code)] // The retained delta uploader applies slot changes in the next checkpoint.
    pub(super) fn apply_updates(
        &mut self,
        updates: &[(usize, T)],
    ) -> Result<DirtySpanSet, MirrorError> {
        if updates.iter().any(|(slot, _)| *slot >= N) {
            return Err(MirrorError::CapacityExceeded);
        }
        let mut spans = DirtySpanSet::default();
        for (slot, _) in updates {
            spans.insert(ByteSpan::slots::<T>(*slot, 1));
        }
        for (slot, value) in updates {
            self.values[*slot] = *value;
        }
        Ok(spans)
    }
}

/// Instance buffers held in a small ring so a frame writes into a slot the GPU is
/// unlikely to still be reading from the previous present, avoiding a
/// write-vs-read stall without CPU-side fences.
pub(super) const INSTANCE_RING_LEN: usize = 3;

/// One instance's stride in the persistent buffer.
pub(super) const INSTANCE_STRIDE: usize = std::mem::size_of::<GpuPrimitive>();

/// Maximum primitive count observed by the deterministic full current-renderer
/// fixture matrix. The fixed minimum ring adds explicit non-growth headroom;
/// larger generation requests remain possible but are not ordinary frames.
pub(super) const FIXED_INSTANCE_RING_MIN: usize = 1_024;

/// A capacity-bounded ring of persistent `VERTEX | COPY_DST` instance buffers.
///
/// A frame stages its instances into the next ring slot through the reusable
/// upload belt and draws only that slot's current instance count. The ring grows —
/// every buffer reallocated to the larger capacity — only when a frame's instance
/// count exceeds the current capacity, which is a declared layout/semantic change,
/// not ordinary motion. Once warmed to the steady-state high-water mark, ordinary
/// animation reuses the buffers and only writes, so no buffer is ever recreated.
pub(super) struct PersistentFrameBuffers {
    ring: Vec<wgpu::Buffer>,
    pub(super) capacity_instances: usize,
    pub(super) cursor: usize,
    staging_belt: wgpu::util::StagingBelt,
}

impl PersistentFrameBuffers {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        Self {
            ring: Vec::new(),
            capacity_instances: 0,
            cursor: 0,
            staging_belt: wgpu::util::StagingBelt::new(
                device.clone(),
                (FIXED_INSTANCE_RING_MIN * INSTANCE_STRIDE) as wgpu::BufferAddress,
            ),
        }
    }

    /// Guarantees every ring buffer can hold at least `instances` instances.
    /// Reallocates the whole ring — counting one buffer creation per slot — only
    /// when the request exceeds the current capacity; a request at or below the
    /// current capacity reuses the existing buffers and creates nothing.
    pub(super) fn ensure_instance_capacity(
        &mut self,
        instances: usize,
        device: &wgpu::Device,
        counters: &mut RetainedResourceCounters,
    ) {
        if !self.ring.is_empty() && instances <= self.capacity_instances {
            return;
        }
        let capacity = persistent_instance_capacity(instances);
        let size = (capacity * INSTANCE_STRIDE) as wgpu::BufferAddress;
        self.ring = (0..INSTANCE_RING_LEN)
            .map(|_| {
                counters.buffer_creations += 1;
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("glorp-retained-instances"),
                    size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        self.capacity_instances = capacity;
        self.cursor = 0;
    }

    /// Writes `instances` into the next ring slot and records the write. The
    /// caller must have called [`Self::ensure_instance_capacity`] for at least
    /// `instances.len()` first. `instance_writes`/`instance_write_bytes` advance
    /// only after the staging copy is encoded.
    pub(super) fn write_frame_instances(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        instances: &[GpuPrimitive],
        counters: &mut RetainedResourceCounters,
    ) {
        debug_assert!(
            !self.ring.is_empty() && instances.len() <= self.capacity_instances,
            "write_frame_instances requires ensure_instance_capacity first",
        );
        self.cursor = (self.cursor + 1) % self.ring.len();
        let bytes: &[u8] = bytemuck::cast_slice(instances);
        counters.instance_writes += 1;
        counters.instance_write_bytes += bytes.len() as u64;
        if bytes.is_empty() {
            return;
        }
        assert_eq!(bytes.len() as u64 % wgpu::COPY_BUFFER_ALIGNMENT, 0);
        let size = wgpu::BufferSize::new(bytes.len() as u64).expect("nonzero instance upload");
        let mut view = self
            .staging_belt
            .write_buffer(encoder, &self.ring[self.cursor], 0, size);
        view.copy_from_slice(bytes);
        drop(view);
    }

    pub(super) fn finish_uploads(&mut self) {
        self.staging_belt.finish();
    }

    pub(super) fn recall_uploads(&mut self) {
        self.staging_belt.recall();
    }

    /// The ring buffer holding the current frame's instances.
    pub(super) fn current_buffer(&self) -> &wgpu::Buffer {
        &self.ring[self.cursor]
    }
}

pub(super) fn persistent_instance_capacity(instances: usize) -> usize {
    if instances <= FIXED_INSTANCE_RING_MIN {
        FIXED_INSTANCE_RING_MIN
    } else {
        instances.next_power_of_two()
    }
}

/// The off-screen capture intermediate and its mappable staging buffer, keyed by
/// the physical size and surface format they were built for. A resize or
/// backing-scale change replaces them once; ordinary captures reuse them.
pub(super) struct PersistentCaptureResources {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) format: wgpu::TextureFormat,
    pub(super) intermediate: wgpu::Texture,
    pub(super) intermediate_view: wgpu::TextureView,
    pub(super) staging: wgpu::Buffer,
}

#[cfg(test)]
mod fixed_mirror_tests {
    use super::*;
    use bytemuck::{Pod, Zeroable};

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
    struct Word(u32);

    #[test]
    fn adjacent_overlap_and_zero_dirty_spans_coalesce() {
        let mut mirror = FixedPodMirror::<Word, 8>::zeroed();
        assert!(mirror.apply_updates(&[]).unwrap().as_slice().is_empty());
        let spans = mirror
            .apply_updates(&[(2, Word(10)), (3, Word(11)), (3, Word(12)), (6, Word(13))])
            .unwrap();
        assert_eq!(
            spans.as_slice(),
            &[ByteSpan::slots::<Word>(2, 2), ByteSpan::slots::<Word>(6, 1)]
        );
    }

    #[test]
    fn out_of_range_update_leaves_fixed_mirror_unchanged() {
        let mut mirror = FixedPodMirror::<Word, 2>::zeroed();
        let before = mirror.clone();
        assert_eq!(
            mirror.apply_updates(&[(2, Word(1))]),
            Err(MirrorError::CapacityExceeded)
        );
        assert_eq!(mirror, before);
        assert_eq!(mirror.capacity(), 2);
    }

    #[test]
    fn fragmented_writes_bridge_gaps_without_growing_span_storage() {
        let mut mirror = FixedPodMirror::<Word, 130>::zeroed();
        let updates = (0..65)
            .map(|index| (index * 2, Word(index as u32)))
            .collect::<Vec<_>>();
        let spans = mirror.apply_updates(&updates).unwrap();
        assert_eq!(spans.as_slice().len(), DirtySpanSet::CAPACITY);
        assert!(spans
            .as_slice()
            .windows(2)
            .all(|pair| pair[0].end() < pair[1].offset));
        for (slot, value) in &updates {
            assert_eq!(mirror.as_slice()[*slot], *value);
            let changed = ByteSpan::slots::<Word>(*slot, 1);
            assert!(spans
                .as_slice()
                .iter()
                .any(|span| { span.offset <= changed.offset && span.end() >= changed.end() }));
        }
    }
}
