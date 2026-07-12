use serde::Serialize;
use std::time::Duration;

#[cfg(test)]
use crate::presentation::companion_scene::scene::{
    MAX_AMBIENT_INSTANCES, MAX_ATTACHMENTS, MAX_BLENDED_DRAWS, MAX_LIGHTS, MAX_PET_ART_SLOTS,
    MAX_ROUND_TANK_INHABITANTS, MAX_SCENE_NODES, MAX_STATIC_PRIMITIVES, MAX_VISIBLE_PROPS,
};

pub(crate) const METRIC_SAMPLE_CAPACITY: usize = 4_096;
pub(crate) const APPKIT_RASTER_DIAGNOSTIC_CAPACITY: usize = 256;

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

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub(crate) struct AppkitRasterSliceDiagnostic {
    pub start_cursor: u32,
    pub end_cursor: u32,
    pub items_completed: u32,
    pub elapsed_us: u32,
    pub setup_us: u32,
    pub max_item_us: u32,
    pub max_item_index: Option<u32>,
}

#[derive(Debug, Clone)]
struct FixedAppkitRasterDiagnostics<const N: usize> {
    values: [AppkitRasterSliceDiagnostic; N],
    len: usize,
    next: usize,
}

impl<const N: usize> Default for FixedAppkitRasterDiagnostics<N> {
    fn default() -> Self {
        assert!(
            N > 0,
            "FixedAppkitRasterDiagnostics requires non-zero capacity"
        );
        Self {
            values: [AppkitRasterSliceDiagnostic::default(); N],
            len: 0,
            next: 0,
        }
    }
}

impl<const N: usize> FixedAppkitRasterDiagnostics<N> {
    fn push(&mut self, value: AppkitRasterSliceDiagnostic) {
        self.values[self.next] = value;
        self.next = (self.next + 1) % N;
        self.len = self.len.saturating_add(1).min(N);
    }

    fn chronological_values(&self) -> Vec<AppkitRasterSliceDiagnostic> {
        if self.len < N {
            return self.values[..self.len].to_vec();
        }
        self.values[self.next..]
            .iter()
            .chain(self.values[..self.next].iter())
            .copied()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct RuntimeIdentity {
    pub device_epoch: Option<u64>,
    pub surface_epoch: u64,
    pub layout_generation: Option<u64>,
    pub resource_generation: Option<u64>,
    pub semantic_revision: Option<u64>,
    pub frame_revision: Option<u64>,
    pub present_attempt: u64,
}

impl RuntimeIdentity {
    #[cfg(test)]
    pub(crate) const fn baseline() -> Self {
        Self {
            device_epoch: None,
            surface_epoch: 1,
            layout_generation: None,
            resource_generation: Some(1),
            semantic_revision: None,
            frame_revision: None,
            present_attempt: 0,
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
pub(crate) struct CapacityContract {
    pub observed: Option<u32>,
    pub reservation: u32,
    pub headroom: u32,
    pub limit: u32,
}

impl CapacityContract {
    pub(crate) const fn observed(value: u32, headroom: u32, limit: u32) -> Self {
        Self {
            observed: Some(value),
            reservation: 0,
            headroom,
            limit,
        }
    }

    pub(crate) const fn reserved(reservation: u32, headroom: u32, limit: u32) -> Self {
        Self {
            observed: None,
            reservation,
            headroom,
            limit,
        }
    }

    pub(crate) const fn fits(self) -> bool {
        let observed = match self.observed {
            Some(value) => value,
            None => 0,
        };
        observed
            .saturating_add(self.reservation)
            .saturating_add(self.headroom)
            <= self.limit
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub(crate) struct CompanionCapacityInventory {
    pub matrix_fixture_count: u32,
    pub dimmed_fixture_count: u32,
    pub full_props_tank_fixture_count: u32,
    pub max_prepared_gpu_primitives: CapacityContract,
    pub max_nodes: CapacityContract,
    pub max_static_primitives: CapacityContract,
    pub max_pet_slots: CapacityContract,
    pub max_visible_props: CapacityContract,
    pub max_round_tank_inhabitants: CapacityContract,
    pub max_ambient_instances: CapacityContract,
    pub max_blended_draws: CapacityContract,
    pub max_lights: CapacityContract,
    pub max_attachments: CapacityContract,
}

impl CompanionCapacityInventory {
    #[cfg(test)]
    pub(crate) const fn contract_fixture() -> Self {
        Self {
            matrix_fixture_count: 630,
            dimmed_fixture_count: 126,
            full_props_tank_fixture_count: 630,
            max_prepared_gpu_primitives: CapacityContract::observed(782, 242, 1_024),
            max_nodes: CapacityContract::reserved(96, 32, MAX_SCENE_NODES as u32),
            max_static_primitives: CapacityContract::reserved(
                640,
                128,
                MAX_STATIC_PRIMITIVES as u32,
            ),
            max_pet_slots: CapacityContract::observed(96, 34, MAX_PET_ART_SLOTS as u32),
            max_visible_props: CapacityContract::observed(8, 2, MAX_VISIBLE_PROPS as u32),
            max_round_tank_inhabitants: CapacityContract::observed(
                2,
                0,
                MAX_ROUND_TANK_INHABITANTS as u32,
            ),
            max_ambient_instances: CapacityContract::reserved(48, 16, MAX_AMBIENT_INSTANCES as u32),
            max_blended_draws: CapacityContract::reserved(192, 64, MAX_BLENDED_DRAWS as u32),
            max_lights: CapacityContract::reserved(1, 1, MAX_LIGHTS as u32),
            max_attachments: CapacityContract::reserved(16, 16, MAX_ATTACHMENTS as u32),
        }
    }

    pub(crate) const fn fits_global_constraints(self) -> bool {
        self.max_prepared_gpu_primitives.fits()
            && self.max_nodes.fits()
            && self.max_static_primitives.fits()
            && self.max_pet_slots.fits()
            && self.max_visible_props.fits()
            && self.max_round_tank_inhabitants.fits()
            && self.max_ambient_instances.fits()
            && self.max_blended_draws.fits()
            && self.max_lights.fits()
            && self.max_attachments.fits()
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub(crate) struct RuntimeWorkCounters {
    pub prepare: u64,
    pub appkit_raster_slices: u64,
    pub queue_writes: u64,
    pub surface_acquires: u64,
    pub encode: u64,
    pub submit: u64,
}

impl RuntimeWorkCounters {
    fn saturating_sub(self, previous: Self) -> Self {
        Self {
            prepare: self.prepare.saturating_sub(previous.prepare),
            appkit_raster_slices: self
                .appkit_raster_slices
                .saturating_sub(previous.appkit_raster_slices),
            queue_writes: self.queue_writes.saturating_sub(previous.queue_writes),
            surface_acquires: self
                .surface_acquires
                .saturating_sub(previous.surface_acquires),
            encode: self.encode.saturating_sub(previous.encode),
            submit: self.submit.saturating_sub(previous.submit),
        }
    }

    fn saturating_add(self, other: Self) -> Self {
        Self {
            prepare: self.prepare.saturating_add(other.prepare),
            appkit_raster_slices: self
                .appkit_raster_slices
                .saturating_add(other.appkit_raster_slices),
            queue_writes: self.queue_writes.saturating_add(other.queue_writes),
            surface_acquires: self.surface_acquires.saturating_add(other.surface_acquires),
            encode: self.encode.saturating_add(other.encode),
            submit: self.submit.saturating_add(other.submit),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub(crate) struct HiddenSegmentSnapshot {
    pub transition_ticks: u64,
    pub steady_ticks: u64,
    pub steady_delta: RuntimeWorkCounters,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub(crate) struct GpuByteBreakdown {
    pub atlas_bytes: u64,
    pub instance_ring_bytes: u64,
    pub capture_bytes: u64,
    pub total_bytes: u64,
}

impl GpuByteBreakdown {
    fn with_total(mut self) -> Self {
        self.total_bytes = self
            .atlas_bytes
            .saturating_add(self.instance_ring_bytes)
            .saturating_add(self.capture_bytes);
        self
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub(crate) struct GpuObjectBreakdown {
    pub host_infrastructure: u64,
    pub atlas: u64,
    pub instance_ring: u64,
    pub capture: u64,
    pub total_objects: u64,
}

impl GpuObjectBreakdown {
    fn with_total(mut self) -> Self {
        self.total_objects = self
            .host_infrastructure
            .saturating_add(self.atlas)
            .saturating_add(self.instance_ring)
            .saturating_add(self.capture);
        self
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub(crate) struct GpuAccountingSnapshot {
    pub current_bytes: GpuByteBreakdown,
    pub peak_total_bytes: u64,
    pub current_objects: GpuObjectBreakdown,
    pub peak_total_objects: u64,
    pub objects_created_total: u64,
    pub objects_destroyed_total: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum GpuAllocationKind {
    HostInfrastructure,
    Atlas,
    InstanceRing,
    Capture,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub(crate) struct LifetimeAuditSnapshot {
    pub frames: u64,
    pub warmup_frames: u64,
    pub cadence_ms: u64,
    pub virtual_elapsed_ms: u64,
    pub prepared_frames: u64,
    pub encoded_frames: u64,
    pub semantic_frame_changes: u64,
    pub gpu_frame_hash_changes: u64,
    pub draw_calls: u64,
    pub poll_count: u64,
    pub rss_warmup_bytes: u64,
    pub rss_warmup_peak_bytes: u64,
    pub rss_final_bytes: u64,
    pub rss_peak_bytes: u64,
    pub gpu_warmup_bytes: u64,
    pub gpu_warmup_peak_bytes: u64,
    pub gpu_final_bytes: u64,
    pub gpu_peak_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub(crate) struct MetricsOverheadAudit {
    pub iterations: u64,
    pub trials: u32,
    pub control_ticks: u64,
    pub instrumented_ticks: u64,
    pub control_ns_per_tick: u64,
    pub instrumented_ns_per_tick: u64,
    pub net_ns_per_tick: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct RuntimeFixtureIdentity {
    pub fixture_id: &'static str,
    pub seed: &'static str,
    pub update_source: &'static str,
    pub cadence_ms: u64,
    pub logical_width: f64,
    pub logical_height: f64,
    pub physical_width: u32,
    pub physical_height: u32,
    pub backing_scale: f64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CompanionRuntimeMetricsSnapshot {
    pub schema_version: u32,
    pub identity: RuntimeIdentity,
    pub fixture: RuntimeFixtureIdentity,
    pub sample_capacity: usize,
    pub visible_samples: usize,
    pub ui_tick_us: Percentiles,
    pub state_prepare_us: Percentiles,
    pub gpu_translate_us: Percentiles,
    pub encode_us: Percentiles,
    pub queue_wait_us: Percentiles,
    pub compile_us: Percentiles,
    pub appkit_raster_queue_wait_us: Percentiles,
    pub appkit_raster_slice_us: Percentiles,
    pub appkit_raster_total_us: Percentiles,
    pub appkit_raster_slice_count: u64,
    pub appkit_raster_deadline_misses: u64,
    pub appkit_raster_coalesces: u64,
    pub appkit_raster_cancellations: u64,
    pub appkit_raster_diagnostic_capacity: usize,
    pub appkit_raster_slice_diagnostics: Vec<AppkitRasterSliceDiagnostic>,
    pub activation_render_owner_us: Percentiles,
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
    pub capture_attempted: u64,
    pub capture_succeeded: u64,
    pub capture_failed: u64,
    pub node_high_water: u32,
    pub primitive_high_water: u32,
    pub blended_draw_high_water: u32,
    pub cpu_bytes_high_water: u64,
    pub gpu_bytes_high_water: u64,
    pub metrics_overhead_us_high_water: u32,
    pub metrics_overhead_control: MetricsOverheadAudit,
    pub inventory: CompanionCapacityInventory,
    pub hidden_segment: HiddenSegmentSnapshot,
    pub gpu_accounting: GpuAccountingSnapshot,
    pub lifetime_audit: Option<LifetimeAuditSnapshot>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompanionRuntimeMetrics {
    ui_tick_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    state_prepare_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    gpu_translate_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    encode_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    queue_wait_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    compile_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    appkit_raster_queue_wait_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    appkit_raster_slice_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    appkit_raster_total_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    appkit_raster_slice_count: u64,
    appkit_raster_deadline_misses: u64,
    appkit_raster_coalesces: u64,
    appkit_raster_cancellations: u64,
    appkit_raster_slice_diagnostics:
        FixedAppkitRasterDiagnostics<APPKIT_RASTER_DIAGNOSTIC_CAPACITY>,
    activation_render_owner_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
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
    capture_attempted: u64,
    capture_succeeded: u64,
    capture_failed: u64,
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
    hidden_transition_seen: bool,
    hidden_steady_ticks: u64,
    hidden_steady_delta: RuntimeWorkCounters,
    gpu_current: GpuByteBreakdown,
    gpu_peak_total_bytes: u64,
    gpu_objects_current: GpuObjectBreakdown,
    gpu_peak_total_objects: u64,
    gpu_objects_created_total: u64,
    gpu_objects_destroyed_total: u64,
    lifetime_audit: Option<LifetimeAuditSnapshot>,
}

impl Default for CompanionRuntimeMetrics {
    fn default() -> Self {
        Self {
            ui_tick_us: FixedSamples::default(),
            state_prepare_us: FixedSamples::default(),
            gpu_translate_us: FixedSamples::default(),
            encode_us: FixedSamples::default(),
            queue_wait_us: FixedSamples::default(),
            compile_us: FixedSamples::default(),
            appkit_raster_queue_wait_us: FixedSamples::default(),
            appkit_raster_slice_us: FixedSamples::default(),
            appkit_raster_total_us: FixedSamples::default(),
            appkit_raster_slice_count: 0,
            appkit_raster_deadline_misses: 0,
            appkit_raster_coalesces: 0,
            appkit_raster_cancellations: 0,
            appkit_raster_slice_diagnostics: FixedAppkitRasterDiagnostics::default(),
            activation_render_owner_us: FixedSamples::default(),
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
            capture_attempted: 0,
            capture_succeeded: 0,
            capture_failed: 0,
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
            hidden_transition_seen: false,
            hidden_steady_ticks: 0,
            hidden_steady_delta: RuntimeWorkCounters::default(),
            gpu_current: GpuByteBreakdown::default(),
            gpu_peak_total_bytes: 0,
            gpu_objects_current: GpuObjectBreakdown::default(),
            gpu_peak_total_objects: 0,
            gpu_objects_created_total: 0,
            gpu_objects_destroyed_total: 0,
            lifetime_audit: None,
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
                self.capture_attempted = 0;
                self.capture_succeeded = 0;
                self.capture_failed = 0;
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

    pub(crate) fn record_state_prepare_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.state_prepare_us.push(value);
            increment(&mut self.prepare_count, 1);
        }
    }

    pub(crate) fn record_gpu_translate_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.gpu_translate_us.push(value);
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

    pub(crate) fn record_appkit_raster_queue_wait_us(&mut self, value: u32) {
        self.appkit_raster_queue_wait_us.push(value);
    }

    pub(crate) fn record_appkit_raster_slice_us(&mut self, value: u32, deadline_missed: bool) {
        self.appkit_raster_slice_us.push(value);
        increment(&mut self.appkit_raster_slice_count, 1);
        if deadline_missed {
            increment(&mut self.appkit_raster_deadline_misses, 1);
        }
    }

    pub(crate) fn record_appkit_raster_total_us(&mut self, value: u32) {
        self.appkit_raster_total_us.push(value);
    }

    pub(crate) fn record_appkit_raster_coalesce(&mut self) {
        increment(&mut self.appkit_raster_coalesces, 1);
    }

    pub(crate) fn record_appkit_raster_cancellation(&mut self) {
        increment(&mut self.appkit_raster_cancellations, 1);
    }

    pub(crate) fn record_appkit_raster_slice_diagnostic(
        &mut self,
        diagnostic: AppkitRasterSliceDiagnostic,
    ) {
        self.appkit_raster_slice_diagnostics.push(diagnostic);
    }

    pub(crate) fn record_activation_render_owner_us(&mut self, value: u32) {
        self.activation_render_owner_us.push(value);
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

    pub(crate) fn record_hidden_tick(&mut self, tick_start: RuntimeWorkCounters) {
        increment(&mut self.hidden_ticks, 1);
        if self.hidden_transition_seen {
            let delta = self.work_counters().saturating_sub(tick_start);
            self.hidden_steady_delta = self.hidden_steady_delta.saturating_add(delta);
            increment(&mut self.hidden_steady_ticks, 1);
        } else {
            self.hidden_transition_seen = true;
        }
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

    pub(crate) fn record_capture_attempt(&mut self) {
        increment(&mut self.capture_attempted, 1);
    }

    pub(crate) fn record_capture_success(&mut self) {
        increment(&mut self.capture_succeeded, 1);
    }

    pub(crate) fn record_capture_failure(&mut self) {
        increment(&mut self.capture_failed, 1);
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

    pub(crate) fn work_counters(&self) -> RuntimeWorkCounters {
        RuntimeWorkCounters {
            prepare: self.prepare_count,
            appkit_raster_slices: self.appkit_raster_slice_count,
            queue_writes: self.queue_writes,
            surface_acquires: self.surface_acquires,
            encode: self.encode_count,
            submit: self.submit_count,
        }
    }

    pub(crate) fn hidden_segment_snapshot(&self) -> HiddenSegmentSnapshot {
        HiddenSegmentSnapshot {
            transition_ticks: u64::from(self.hidden_transition_seen),
            steady_ticks: self.hidden_steady_ticks,
            steady_delta: self.hidden_steady_delta,
        }
    }

    pub(crate) fn replace_gpu_allocation(
        &mut self,
        kind: GpuAllocationKind,
        bytes: u64,
        objects: u64,
    ) {
        let overlap_peak = self.gpu_current.total_bytes.saturating_add(bytes);
        self.gpu_peak_total_bytes = self.gpu_peak_total_bytes.max(overlap_peak);
        let object_overlap_peak = self
            .gpu_objects_current
            .total_objects
            .saturating_add(objects);
        self.gpu_peak_total_objects = self.gpu_peak_total_objects.max(object_overlap_peak);
        let replaced_objects = match kind {
            GpuAllocationKind::HostInfrastructure => self.gpu_objects_current.host_infrastructure,
            GpuAllocationKind::Atlas => self.gpu_objects_current.atlas,
            GpuAllocationKind::InstanceRing => self.gpu_objects_current.instance_ring,
            GpuAllocationKind::Capture => self.gpu_objects_current.capture,
        };
        self.record_persistent_gpu_create(objects);
        self.record_persistent_gpu_destroy(replaced_objects);
        increment(&mut self.gpu_objects_created_total, objects);
        increment(&mut self.gpu_objects_destroyed_total, replaced_objects);
        match kind {
            GpuAllocationKind::HostInfrastructure => {
                self.gpu_objects_current.host_infrastructure = objects;
            }
            GpuAllocationKind::Atlas => {
                self.gpu_current.atlas_bytes = bytes;
                self.gpu_objects_current.atlas = objects;
            }
            GpuAllocationKind::InstanceRing => {
                self.gpu_current.instance_ring_bytes = bytes;
                self.gpu_objects_current.instance_ring = objects;
            }
            GpuAllocationKind::Capture => {
                self.gpu_current.capture_bytes = bytes;
                self.gpu_objects_current.capture = objects;
            }
        }
        self.gpu_current = self.gpu_current.with_total();
        self.gpu_objects_current = self.gpu_objects_current.with_total();
        self.gpu_peak_total_bytes = self.gpu_peak_total_bytes.max(self.gpu_current.total_bytes);
        self.gpu_peak_total_objects = self
            .gpu_peak_total_objects
            .max(self.gpu_objects_current.total_objects);
        self.observe_gpu_bytes(self.gpu_peak_total_bytes);
    }

    pub(crate) fn gpu_accounting_snapshot(&self) -> GpuAccountingSnapshot {
        GpuAccountingSnapshot {
            current_bytes: self.gpu_current,
            peak_total_bytes: self.gpu_peak_total_bytes,
            current_objects: self.gpu_objects_current,
            peak_total_objects: self.gpu_peak_total_objects,
            objects_created_total: self.gpu_objects_created_total,
            objects_destroyed_total: self.gpu_objects_destroyed_total,
        }
    }

    pub(crate) fn record_lifetime_audit(&mut self, audit: LifetimeAuditSnapshot) {
        self.lifetime_audit = Some(audit);
    }

    pub(crate) fn snapshot(
        &self,
        identity: RuntimeIdentity,
        inventory: CompanionCapacityInventory,
        fixture: RuntimeFixtureIdentity,
    ) -> CompanionRuntimeMetricsSnapshot {
        CompanionRuntimeMetricsSnapshot {
            schema_version: 4,
            identity,
            fixture,
            sample_capacity: METRIC_SAMPLE_CAPACITY,
            visible_samples: self.ui_tick_us.len(),
            ui_tick_us: Percentiles::from_samples(&self.ui_tick_us),
            state_prepare_us: Percentiles::from_samples(&self.state_prepare_us),
            gpu_translate_us: Percentiles::from_samples(&self.gpu_translate_us),
            encode_us: Percentiles::from_samples(&self.encode_us),
            queue_wait_us: Percentiles::from_samples(&self.queue_wait_us),
            compile_us: Percentiles::from_samples(&self.compile_us),
            appkit_raster_queue_wait_us: Percentiles::from_samples(
                &self.appkit_raster_queue_wait_us,
            ),
            appkit_raster_slice_us: Percentiles::from_samples(&self.appkit_raster_slice_us),
            appkit_raster_total_us: Percentiles::from_samples(&self.appkit_raster_total_us),
            appkit_raster_slice_count: self.appkit_raster_slice_count,
            appkit_raster_deadline_misses: self.appkit_raster_deadline_misses,
            appkit_raster_coalesces: self.appkit_raster_coalesces,
            appkit_raster_cancellations: self.appkit_raster_cancellations,
            appkit_raster_diagnostic_capacity: APPKIT_RASTER_DIAGNOSTIC_CAPACITY,
            appkit_raster_slice_diagnostics: self
                .appkit_raster_slice_diagnostics
                .chronological_values(),
            activation_render_owner_us: Percentiles::from_samples(&self.activation_render_owner_us),
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
            capture_attempted: self.capture_attempted,
            capture_succeeded: self.capture_succeeded,
            capture_failed: self.capture_failed,
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
            metrics_overhead_control: measure_metrics_overhead_control(),
            inventory,
            hidden_segment: self.hidden_segment_snapshot(),
            gpu_accounting: self.gpu_accounting_snapshot(),
            lifetime_audit: self.lifetime_audit,
        }
    }
}

fn measure_metrics_overhead_control() -> MetricsOverheadAudit {
    const ITERATIONS: u64 = 100_000;
    const TRIALS: u32 = 5;
    let mut best_control = u64::MAX;
    let mut best_instrumented = u64::MAX;
    for _ in 0..TRIALS {
        METRICS_OVERHEAD_AUDIT_STATE.with(|cell| {
            *cell.borrow_mut() = CompanionRuntimeMetrics::default();
        });
        let mut control_ns = 0_u64;
        let mut instrumented_ns = 0_u64;
        let mut state = 0_u64;
        for tick in 0..ITERATIONS {
            let instrumented = tick % 2 == 0;
            let started = std::time::Instant::now();
            representative_metric_tick(instrumented, &mut state, tick);
            let elapsed = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
            if instrumented {
                instrumented_ns = instrumented_ns.saturating_add(elapsed);
            } else {
                control_ns = control_ns.saturating_add(elapsed);
            }
        }
        METRICS_OVERHEAD_AUDIT_STATE.with(|cell| {
            std::hint::black_box((&*cell.borrow(), state));
        });
        best_control = best_control.min(control_ns);
        best_instrumented = best_instrumented.min(instrumented_ns);
    }
    let control_ticks = ITERATIONS / 2;
    let instrumented_ticks = ITERATIONS - control_ticks;
    let control_ns_per_tick = best_control.div_ceil(control_ticks);
    let instrumented_ns_per_tick = best_instrumented.div_ceil(instrumented_ticks);
    MetricsOverheadAudit {
        iterations: ITERATIONS,
        trials: TRIALS,
        control_ticks,
        instrumented_ticks,
        control_ns_per_tick,
        instrumented_ns_per_tick,
        net_ns_per_tick: instrumented_ns_per_tick.saturating_sub(control_ns_per_tick),
    }
}

thread_local! {
    static METRICS_OVERHEAD_AUDIT_STATE: std::cell::RefCell<CompanionRuntimeMetrics> =
        std::cell::RefCell::new(CompanionRuntimeMetrics::default());
}

fn representative_metric_tick(instrumented: bool, state: &mut u64, tick: u64) {
    if instrumented {
        METRICS_OVERHEAD_AUDIT_STATE.with(|cell| cell.borrow_mut().begin_visible_tick());
    }
    let mut timed = |record: fn(&mut CompanionRuntimeMetrics, u32), salt: u64| {
        let started = std::time::Instant::now();
        *state = std::hint::black_box(
            state
                .saturating_add(tick ^ salt)
                .rotate_left((salt % 31) as u32),
        );
        let elapsed = duration_us(started.elapsed());
        if instrumented {
            METRICS_OVERHEAD_AUDIT_STATE.with(|cell| {
                let closure = |metrics: &mut CompanionRuntimeMetrics| record(metrics, elapsed);
                closure(&mut cell.borrow_mut());
            });
        }
    };
    timed(CompanionRuntimeMetrics::record_state_prepare_us, 3);
    timed(CompanionRuntimeMetrics::record_gpu_translate_us, 7);
    timed(CompanionRuntimeMetrics::record_encode_us, 11);
    timed(CompanionRuntimeMetrics::record_queue_wait_us, 13);
    if instrumented {
        METRICS_OVERHEAD_AUDIT_STATE.with(|cell| {
            let mut metrics = cell.borrow_mut();
            metrics.record_queue_write(256);
            metrics.record_surface_acquire();
            metrics.record_submit();
            metrics.record_draws(4);
            metrics.record_ui_tick_us(250);
        });
    }
    std::hint::black_box(*state);
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
        metrics.record_state_prepare_us(900);
        metrics.record_gpu_translate_us(300);
        metrics.record_encode_us(800);
        metrics.record_appkit_raster_queue_wait_us(17);
        metrics.record_appkit_raster_slice_us(3_999, false);
        metrics.record_appkit_raster_slice_us(4_001, true);
        metrics.record_appkit_raster_total_us(12_345);
        metrics.record_appkit_raster_coalesce();
        metrics.record_appkit_raster_cancellation();
        metrics.record_persistent_gpu_create(3);
        metrics.observe_nodes(72);
        let snapshot = metrics.snapshot(
            RuntimeIdentity::baseline(),
            CompanionCapacityInventory::contract_fixture(),
            RuntimeFixtureIdentity {
                fixture_id: "test",
                seed: "test",
                update_source: "fixed",
                cadence_ms: 250,
                logical_width: 360.0,
                logical_height: 360.0,
                physical_width: 720,
                physical_height: 720,
                backing_scale: 2.0,
            },
        );
        assert_eq!(snapshot.schema_version, 4);
        assert_eq!(snapshot.ui_tick_us.p95, Some(1_500));
        assert_eq!(snapshot.state_prepare_us.p95, Some(900));
        assert_eq!(snapshot.gpu_translate_us.p95, Some(300));
        assert_eq!(snapshot.encode_us.p99, Some(800));
        assert_eq!(snapshot.appkit_raster_queue_wait_us.p95, Some(17));
        assert_eq!(snapshot.appkit_raster_slice_us.p95, Some(4_001));
        assert_eq!(snapshot.appkit_raster_total_us.p95, Some(12_345));
        assert_eq!(snapshot.appkit_raster_slice_count, 2);
        assert_eq!(snapshot.appkit_raster_deadline_misses, 1);
        assert_eq!(snapshot.appkit_raster_coalesces, 1);
        assert_eq!(snapshot.appkit_raster_cancellations, 1);
        assert_eq!(snapshot.persistent_gpu_objects_created, 3);
        assert_eq!(snapshot.node_high_water, 72);
        assert_eq!(snapshot.identity.layout_generation, None);
        assert_eq!(snapshot.identity.semantic_revision, None);
        assert_eq!(snapshot.identity.frame_revision, None);
    }

    #[test]
    fn appkit_raster_diagnostics_are_bounded_and_privacy_safe() {
        let mut metrics = CompanionRuntimeMetrics::default();
        for index in 0..=APPKIT_RASTER_DIAGNOSTIC_CAPACITY {
            metrics.record_appkit_raster_slice_diagnostic(AppkitRasterSliceDiagnostic {
                start_cursor: index as u32,
                end_cursor: index as u32 + 1,
                items_completed: 1,
                elapsed_us: 1_200,
                setup_us: 100,
                max_item_us: 900,
                max_item_index: Some(index as u32),
            });
        }

        let snapshot = metrics.snapshot(
            RuntimeIdentity::baseline(),
            CompanionCapacityInventory::contract_fixture(),
            RuntimeFixtureIdentity {
                fixture_id: "test",
                seed: "test",
                update_source: "fixed",
                cadence_ms: 250,
                logical_width: 360.0,
                logical_height: 360.0,
                physical_width: 720,
                physical_height: 720,
                backing_scale: 2.0,
            },
        );

        assert_eq!(snapshot.schema_version, 4);
        assert_eq!(snapshot.appkit_raster_diagnostic_capacity, 256);
        assert_eq!(snapshot.appkit_raster_slice_diagnostics.len(), 256);
        assert_eq!(snapshot.appkit_raster_slice_diagnostics[0].start_cursor, 1);
        assert_eq!(
            snapshot.appkit_raster_slice_diagnostics[255].start_cursor,
            256
        );
        assert_eq!(
            snapshot.appkit_raster_slice_diagnostics[255].items_completed,
            1
        );
        assert_eq!(
            snapshot.appkit_raster_slice_diagnostics[255].elapsed_us,
            1_200
        );
        let json = serde_json::to_value(&snapshot.appkit_raster_slice_diagnostics).unwrap();
        let text = json.to_string();
        assert!(!text.contains("glyph"));
        assert!(!text.contains("sequence"));
        assert!(!text.contains("character"));
    }

    #[test]
    fn capacity_contract_distinguishes_observations_reservations_headroom_and_limits() {
        let inventory = CompanionCapacityInventory::contract_fixture();
        assert_eq!(inventory.max_pet_slots.observed, Some(96));
        assert_eq!(inventory.matrix_fixture_count, 630);
        assert_eq!(inventory.dimmed_fixture_count, 126);
        assert_eq!(inventory.full_props_tank_fixture_count, 630);
        assert_eq!(inventory.max_prepared_gpu_primitives.limit, 1_024);
        assert_eq!(inventory.max_pet_slots.reservation, 0);
        assert_eq!(inventory.max_pet_slots.headroom, 34);
        assert_eq!(inventory.max_pet_slots.limit, MAX_PET_ART_SLOTS as u32);
        assert_eq!(inventory.max_nodes.observed, None);
        assert_eq!(inventory.max_nodes.reservation, 96);
        assert_eq!(inventory.max_nodes.headroom, 32);
        assert_eq!(inventory.max_nodes.limit, MAX_SCENE_NODES as u32);
        assert!(inventory.fits_global_constraints());
    }

    #[test]
    fn hidden_steady_ticks_capture_zero_render_work_after_transition() {
        let mut metrics = CompanionRuntimeMetrics::default();
        let transition = metrics.work_counters();
        metrics.record_hidden_tick(transition);
        let steady = metrics.work_counters();
        metrics.record_hidden_tick(steady);
        metrics.record_hidden_tick(steady);
        let audit = metrics.hidden_segment_snapshot();
        assert_eq!(audit.transition_ticks, 1);
        assert_eq!(audit.steady_ticks, 2);
        assert_eq!(audit.steady_delta, RuntimeWorkCounters::default());
    }

    #[test]
    fn hidden_audit_detects_any_appkit_raster_slice() {
        let mut metrics = CompanionRuntimeMetrics::default();
        metrics.record_hidden_tick(metrics.work_counters());
        let steady = metrics.work_counters();
        metrics.record_appkit_raster_slice_us(1, false);
        metrics.record_hidden_tick(steady);
        assert_eq!(
            metrics
                .hidden_segment_snapshot()
                .steady_delta
                .appkit_raster_slices,
            1
        );
    }

    #[test]
    fn gpu_accounting_includes_concurrent_replacement_peak() {
        let mut metrics = CompanionRuntimeMetrics::default();
        metrics.replace_gpu_allocation(GpuAllocationKind::HostInfrastructure, 0, 4);
        metrics.replace_gpu_allocation(GpuAllocationKind::Atlas, 100, 2);
        metrics.replace_gpu_allocation(GpuAllocationKind::InstanceRing, 60, 3);
        metrics.replace_gpu_allocation(GpuAllocationKind::Capture, 80, 2);
        metrics.replace_gpu_allocation(GpuAllocationKind::Atlas, 120, 2);
        let accounting = metrics.gpu_accounting_snapshot();
        assert_eq!(accounting.current_bytes.atlas_bytes, 120);
        assert_eq!(accounting.current_bytes.instance_ring_bytes, 60);
        assert_eq!(accounting.current_bytes.capture_bytes, 80);
        assert_eq!(accounting.current_bytes.total_bytes, 260);
        assert_eq!(accounting.peak_total_bytes, 360);
        assert_eq!(accounting.current_objects.host_infrastructure, 4);
        assert_eq!(accounting.current_objects.total_objects, 11);
        assert_eq!(accounting.peak_total_objects, 13);
        assert_eq!(accounting.objects_created_total, 13);
        assert_eq!(accounting.objects_destroyed_total, 2);
        assert_eq!(metrics.persistent_gpu_objects_created, 13);
        assert_eq!(metrics.persistent_gpu_objects_destroyed, 2);
    }

    #[test]
    fn capture_metrics_distinguish_attempt_success_and_failure() {
        let mut metrics = CompanionRuntimeMetrics::default();
        metrics.record_capture_attempt();
        metrics.record_capture_success();
        metrics.record_capture_attempt();
        metrics.record_capture_failure();
        assert_eq!(metrics.capture_attempted, 2);
        assert_eq!(metrics.capture_succeeded, 1);
        assert_eq!(metrics.capture_failed, 1);
    }

    #[test]
    fn metrics_overhead_audit_pairs_full_tick_instrumentation_with_control() {
        let audit = measure_metrics_overhead_control();
        assert_eq!(audit.iterations, 100_000);
        assert_eq!(audit.trials, 5);
        assert_eq!(audit.control_ticks, audit.instrumented_ticks);
        assert_eq!(audit.control_ticks, 50_000);
        assert!(audit.control_ns_per_tick > 0);
        assert!(audit.instrumented_ns_per_tick > 0);
        assert!(audit.instrumented_ns_per_tick >= audit.net_ns_per_tick);
    }
}
