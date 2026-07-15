use serde::Serialize;
use std::time::Duration;

#[cfg(test)]
use crate::presentation::companion_scene::scene::{
    MAX_AMBIENT_INSTANCES, MAX_ATTACHMENTS, MAX_BLENDED_DRAWS, MAX_LIGHTS, MAX_PET_ART_SLOTS,
    MAX_ROUND_TANK_INHABITANTS, MAX_SCENE_NODES, MAX_STATIC_PRIMITIVES, MAX_VISIBLE_PROPS,
};

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
    pub max: Option<u32>,
}

impl Percentiles {
    fn from_samples<const N: usize>(samples: &FixedSamples<N>) -> Self {
        Self {
            p50: samples.percentile(50),
            p95: samples.percentile(95),
            p99: samples.percentile(99),
            max: samples.percentile(100),
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
    pub worker_submissions: u64,
    pub gpu_materializations: u64,
    pub queue_writes: u64,
    pub surface_acquires: u64,
    pub encode: u64,
    pub submit: u64,
}

impl RuntimeWorkCounters {
    pub(crate) fn saturating_sub(self, previous: Self) -> Self {
        Self {
            prepare: self.prepare.saturating_sub(previous.prepare),
            worker_submissions: self
                .worker_submissions
                .saturating_sub(previous.worker_submissions),
            gpu_materializations: self
                .gpu_materializations
                .saturating_sub(previous.gpu_materializations),
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
            worker_submissions: self
                .worker_submissions
                .saturating_add(other.worker_submissions),
            gpu_materializations: self
                .gpu_materializations
                .saturating_add(other.gpu_materializations),
            queue_writes: self.queue_writes.saturating_add(other.queue_writes),
            surface_acquires: self.surface_acquires.saturating_add(other.surface_acquires),
            encode: self.encode.saturating_add(other.encode),
            submit: self.submit.saturating_add(other.submit),
        }
    }

    pub(crate) fn normalized_per_second(self, ticks: u64, cadence_hz: u32) -> Self {
        fn normalize(value: u64, ticks: u64, cadence_hz: u32) -> u64 {
            if ticks == 0 {
                return 0;
            }
            u64::try_from(
                u128::from(value)
                    .saturating_mul(u128::from(cadence_hz))
                    .saturating_add(u128::from(ticks / 2))
                    / u128::from(ticks),
            )
            .unwrap_or(u64::MAX)
        }

        Self {
            prepare: normalize(self.prepare, ticks, cadence_hz),
            worker_submissions: normalize(self.worker_submissions, ticks, cadence_hz),
            gpu_materializations: normalize(self.gpu_materializations, ticks, cadence_hz),
            queue_writes: normalize(self.queue_writes, ticks, cadence_hz),
            surface_acquires: normalize(self.surface_acquires, ticks, cadence_hz),
            encode: normalize(self.encode, ticks, cadence_hz),
            submit: normalize(self.submit, ticks, cadence_hz),
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
    pub semantic_samples: u64,
    pub warmup_semantic_samples: u64,
    pub presentation_ticks: u64,
    pub warmup_presentation_ticks: u64,
    pub semantic_cadence_ms: u64,
    pub presentation_cadence_hz: u32,
    pub virtual_elapsed_ms: u64,
    pub snapshot_projections: u64,
    pub semantic_reconciles: u64,
    pub frame_projections: u64,
    pub frame_reconciles: u64,
    pub encoded_ticks: u64,
    pub submitted_ticks: u64,
    pub draw_calls: u64,
    pub poll_count: u64,
    pub work_delta: RuntimeWorkCounters,
    pub work_per_second: RuntimeWorkCounters,
    pub capacity_growth_events: u64,
    pub stale_mutations: u64,
    pub stale_rejections: u64,
    pub stale_regenerations: u64,
    pub post_warmup_resource_creations: u64,
    pub post_warmup_static_upload_bytes: u64,
    pub direct_target_prewarmed: bool,
    pub direct_target_reused: bool,
    pub direct_readback_prewarmed: bool,
    pub direct_readback_reused: bool,
    pub terminal_direct_capture_attempted: u64,
    pub terminal_direct_capture_succeeded: u64,
    pub terminal_direct_capture_nonblank: u64,
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
    pub semantic_cadence_ms: u64,
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
    pub presentation_cadence_target_hz: u32,
    pub semantic_cadence_target_hz: u32,
    pub sample_capacity: usize,
    pub visible_samples: usize,
    pub ui_tick_us: Percentiles,
    pub projection_us: Percentiles,
    pub reconcile_us: Percentiles,
    pub state_prepare_us: Percentiles,
    pub gpu_translate_us: Percentiles,
    pub delta_write_us: Percentiles,
    pub encode_us: Percentiles,
    pub submit_us: Percentiles,
    pub capture_us: Percentiles,
    pub queue_wait_us: Percentiles,
    pub worker_active_compile_us: Percentiles,
    pub raster_request_wall_us: Percentiles,
    pub generation_service_ui_us: Percentiles,
    pub gpu_materialize_publish_us: Percentiles,
    pub activation_render_owner_us: Percentiles,
    pub main_thread_raster_calls: u64,
    pub worker_raster_calls: u64,
    pub worker_submissions: u64,
    pub worker_completions: u64,
    pub worker_cancellations: u64,
    pub worker_coalesces: u64,
    pub worker_stale_rejections: u64,
    pub worker_failures: u64,
    pub gpu_materializations: u64,
    pub generation_count: u64,
    pub generation_requests: u64,
    pub generation_coalesces: u64,
    pub generation_completions: u64,
    pub generation_failures: u64,
    pub generation_retries: u64,
    pub generation_stale_drops: u64,
    pub generation_activations: u64,
    pub snapshot_projections: u64,
    pub semantic_reconciles: u64,
    pub frame_reconciles: u64,
    pub unchanged_ticks: u64,
    pub content_dirty_ranges: u64,
    pub content_dirty_bytes: u64,
    pub frame_dirty_ranges: u64,
    pub frame_dirty_bytes: u64,
    pub static_upload_bytes: u64,
    pub dynamic_upload_bytes: u64,
    pub queue_writes: u64,
    pub draw_calls: u64,
    pub persistent_gpu_objects_created: u64,
    pub persistent_gpu_objects_destroyed: u64,
    pub hidden_ticks: u64,
    pub prepare_count: u64,
    pub present_attempts: u64,
    pub surface_acquires: u64,
    pub encode_count: u64,
    pub submit_count: u64,
    pub successful_presents: u64,
    pub skipped_frames: u64,
    pub skipped_resource_preparation: u64,
    pub skipped_outdated: u64,
    pub skipped_timeout: u64,
    pub skipped_occluded: u64,
    pub longest_visible_no_present_ms: u64,
    pub resize_invalidations: u64,
    pub scale_invalidations: u64,
    pub atlas_backing_scale: Option<f64>,
    pub fallback_count: u64,
    pub fallback_pending_transitions: u64,
    pub fallback_painted_transitions: u64,
    pub capture_attempted: u64,
    pub capture_succeeded: u64,
    pub capture_failed: u64,
    pub capture_nonblank_validated: u64,
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
    projection_us: Box<FixedSamples<METRIC_SAMPLE_CAPACITY>>,
    reconcile_us: Box<FixedSamples<METRIC_SAMPLE_CAPACITY>>,
    state_prepare_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    gpu_translate_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    delta_write_us: Box<FixedSamples<METRIC_SAMPLE_CAPACITY>>,
    encode_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    submit_us: Box<FixedSamples<METRIC_SAMPLE_CAPACITY>>,
    capture_us: Box<FixedSamples<METRIC_SAMPLE_CAPACITY>>,
    queue_wait_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    worker_active_compile_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    raster_request_wall_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    generation_service_ui_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    gpu_materialize_publish_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    activation_render_owner_us: FixedSamples<METRIC_SAMPLE_CAPACITY>,
    main_thread_raster_calls: u64,
    worker_raster_calls: u64,
    worker_submissions: u64,
    worker_completions: u64,
    worker_cancellations: u64,
    worker_coalesces: u64,
    worker_stale_rejections: u64,
    worker_failures: u64,
    gpu_materializations: u64,
    generation_count: u64,
    generation_retries: u64,
    generation_activations: u64,
    snapshot_projections: u64,
    semantic_reconciles: u64,
    frame_reconciles: u64,
    unchanged_ticks: u64,
    content_dirty_ranges: u64,
    content_dirty_bytes: u64,
    frame_dirty_ranges: u64,
    frame_dirty_bytes: u64,
    static_upload_bytes: u64,
    dynamic_upload_bytes: u64,
    queue_writes: u64,
    draw_calls: u64,
    persistent_gpu_objects_created: u64,
    persistent_gpu_objects_destroyed: u64,
    hidden_ticks: u64,
    prepare_count: u64,
    present_attempts: u64,
    surface_acquires: u64,
    encode_count: u64,
    submit_count: u64,
    successful_presents: u64,
    skipped_frames: u64,
    skipped_resource_preparation: u64,
    skipped_outdated: u64,
    skipped_timeout: u64,
    skipped_occluded: u64,
    longest_visible_no_present_ms: u64,
    resize_invalidations: u64,
    scale_invalidations: u64,
    atlas_backing_scale: Option<f64>,
    fallback_count: u64,
    fallback_pending_transitions: u64,
    fallback_painted_transitions: u64,
    capture_attempted: u64,
    capture_succeeded: u64,
    capture_failed: u64,
    capture_nonblank_validated: u64,
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
            projection_us: Box::default(),
            reconcile_us: Box::default(),
            state_prepare_us: FixedSamples::default(),
            gpu_translate_us: FixedSamples::default(),
            delta_write_us: Box::default(),
            encode_us: FixedSamples::default(),
            submit_us: Box::default(),
            capture_us: Box::default(),
            queue_wait_us: FixedSamples::default(),
            worker_active_compile_us: FixedSamples::default(),
            raster_request_wall_us: FixedSamples::default(),
            generation_service_ui_us: FixedSamples::default(),
            gpu_materialize_publish_us: FixedSamples::default(),
            activation_render_owner_us: FixedSamples::default(),
            main_thread_raster_calls: 0,
            worker_raster_calls: 0,
            worker_submissions: 0,
            worker_completions: 0,
            worker_cancellations: 0,
            worker_coalesces: 0,
            worker_stale_rejections: 0,
            worker_failures: 0,
            gpu_materializations: 0,
            generation_count: 0,
            generation_retries: 0,
            generation_activations: 0,
            snapshot_projections: 0,
            semantic_reconciles: 0,
            frame_reconciles: 0,
            unchanged_ticks: 0,
            content_dirty_ranges: 0,
            content_dirty_bytes: 0,
            frame_dirty_ranges: 0,
            frame_dirty_bytes: 0,
            static_upload_bytes: 0,
            dynamic_upload_bytes: 0,
            queue_writes: 0,
            draw_calls: 0,
            persistent_gpu_objects_created: 0,
            persistent_gpu_objects_destroyed: 0,
            hidden_ticks: 0,
            prepare_count: 0,
            present_attempts: 0,
            surface_acquires: 0,
            encode_count: 0,
            submit_count: 0,
            successful_presents: 0,
            skipped_frames: 0,
            skipped_resource_preparation: 0,
            skipped_outdated: 0,
            skipped_timeout: 0,
            skipped_occluded: 0,
            longest_visible_no_present_ms: 0,
            resize_invalidations: 0,
            scale_invalidations: 0,
            atlas_backing_scale: None,
            fallback_count: 0,
            fallback_pending_transitions: 0,
            fallback_painted_transitions: 0,
            capture_attempted: 0,
            capture_succeeded: 0,
            capture_failed: 0,
            capture_nonblank_validated: 0,
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
                self.present_attempts = 0;
                self.snapshot_projections = 0;
                self.semantic_reconciles = 0;
                self.frame_reconciles = 0;
                self.unchanged_ticks = 0;
                self.content_dirty_ranges = 0;
                self.content_dirty_bytes = 0;
                self.frame_dirty_ranges = 0;
                self.frame_dirty_bytes = 0;
                self.surface_acquires = 0;
                self.encode_count = 0;
                self.submit_count = 0;
                self.successful_presents = 0;
                self.skipped_frames = 0;
                self.skipped_resource_preparation = 0;
                self.skipped_outdated = 0;
                self.skipped_timeout = 0;
                self.skipped_occluded = 0;
                self.longest_visible_no_present_ms = 0;
                self.resize_invalidations = 0;
                self.scale_invalidations = 0;
                self.fallback_count = 0;
                self.fallback_pending_transitions = 0;
                self.fallback_painted_transitions = 0;
                self.capture_attempted = 0;
                self.capture_succeeded = 0;
                self.capture_failed = 0;
                self.capture_nonblank_validated = 0;
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

    pub(crate) fn record_projection_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.projection_us.push(value);
        }
    }

    pub(crate) fn record_reconcile_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.reconcile_us.push(value);
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

    pub(crate) fn record_delta_write_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.delta_write_us.push(value);
        }
    }

    pub(crate) fn record_encode_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.encode_us.push(value);
            increment(&mut self.encode_count, 1);
        }
    }

    pub(crate) fn record_submit_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.submit_us.push(value);
        }
    }

    pub(crate) fn record_capture_us(&mut self, value: u32) {
        self.capture_us.push(value);
    }

    pub(crate) fn record_queue_wait_us(&mut self, value: u32) {
        if self.sample_current_tick {
            self.queue_wait_us.push(value);
        }
    }

    pub(crate) fn record_worker_terminal(
        &mut self,
        active_compile: Duration,
        raster_calls: u32,
        main_thread_raster_calls: u32,
    ) {
        self.worker_active_compile_us
            .push(duration_us(active_compile));
        increment(&mut self.worker_raster_calls, u64::from(raster_calls));
        increment(
            &mut self.main_thread_raster_calls,
            u64::from(main_thread_raster_calls),
        );
    }

    pub(crate) fn record_raster_request_wall_us(&mut self, value: u32) {
        self.raster_request_wall_us.push(value);
    }

    pub(crate) fn record_generation_service_ui_us(&mut self, value: u32) {
        self.generation_service_ui_us.push(value);
    }

    pub(crate) fn record_gpu_materialize_publish_us(&mut self, value: u32) {
        self.gpu_materialize_publish_us.push(value);
        increment(&mut self.gpu_materializations, 1);
    }

    pub(crate) fn record_worker_submission(&mut self) {
        increment(&mut self.worker_submissions, 1);
    }

    pub(crate) fn record_worker_completion(&mut self) {
        increment(&mut self.worker_completions, 1);
    }

    pub(crate) fn record_worker_cancellation(&mut self) {
        increment(&mut self.worker_cancellations, 1);
    }

    pub(crate) fn record_worker_coalesce(&mut self) {
        increment(&mut self.worker_coalesces, 1);
    }

    pub(crate) fn record_worker_stale_rejection(&mut self) {
        increment(&mut self.worker_stale_rejections, 1);
    }

    pub(crate) fn record_worker_failure(&mut self) {
        increment(&mut self.worker_failures, 1);
    }

    pub(crate) fn record_generation_accepted(&mut self) {
        increment(&mut self.generation_count, 1);
    }

    pub(crate) fn record_generation_retry(&mut self) {
        increment(&mut self.generation_retries, 1);
    }

    pub(crate) fn record_generation_activation(&mut self, atlas_backing_scale: f64) {
        increment(&mut self.generation_activations, 1);
        self.atlas_backing_scale = Some(atlas_backing_scale);
    }

    pub(crate) fn record_resize_invalidation(&mut self) {
        increment(&mut self.resize_invalidations, 1);
    }

    pub(crate) fn record_scale_invalidation(&mut self) {
        increment(&mut self.scale_invalidations, 1);
    }

    pub(crate) fn record_snapshot_projection(&mut self) {
        increment(&mut self.snapshot_projections, 1);
    }

    pub(crate) fn record_semantic_reconcile(&mut self) {
        increment(&mut self.semantic_reconciles, 1);
    }

    pub(crate) fn record_frame_reconcile(&mut self) {
        increment(&mut self.frame_reconciles, 1);
    }

    pub(crate) fn record_unchanged_tick(&mut self) {
        increment(&mut self.unchanged_ticks, 1);
    }

    pub(crate) fn record_scene_dirty_upload(
        &mut self,
        content_ranges: u64,
        content_bytes: u64,
        frame_ranges: u64,
        frame_bytes: u64,
    ) {
        increment(&mut self.content_dirty_ranges, content_ranges);
        increment(&mut self.content_dirty_bytes, content_bytes);
        increment(&mut self.frame_dirty_ranges, frame_ranges);
        increment(&mut self.frame_dirty_bytes, frame_bytes);
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

    pub(crate) fn record_present_attempt(&mut self) {
        if self.sample_current_tick {
            increment(&mut self.present_attempts, 1);
        }
    }

    pub(crate) fn record_submit(&mut self) {
        if self.sample_current_tick {
            increment(&mut self.submit_count, 1);
        }
    }

    pub(crate) fn record_skip(&mut self, reason: super::SkipReason) {
        if self.sample_current_tick {
            increment(&mut self.skipped_frames, 1);
            match reason {
                super::SkipReason::ResourcePreparation => {
                    increment(&mut self.skipped_resource_preparation, 1);
                }
                super::SkipReason::Outdated => increment(&mut self.skipped_outdated, 1),
                super::SkipReason::Timeout => increment(&mut self.skipped_timeout, 1),
                super::SkipReason::Occluded => increment(&mut self.skipped_occluded, 1),
            }
        }
    }

    pub(crate) fn record_present(&mut self, visible_no_present: Duration) {
        if self.sample_current_tick {
            increment(&mut self.successful_presents, 1);
            self.observe_visible_no_present(visible_no_present);
        }
    }

    pub(crate) fn observe_visible_no_present(&mut self, interval: Duration) {
        if self.sample_current_tick {
            self.longest_visible_no_present_ms = self
                .longest_visible_no_present_ms
                .max(interval.as_millis().min(u128::from(u64::MAX)) as u64);
        }
    }

    pub(crate) fn record_capture_attempt(&mut self) {
        increment(&mut self.capture_attempted, 1);
    }

    pub(crate) fn record_capture_success(&mut self) {
        increment(&mut self.capture_succeeded, 1);
    }

    pub(crate) fn record_capture_nonblank_validated(&mut self) {
        increment(&mut self.capture_nonblank_validated, 1);
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
            worker_submissions: self.worker_submissions,
            gpu_materializations: self.gpu_materializations,
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

    pub(crate) const fn static_upload_bytes(&self) -> u64 {
        self.static_upload_bytes
    }

    pub(crate) const fn persistent_gpu_objects_created(&self) -> u64 {
        self.persistent_gpu_objects_created
    }

    pub(crate) fn record_lifetime_audit(&mut self, audit: LifetimeAuditSnapshot) {
        self.lifetime_audit = Some(audit);
    }

    pub(crate) fn record_lifetime_terminal_capture(&mut self, succeeded: bool) {
        let Some(audit) = self.lifetime_audit.as_mut() else {
            return;
        };
        audit.terminal_direct_capture_attempted =
            audit.terminal_direct_capture_attempted.saturating_add(1);
        if succeeded {
            audit.terminal_direct_capture_succeeded =
                audit.terminal_direct_capture_succeeded.saturating_add(1);
            audit.terminal_direct_capture_nonblank =
                audit.terminal_direct_capture_nonblank.saturating_add(1);
        }
    }

    pub(crate) fn snapshot(
        &self,
        identity: RuntimeIdentity,
        inventory: CompanionCapacityInventory,
        fixture: RuntimeFixtureIdentity,
    ) -> CompanionRuntimeMetricsSnapshot {
        CompanionRuntimeMetricsSnapshot {
            schema_version: 10,
            identity,
            fixture,
            presentation_cadence_target_hz: 30,
            semantic_cadence_target_hz: 4,
            sample_capacity: METRIC_SAMPLE_CAPACITY,
            visible_samples: self.ui_tick_us.len(),
            ui_tick_us: Percentiles::from_samples(&self.ui_tick_us),
            projection_us: Percentiles::from_samples(&self.projection_us),
            reconcile_us: Percentiles::from_samples(&self.reconcile_us),
            state_prepare_us: Percentiles::from_samples(&self.state_prepare_us),
            gpu_translate_us: Percentiles::from_samples(&self.gpu_translate_us),
            delta_write_us: Percentiles::from_samples(&self.delta_write_us),
            encode_us: Percentiles::from_samples(&self.encode_us),
            submit_us: Percentiles::from_samples(&self.submit_us),
            capture_us: Percentiles::from_samples(&self.capture_us),
            queue_wait_us: Percentiles::from_samples(&self.queue_wait_us),
            worker_active_compile_us: Percentiles::from_samples(&self.worker_active_compile_us),
            raster_request_wall_us: Percentiles::from_samples(&self.raster_request_wall_us),
            generation_service_ui_us: Percentiles::from_samples(&self.generation_service_ui_us),
            gpu_materialize_publish_us: Percentiles::from_samples(&self.gpu_materialize_publish_us),
            activation_render_owner_us: Percentiles::from_samples(&self.activation_render_owner_us),
            main_thread_raster_calls: self.main_thread_raster_calls,
            worker_raster_calls: self.worker_raster_calls,
            worker_submissions: self.worker_submissions,
            worker_completions: self.worker_completions,
            worker_cancellations: self.worker_cancellations,
            worker_coalesces: self.worker_coalesces,
            worker_stale_rejections: self.worker_stale_rejections,
            worker_failures: self.worker_failures,
            gpu_materializations: self.gpu_materializations,
            generation_count: self.generation_count,
            generation_requests: self.worker_submissions,
            generation_coalesces: self.worker_coalesces,
            generation_completions: self.worker_completions,
            generation_failures: self.worker_failures,
            generation_retries: self.generation_retries,
            generation_stale_drops: self.worker_stale_rejections,
            generation_activations: self.generation_activations,
            snapshot_projections: self.snapshot_projections,
            semantic_reconciles: self.semantic_reconciles,
            frame_reconciles: self.frame_reconciles,
            unchanged_ticks: self.unchanged_ticks,
            content_dirty_ranges: self.content_dirty_ranges,
            content_dirty_bytes: self.content_dirty_bytes,
            frame_dirty_ranges: self.frame_dirty_ranges,
            frame_dirty_bytes: self.frame_dirty_bytes,
            static_upload_bytes: self.static_upload_bytes,
            dynamic_upload_bytes: self.dynamic_upload_bytes,
            queue_writes: self.queue_writes,
            draw_calls: self.draw_calls,
            persistent_gpu_objects_created: self.persistent_gpu_objects_created,
            persistent_gpu_objects_destroyed: self.persistent_gpu_objects_destroyed,
            hidden_ticks: self.hidden_ticks,
            prepare_count: self.prepare_count,
            present_attempts: self.present_attempts,
            surface_acquires: self.surface_acquires,
            encode_count: self.encode_count,
            submit_count: self.submit_count,
            successful_presents: self.successful_presents,
            skipped_frames: self.skipped_frames,
            skipped_resource_preparation: self.skipped_resource_preparation,
            skipped_outdated: self.skipped_outdated,
            skipped_timeout: self.skipped_timeout,
            skipped_occluded: self.skipped_occluded,
            longest_visible_no_present_ms: self.longest_visible_no_present_ms,
            resize_invalidations: self.resize_invalidations,
            scale_invalidations: self.scale_invalidations,
            atlas_backing_scale: self.atlas_backing_scale,
            fallback_count: self.fallback_count,
            fallback_pending_transitions: self.fallback_pending_transitions,
            fallback_painted_transitions: self.fallback_painted_transitions,
            capture_attempted: self.capture_attempted,
            capture_succeeded: self.capture_succeeded,
            capture_failed: self.capture_failed,
            capture_nonblank_validated: self.capture_nonblank_validated,
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
        assert_eq!(Percentiles::from_samples(&samples).max, Some(50));
    }

    #[test]
    fn snapshot_carries_epochs_counters_and_high_water_marks() {
        let mut metrics = CompanionRuntimeMetrics::default();
        metrics.record_ui_tick_us(1_500);
        metrics.record_projection_us(250);
        metrics.record_reconcile_us(275);
        metrics.record_state_prepare_us(900);
        metrics.record_gpu_translate_us(300);
        metrics.record_delta_write_us(325);
        metrics.record_encode_us(800);
        metrics.record_submit_us(125);
        metrics.record_capture_us(5_000);
        metrics.record_worker_terminal(Duration::from_micros(12_345), 242, 0);
        metrics.record_raster_request_wall_us(250_000);
        metrics.record_generation_service_ui_us(3_999);
        metrics.record_generation_service_ui_us(4_000);
        metrics.record_gpu_materialize_publish_us(15_999);
        metrics.record_worker_submission();
        metrics.record_worker_completion();
        metrics.record_worker_cancellation();
        metrics.record_worker_coalesce();
        metrics.record_worker_stale_rejection();
        metrics.record_worker_failure();
        metrics.record_generation_accepted();
        metrics.record_generation_retry();
        metrics.record_generation_activation(2.0);
        metrics.record_snapshot_projection();
        metrics.record_semantic_reconcile();
        metrics.record_frame_reconcile();
        metrics.record_unchanged_tick();
        metrics.record_scene_dirty_upload(2, 320, 3, 480);
        metrics.record_present_attempt();
        metrics.record_surface_acquire();
        metrics.record_submit();
        metrics.record_draws(12);
        metrics.record_skip(super::super::SkipReason::ResourcePreparation);
        metrics.record_skip(super::super::SkipReason::Outdated);
        metrics.record_skip(super::super::SkipReason::Timeout);
        metrics.record_skip(super::super::SkipReason::Occluded);
        metrics.observe_visible_no_present(Duration::from_millis(95));
        metrics.record_present(Duration::from_millis(120));
        metrics.record_resize_invalidation();
        metrics.record_scale_invalidation();
        metrics.record_persistent_gpu_create(3);
        metrics.observe_nodes(72);
        let snapshot = metrics.snapshot(
            RuntimeIdentity::baseline(),
            CompanionCapacityInventory::contract_fixture(),
            RuntimeFixtureIdentity {
                fixture_id: "test",
                seed: "test",
                update_source: "fixed",
                semantic_cadence_ms: 250,
                logical_width: 360.0,
                logical_height: 360.0,
                physical_width: 720,
                physical_height: 720,
                backing_scale: 2.0,
            },
        );
        assert_eq!(snapshot.schema_version, 10);
        assert_eq!(snapshot.presentation_cadence_target_hz, 30);
        assert_eq!(snapshot.semantic_cadence_target_hz, 4);
        assert_eq!(snapshot.fixture.semantic_cadence_ms, 250);
        assert_eq!(snapshot.ui_tick_us.p95, Some(1_500));
        assert_eq!(snapshot.projection_us.p95, Some(250));
        assert_eq!(snapshot.reconcile_us.p95, Some(275));
        assert_eq!(snapshot.state_prepare_us.p95, Some(900));
        assert_eq!(snapshot.gpu_translate_us.p95, Some(300));
        assert_eq!(snapshot.delta_write_us.p95, Some(325));
        assert_eq!(snapshot.encode_us.p99, Some(800));
        assert_eq!(snapshot.submit_us.p99, Some(125));
        assert_eq!(snapshot.capture_us.p99, Some(5_000));
        assert_eq!(snapshot.worker_active_compile_us.max, Some(12_345));
        assert_eq!(snapshot.raster_request_wall_us.max, Some(250_000));
        assert_eq!(snapshot.generation_service_ui_us.max, Some(4_000));
        assert_eq!(snapshot.gpu_materialize_publish_us.max, Some(15_999));
        assert_eq!(snapshot.main_thread_raster_calls, 0);
        assert_eq!(snapshot.worker_raster_calls, 242);
        assert_eq!(snapshot.worker_submissions, 1);
        assert_eq!(snapshot.worker_completions, 1);
        assert_eq!(snapshot.worker_cancellations, 1);
        assert_eq!(snapshot.worker_coalesces, 1);
        assert_eq!(snapshot.worker_stale_rejections, 1);
        assert_eq!(snapshot.worker_failures, 1);
        assert_eq!(snapshot.gpu_materializations, 1);
        assert_eq!(snapshot.generation_count, 1);
        assert_eq!(snapshot.generation_requests, 1);
        assert_eq!(snapshot.generation_coalesces, 1);
        assert_eq!(snapshot.generation_completions, 1);
        assert_eq!(snapshot.generation_failures, 1);
        assert_eq!(snapshot.generation_retries, 1);
        assert_eq!(snapshot.generation_stale_drops, 1);
        assert_eq!(snapshot.generation_activations, 1);
        assert_eq!(snapshot.atlas_backing_scale, Some(2.0));
        assert_eq!(snapshot.snapshot_projections, 1);
        assert_eq!(snapshot.semantic_reconciles, 1);
        assert_eq!(snapshot.frame_reconciles, 1);
        assert_eq!(snapshot.unchanged_ticks, 1);
        assert_eq!(snapshot.content_dirty_ranges, 2);
        assert_eq!(snapshot.content_dirty_bytes, 320);
        assert_eq!(snapshot.frame_dirty_ranges, 3);
        assert_eq!(snapshot.frame_dirty_bytes, 480);
        assert_eq!(snapshot.present_attempts, 1);
        assert_eq!(snapshot.surface_acquires, 1);
        assert_eq!(snapshot.submit_count, 1);
        assert_eq!(snapshot.draw_calls, 12);
        assert_eq!(snapshot.successful_presents, 1);
        assert_eq!(snapshot.skipped_frames, 4);
        assert_eq!(snapshot.skipped_resource_preparation, 1);
        assert_eq!(snapshot.skipped_outdated, 1);
        assert_eq!(snapshot.skipped_timeout, 1);
        assert_eq!(snapshot.skipped_occluded, 1);
        assert_eq!(snapshot.longest_visible_no_present_ms, 120);
        assert_eq!(snapshot.resize_invalidations, 1);
        assert_eq!(snapshot.scale_invalidations, 1);
        assert_eq!(snapshot.fallback_count, 0);
        assert_eq!(snapshot.fallback_pending_transitions, 0);
        assert_eq!(snapshot.fallback_painted_transitions, 0);
        assert_eq!(snapshot.persistent_gpu_objects_created, 3);
        assert_eq!(snapshot.node_high_water, 72);
        assert_eq!(snapshot.identity.layout_generation, None);
        assert_eq!(snapshot.identity.semantic_revision, None);
        assert_eq!(snapshot.identity.frame_revision, None);
    }

    #[test]
    fn lifetime_audit_serializes_explicit_dual_cadence_and_direct_evidence() {
        let audit = LifetimeAuditSnapshot {
            semantic_samples: 4_500,
            warmup_semantic_samples: 4_500,
            presentation_ticks: 33_750,
            warmup_presentation_ticks: 33_750,
            semantic_cadence_ms: 250,
            presentation_cadence_hz: 30,
            virtual_elapsed_ms: 1_125_000,
            encoded_ticks: 33_750,
            submitted_ticks: 33_750,
            direct_target_prewarmed: true,
            direct_target_reused: true,
            direct_readback_prewarmed: true,
            direct_readback_reused: true,
            terminal_direct_capture_attempted: 1,
            terminal_direct_capture_succeeded: 1,
            terminal_direct_capture_nonblank: 1,
            ..LifetimeAuditSnapshot::default()
        };

        let value = serde_json::to_value(audit).unwrap();
        assert_eq!(value["semantic_samples"], 4_500);
        assert_eq!(value["presentation_ticks"], 33_750);
        assert_eq!(value["semantic_cadence_ms"], 250);
        assert_eq!(value["presentation_cadence_hz"], 30);
        assert_eq!(value["virtual_elapsed_ms"], 1_125_000);
        assert_eq!(value["terminal_direct_capture_succeeded"], 1);
        assert_eq!(value["terminal_direct_capture_nonblank"], 1);
        assert!(value.get("frames").is_none());
        assert!(value.get("prepared_frames").is_none());
    }

    #[test]
    fn runtime_work_normalization_reports_per_second_without_float_drift() {
        let work = RuntimeWorkCounters {
            prepare: 33_750,
            worker_submissions: 0,
            gpu_materializations: 0,
            queue_writes: 67_500,
            surface_acquires: 0,
            encode: 33_750,
            submit: 33_750,
        };

        assert_eq!(
            work.normalized_per_second(33_750, 30),
            RuntimeWorkCounters {
                prepare: 30,
                worker_submissions: 0,
                gpu_materializations: 0,
                queue_writes: 60,
                surface_acquires: 0,
                encode: 30,
                submit: 30,
            }
        );
        assert_eq!(
            work.normalized_per_second(0, 30),
            RuntimeWorkCounters::default()
        );
    }

    #[test]
    fn worker_and_materialization_evidence_survives_visible_warmup_filter() {
        let mut metrics = CompanionRuntimeMetrics::default();
        metrics.discard_initial_visible_ticks(20);
        metrics.begin_visible_tick();
        metrics.record_worker_terminal(Duration::from_micros(9_000), 242, 0);
        metrics.record_raster_request_wall_us(250_000);
        metrics.record_generation_service_ui_us(1_500);
        metrics.record_gpu_materialize_publish_us(8_000);
        metrics.record_worker_submission();
        metrics.record_worker_completion();
        metrics.record_generation_accepted();

        let snapshot = metrics.snapshot(
            RuntimeIdentity::baseline(),
            CompanionCapacityInventory::contract_fixture(),
            RuntimeFixtureIdentity {
                fixture_id: "test",
                seed: "test",
                update_source: "fixed",
                semantic_cadence_ms: 250,
                logical_width: 360.0,
                logical_height: 360.0,
                physical_width: 720,
                physical_height: 720,
                backing_scale: 2.0,
            },
        );

        assert_eq!(snapshot.visible_samples, 0);
        assert_eq!(snapshot.worker_active_compile_us.max, Some(9_000));
        assert_eq!(snapshot.raster_request_wall_us.max, Some(250_000));
        assert_eq!(snapshot.generation_service_ui_us.max, Some(1_500));
        assert_eq!(snapshot.gpu_materialize_publish_us.max, Some(8_000));
        assert_eq!(snapshot.worker_submissions, 1);
        assert_eq!(snapshot.worker_completions, 1);
        assert_eq!(snapshot.gpu_materializations, 1);
        assert_eq!(snapshot.generation_count, 1);
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
    fn hidden_audit_detects_worker_submission_and_gpu_materialization() {
        let mut metrics = CompanionRuntimeMetrics::default();
        metrics.record_hidden_tick(metrics.work_counters());
        let steady = metrics.work_counters();
        metrics.record_worker_submission();
        metrics.record_gpu_materialize_publish_us(1);
        metrics.record_hidden_tick(steady);
        let delta = metrics.hidden_segment_snapshot().steady_delta;
        assert_eq!(delta.worker_submissions, 1);
        assert_eq!(delta.gpu_materializations, 1);
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
        metrics.record_capture_nonblank_validated();
        metrics.record_capture_attempt();
        metrics.record_capture_failure();
        assert_eq!(metrics.capture_attempted, 2);
        assert_eq!(metrics.capture_succeeded, 1);
        assert_eq!(metrics.capture_failed, 1);
        assert_eq!(metrics.capture_nonblank_validated, 1);
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
