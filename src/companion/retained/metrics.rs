use serde::Serialize;
use std::time::Duration;

pub(crate) const METRIC_SAMPLE_CAPACITY: usize = 4_096;

#[derive(Debug, Clone)]
pub(crate) struct FixedSamples<const N: usize> {
    values: [u32; N],
    len: usize,
    next: usize,
}

impl<const N: usize> Default for FixedSamples<N> {
    fn default() -> Self {
        assert!(N > 0, "FixedSamples requires non-zero capacity");
        Self { values: [0; N], len: 0, next: 0 }
    }
}

impl<const N: usize> FixedSamples<N> {
    pub(crate) fn push(&mut self, value: u32) {
        self.values[self.next] = value;
        self.next = (self.next + 1) % N;
        self.len = self.len.saturating_add(1).min(N);
    }

    fn sorted_values(&self) -> Vec<u32> {
        let mut values = self.values[..self.len].to_vec();
        values.sort_unstable();
        values
    }

    fn percentile(&self, percentile: u32) -> Option<u32> {
        let values = self.sorted_values();
        if values.is_empty() {
            return None;
        }
        let percentile = percentile.clamp(1, 100) as usize;
        let rank = percentile.saturating_mul(values.len()).div_ceil(100);
        values.get(rank.saturating_sub(1)).copied()
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct RuntimeIdentity {
    pub device_epoch: u64,
    pub surface_epoch: u64,
    pub layout_generation: u64,
    pub resource_generation: u64,
    pub semantic_revision: u64,
    pub frame_revision: u64,
}

impl RuntimeIdentity {
    #[cfg(test)]
    pub(crate) const fn baseline() -> Self {
        Self {
            device_epoch: 1,
            surface_epoch: 1,
            layout_generation: 1,
            resource_generation: 1,
            semantic_revision: 0,
            frame_revision: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct Percentiles {
    pub p50: Option<u32>,
    pub p95: Option<u32>,
    pub p99: Option<u32>,
}

impl Percentiles {
    fn from_samples<const N: usize>(samples: &FixedSamples<N>) -> Self {
        Self {
            p50: samples.percentile(50),
            p95: samples.percentile(95),
            p99: samples.percentile(99),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub(crate) struct CompanionCapacityInventory {
    pub max_nodes: u32,
    pub max_static_primitives: u32,
    pub max_pet_slots: u32,
    pub max_visible_props: u32,
    pub max_round_tank_inhabitants: u32,
    pub max_ambient_instances: u32,
    pub max_blended_draws: u32,
    pub max_lights: u32,
    pub max_attachments: u32,
}

impl CompanionCapacityInventory {
    /// Frozen Stage 0 inventory exercised by the full deterministic Preview Lab
    /// matrix. Later scene tasks replace these upper-bound fixture observations
    /// with direct snapshot counts without changing the serialized contract.
    pub(crate) const fn full_preview_fixture() -> Self {
        Self {
            max_nodes: 128,
            max_static_primitives: 768,
            max_pet_slots: 130,
            max_visible_props: 10,
            max_round_tank_inhabitants: 2,
            max_ambient_instances: 64,
            max_blended_draws: 256,
            max_lights: 2,
            max_attachments: 32,
        }
    }

    pub(crate) const fn fits_global_constraints(self) -> bool {
        self.max_nodes <= 128
            && self.max_static_primitives <= 768
            && self.max_pet_slots <= 130
            && self.max_visible_props <= 10
            && self.max_round_tank_inhabitants <= 2
            && self.max_ambient_instances <= 64
            && self.max_blended_draws <= 256
            && self.max_lights <= 2
            && self.max_attachments <= 32
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CompanionRuntimeMetricsSnapshot {
    pub schema_version: u32,
    pub identity: RuntimeIdentity,
    pub sample_capacity: usize,
    pub visible_samples: usize,
    pub ui_tick_us: Percentiles,
    pub prepare_us: Percentiles,
    pub encode_us: Percentiles,
    pub queue_wait_us: Percentiles,
    pub compile_us: Percentiles,
    pub activation_us: Percentiles,
    pub generation_count: u64,
    pub coalesced_updates: u64,
    pub cancellations: u64,
    pub stale_rejections: u64,
    pub static_upload_bytes: u64,
    pub dynamic_upload_bytes: u64,
    pub queue_writes: u64,
    pub draw_calls: u64,
    pub persistent_gpu_objects_created: u64,
    pub persistent_gpu_objects_destroyed: u64,
    pub hidden_ticks: u64,
    pub prepare_count: u64,
    pub surface_acquires: u64,
    pub encode_count: u64,
    pub submit_count: u64,
    pub skipped_frames: u64,
    pub fallback_count: u64,
    pub capture_count: u64,
    pub node_high_water: u32,
    pub primitive_high_water: u32,
    pub blended_draw_high_water: u32,
    pub cpu_bytes_high_water: u64,
    pub gpu_bytes_high_water: u64,
    pub metrics_overhead_us_high_water: u32,
    pub inventory: CompanionCapacityInventory,
}

#[derive(Debug, Clone)]
pub(crate) struct CompanionRuntimeMetrics {
    ui_tick_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    prepare_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    encode_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    queue_wait_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    compile_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    activation_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    generation_count: u64,
    coalesced_updates: u64,
    cancellations: u64,
    stale_rejections: u64,
    static_upload_bytes: u64,
    dynamic_upload_bytes: u64,
    queue_writes: u64,
    draw_calls: u64,
    persistent_gpu_objects_created: u64,
    persistent_gpu_objects_destroyed: u64,
    hidden_ticks: u64,
    prepare_count: u64,
    surface_acquires: u64,
    encode_count: u64,
    submit_count: u64,
    skipped_frames: u64,
    fallback_count: u64,
    capture_count: u64,
    node_high_water: u32,
    primitive_high_water: u32,
    blended_draw_high_water: u32,
    cpu_bytes_high_water: u64,
    gpu_bytes_high_water: u64,
    visible_warmup_remaining: u32,
    sample_current_tick: bool,
    reset_steady_state_after_warmup: bool,
    metrics_overhead_ns_current_tick: u64,
    metrics_overhead_ns_high_water: u64,
}

impl Default for CompanionRuntimeMetrics {
    fn default() -> Self {
        Self {
            ui_tick_us: FixedSamples::default(),
            prepare_us: FixedSamples::default(),
            encode_us: FixedSamples::default(),
            queue_wait_us: FixedSamples::default(),
            compile_us: FixedSamples::default(),
            activation_us: FixedSamples::default(),
            generation_count: 0,
            coalesced_updates: 0,
            cancellations: 0,
            stale_rejections: 0,
            static_upload_bytes: 0,
            dynamic_upload_bytes: 0,
            queue_writes: 0,
            draw_calls: 0,
            persistent_gpu_objects_created: 0,
            persistent_gpu_objects_destroyed: 0,
            hidden_ticks: 0,
            prepare_count: 0,
            surface_acquires: 0,
            encode_count: 0,
            submit_count: 0,
            skipped_frames: 0,
            fallback_count: 0,
            capture_count: 0,
            node_high_water: 0,
            primitive_high_water: 0,
            blended_draw_high_water: 0,
            cpu_bytes_high_water: 0,
            gpu_bytes_high_water: 0,
            visible_warmup_remaining: 0,
            sample_current_tick: true,
            reset_steady_state_after_warmup: false,
            metrics_overhead_ns_current_tick: 0,
            metrics_overhead_ns_high_water: 0,
        }
    }
}

impl CompanionRuntimeMetrics {
    pub(crate) fn discard_initial_visible_ticks(&mut self, count: u32) {
        self.visible_warmup_remaining = count;
        self.sample_current_tick = count == 0;
        self.reset_steady_state_after_warmup = count > 0;
    }

    pub(crate) fn begin_visible_tick(&mut self) {
        self.metrics_overhead_ns_high_water = self
            .metrics_overhead_ns_high_water
            .max(self.metrics_overhead_ns_current_tick);
        self.metrics_overhead_ns_current_tick = 0;
        if self.visible_warmup_remaining > 0 {
            self.visible_warmup_remaining -= 1;
            self.sample_current_tick = false;
        } else {
            if self.reset_steady_state_after_warmup {
                self.static_upload_bytes = 0;
                self.dynamic_upload_bytes = 0;
                self.queue_writes = 0;
                self.draw_calls = 0;
                self.persistent_gpu_objects_created = 0;
                self.persistent_gpu_objects_destroyed = 0;
                self.prepare_count = 0;
                self.surface_acquires = 0;
                self.encode_count = 0;
                self.submit_count = 0;
                self.skipped_frames = 0;
                self.fallback_count = 0;
                self.capture_count = 0;
                self.reset_steady_state_after_warmup = false;
            }
            self.sample_current_tick = true;
        }
    }

    pub(crate) fn record_ui_tick_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.ui_tick_us.push(value);
        }
    }

    pub(crate) fn record_prepare_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.prepare_us.push(value);
            increment(&mut self.prepare_count, 1);
        }
    }

    pub(crate) fn record_encode_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.encode_us.push(value);
            increment(&mut self.encode_count, 1);
        }
    }

    pub(crate) fn record_queue_wait_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.queue_wait_us.push(value);
        }
    }

    pub(crate) fn record_compile_us(&mut self, value: u32) {
        self.compile_us.push(value);
        increment(&mut self.generation_count, 1);
    }

    pub(crate) fn record_activation_us(&mut self, value: u32) {
        self.activation_us.push(value);
    }

    pub(crate) fn record_queue_write(&mut self, bytes: u64) {
        if self.sample_current_tick {
            increment(&mut self.queue_writes, 1);
            increment(&mut self.dynamic_upload_bytes, bytes);
        }
    }

    pub(crate) fn record_static_upload(&mut self, bytes: u64) {
        increment(&mut self.static_upload_bytes, bytes);
    }

    pub(crate) fn record_draws(&mut self, draws: u64) {
        if self.sample_current_tick {
            increment(&mut self.draw_calls, draws);
        }
    }

    pub(crate) fn record_persistent_gpu_create(&mut self, count: u64) {
        increment(&mut self.persistent_gpu_objects_created, count);
    }

    #[allow(dead_code)] // Task 9 records generation retirement through this fixed counter.
    pub(crate) fn record_persistent_gpu_destroy(&mut self, count: u64) {
        increment(&mut self.persistent_gpu_objects_destroyed, count);
    }

    pub(crate) fn record_hidden_tick(&mut self) {
        increment(&mut self.hidden_ticks, 1);
    }

    pub(crate) fn record_surface_acquire(&mut self) {
        if self.sample_current_tick {
            increment(&mut self.surface_acquires, 1);
        }
    }

    pub(crate) fn record_submit(&mut self) {
        if self.sample_current_tick {
            increment(&mut self.submit_count, 1);
        }
    }

    pub(crate) fn record_skip(&mut self) {
        if self.sample_current_tick {
            increment(&mut self.skipped_frames, 1);
        }
    }

    pub(crate) fn record_fallback(&mut self) {
        increment(&mut self.fallback_count, 1);
    }

    pub(crate) fn record_capture(&mut self) {
        increment(&mut self.capture_count, 1);
    }

    pub(crate) fn record_metrics_overhead(&mut self, duration: Duration) {
        let nanos = duration.as_nanos().min(u128::from(u64::MAX)) as u64;
        increment(&mut self.metrics_overhead_ns_current_tick, nanos);
    }

    #[cfg_attr(not(test), allow(dead_code))] // Task 2 supplies direct scene-node observations.
    pub(crate) fn observe_nodes(&mut self, value: u32) {
        self.node_high_water = self.node_high_water.max(value);
    }

    pub(crate) fn observe_primitives(&mut self, value: u32) {
        self.primitive_high_water = self.primitive_high_water.max(value);
    }

    pub(crate) fn observe_blended_draws(&mut self, value: u32) {
        self.blended_draw_high_water = self.blended_draw_high_water.max(value);
    }

    pub(crate) fn observe_cpu_bytes(&mut self, value: u64) {
        self.cpu_bytes_high_water = self.cpu_bytes_high_water.max(value);
    }

    pub(crate) fn observe_gpu_bytes(&mut self, value: u64) {
        self.gpu_bytes_high_water = self.gpu_bytes_high_water.max(value);
    }

    pub(crate) fn snapshot(&self, identity: RuntimeIdentity) -> CompanionRuntimeMetricsSnapshot {
        CompanionRuntimeMetricsSnapshot {
            schema_version: 1,
            identity,
            sample_capacity: METRIC_SAMPLE_CAPACITY,
            visible_samples: self.ui_tick_us.len(),
            ui_tick_us: Percentiles::from_samples(&self.ui_tick_us),
            prepare_us: Percentiles::from_samples(&self.prepare_us),
            encode_us: Percentiles::from_samples(&self.encode_us),
            queue_wait_us: Percentiles::from_samples(&self.queue_wait_us),
            compile_us: Percentiles::from_samples(&self.compile_us),
            activation_us: Percentiles::from_samples(&self.activation_us),
            generation_count: self.generation_count,
            coalesced_updates: self.coalesced_updates,
            cancellations: self.cancellations,
            stale_rejections: self.stale_rejections,
            static_upload_bytes: self.static_upload_bytes,
            dynamic_upload_bytes: self.dynamic_upload_bytes,
            queue_writes: self.queue_writes,
            draw_calls: self.draw_calls,
            persistent_gpu_objects_created: self.persistent_gpu_objects_created,
            persistent_gpu_objects_destroyed: self.persistent_gpu_objects_destroyed,
            hidden_ticks: self.hidden_ticks,
            prepare_count: self.prepare_count,
            surface_acquires: self.surface_acquires,
            encode_count: self.encode_count,
            submit_count: self.submit_count,
            skipped_frames: self.skipped_frames,
            fallback_count: self.fallback_count,
            capture_count: self.capture_count,
            node_high_water: self.node_high_water,
            primitive_high_water: self.primitive_high_water,
            blended_draw_high_water: self.blended_draw_high_water,
            cpu_bytes_high_water: self.cpu_bytes_high_water,
            gpu_bytes_high_water: self.gpu_bytes_high_water,
            metrics_overhead_us_high_water: self
                .metrics_overhead_ns_high_water
                .max(self.metrics_overhead_ns_current_tick)
                .div_ceil(1_000)
                .min(u64::from(u32::MAX)) as u32,
            inventory: CompanionCapacityInventory::full_preview_fixture(),
        }
    }
}

pub(crate) fn duration_us(duration: Duration) -> u32 {
    duration.as_micros().min(u128::from(u32::MAX)) as u32
}

fn increment(counter: &mut u64, amount: u64) {
    *counter = counter.saturating_add(amount);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_samples_overwrite_oldest_and_report_percentiles() {
        let mut samples = FixedSamples::<4>::default();
        for value in [10, 20, 30, 40, 50] {
            samples.push(value);
        }
        assert_eq!(samples.sorted_values(), vec![20, 30, 40, 50]);
        assert_eq!(samples.percentile(50), Some(30));
        assert_eq!(samples.percentile(95), Some(50));
        assert_eq!(samples.percentile(99), Some(50));
    }

    #[test]
    fn snapshot_carries_epochs_counters_and_high_water_marks() {
        let mut metrics = CompanionRuntimeMetrics::default();
        metrics.record_ui_tick_us(1_500);
        metrics.record_encode_us(800);
        metrics.record_persistent_gpu_create(3);
        metrics.observe_nodes(72);
        let snapshot = metrics.snapshot(RuntimeIdentity::baseline());
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.ui_tick_us.p95, Some(1_500));
        assert_eq!(snapshot.encode_us.p99, Some(800));
        assert_eq!(snapshot.persistent_gpu_objects_created, 3);
        assert_eq!(snapshot.node_high_water, 72);
    }

    #[test]
    fn full_preview_inventory_fits_frozen_scene_limits() {
        assert!(CompanionCapacityInventory::full_preview_fixture().fits_global_constraints());
    }
}
