//! Persistent GPU frame and capture buffers owned by the retained host.

use super::{GpuPrimitive, RetainedResourceCounters};

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
