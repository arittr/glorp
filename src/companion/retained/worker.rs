use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::autoreleasepool;
use objc2_foundation::NSThread;

use super::resources::CompiledRetainedResourcesPreparation;
use super::resources::{CompiledRetainedResources, GlyphRepertoireManifest};
use super::RetainedFailureCategory;

pub(super) struct RasterJob {
    pub(super) job_id: u64,
    pub(super) manifest: GlyphRepertoireManifest,
    #[cfg(test)]
    force_font_unavailable: bool,
}

impl RasterJob {
    pub(super) fn new(job_id: u64, manifest: GlyphRepertoireManifest) -> Self {
        Self {
            job_id,
            manifest,
            #[cfg(test)]
            force_font_unavailable: false,
        }
    }

    #[cfg(test)]
    fn with_unavailable_font_for_test(job_id: u64, manifest: GlyphRepertoireManifest) -> Self {
        Self {
            job_id,
            manifest,
            force_font_unavailable: true,
        }
    }
}

pub(super) enum RasterReply {
    Completed {
        job_id: u64,
        resources: CompiledRetainedResources,
        timing: RasterWorkerTiming,
    },
    Cancelled {
        job_id: u64,
        timing: RasterWorkerTiming,
    },
    Failed {
        job_id: u64,
        category: RetainedFailureCategory,
        timing: RasterWorkerTiming,
    },
    WorkerPanicked {
        job_id: u64,
        timing: RasterWorkerTiming,
    },
}

impl RasterReply {
    fn job_id(&self) -> u64 {
        match self {
            Self::Completed { job_id, .. }
            | Self::Cancelled { job_id, .. }
            | Self::Failed { job_id, .. }
            | Self::WorkerPanicked { job_id, .. } => *job_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RasterWorkerTiming {
    pub(super) active_compile: Duration,
    pub(super) raster_calls: u32,
    pub(super) main_thread_raster_calls: u32,
}

impl RasterWorkerTiming {
    fn since(started_at: Instant, attempts: RasterAttemptCounts) -> Self {
        Self {
            active_compile: started_at.elapsed(),
            raster_calls: attempts.raster_calls,
            main_thread_raster_calls: attempts.main_thread_raster_calls,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct RasterAttemptCounts {
    raster_calls: u32,
    main_thread_raster_calls: u32,
}

pub(super) enum RasterSubmitError {
    Busy(RasterJob),
    Stale(RasterJob),
    Disconnected(RasterJob),
}

impl fmt::Debug for RasterSubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, job_id) = match self {
            Self::Busy(job) => ("Busy", job.job_id),
            Self::Stale(job) => ("Stale", job.job_id),
            Self::Disconnected(job) => ("Disconnected", job.job_id),
        };
        f.debug_struct(kind).field("job_id", &job_id).finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RasterWorkerThreadIdentity {
    pub(super) is_main_thread: bool,
    pub(super) cocoa_multithreaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RasterWorkerLaunchError {
    StartupDisconnected,
    StartupTimedOut,
    InvalidThreadIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RasterWorkerDisconnected {
    ReplyMailbox,
    Panicked,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RasterCancellationError {
    NonMonotonic,
}

enum RasterCommand {
    Compile(RasterJob),
    #[cfg(test)]
    Panic {
        job_id: u64,
    },
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerExit {
    Shutdown,
    ReplyDisconnected,
    Panicked,
}

struct WorkerThreadState {
    commands: Receiver<RasterCommand>,
    replies: SyncSender<RasterReply>,
    started: SyncSender<RasterWorkerThreadIdentity>,
    done: SyncSender<WorkerExit>,
    cancel_epoch: Arc<AtomicU64>,
    exited: Arc<AtomicBool>,
}

pub(super) struct RasterWorker {
    commands: Option<SyncSender<RasterCommand>>,
    replies: Receiver<RasterReply>,
    done: Receiver<WorkerExit>,
    cancel_epoch: Arc<AtomicU64>,
    identity: RasterWorkerThreadIdentity,
    in_flight: Option<u64>,
    last_accepted_job_id: u64,
    latest_epoch: u64,
    #[cfg(test)]
    exited: Arc<AtomicBool>,
}

impl RasterWorker {
    pub(super) fn launch() -> Result<Self, RasterWorkerLaunchError> {
        let (command_tx, command_rx) = mpsc::sync_channel(1);
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let cancel_epoch = Arc::new(AtomicU64::new(0));
        let exited = Arc::new(AtomicBool::new(false));
        let thread_state = Mutex::new(Some(WorkerThreadState {
            commands: command_rx,
            replies: reply_tx,
            started: started_tx,
            done: done_tx,
            cancel_epoch: cancel_epoch.clone(),
            exited: exited.clone(),
        }));
        let block = RcBlock::new(move || {
            let state = thread_state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            let Some(state) = state else {
                return;
            };
            let done = state.done.clone();
            let exited = state.exited.clone();
            let exit = catch_unwind(AssertUnwindSafe(|| autoreleasepool(|_| run_worker(state))))
                .unwrap_or(WorkerExit::Panicked);
            let _ = done.try_send(exit);
            exited.store(true, Ordering::Release);
        });
        // SAFETY: Foundation copies the heap block for the lifetime of the
        // detached NSThread. Its captures are Send plain-Rust channel/atomic
        // values, and the block catches Rust panics before returning through FFI.
        unsafe { NSThread::detachNewThreadWithBlock(&block) };

        let identity = match started_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(identity) => identity,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(RasterWorkerLaunchError::StartupDisconnected)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(RasterWorkerLaunchError::StartupTimedOut)
            }
        };
        if identity.is_main_thread || !identity.cocoa_multithreaded {
            return Err(RasterWorkerLaunchError::InvalidThreadIdentity);
        }
        Ok(Self {
            commands: Some(command_tx),
            replies: reply_rx,
            done: done_rx,
            cancel_epoch,
            identity,
            in_flight: None,
            last_accepted_job_id: 0,
            latest_epoch: 0,
            #[cfg(test)]
            exited,
        })
    }

    pub(super) fn thread_identity(&self) -> RasterWorkerThreadIdentity {
        self.identity
    }

    pub(super) fn try_submit(&mut self, job: RasterJob) -> Result<(), RasterSubmitError> {
        if job.job_id == 0
            || job.job_id == u64::MAX
            || job.job_id <= self.last_accepted_job_id
            || job.job_id < self.latest_epoch
        {
            return Err(RasterSubmitError::Stale(job));
        }
        if job.job_id > self.latest_epoch {
            self.latest_epoch = job.job_id;
            self.cancel_epoch.store(job.job_id, Ordering::Release);
        }
        if self.in_flight.is_some() {
            return Err(RasterSubmitError::Busy(job));
        }
        let job_id = job.job_id;
        let Some(commands) = &self.commands else {
            return Err(RasterSubmitError::Disconnected(job));
        };
        match commands.try_send(RasterCommand::Compile(job)) {
            Ok(()) => {
                self.in_flight = Some(job_id);
                self.last_accepted_job_id = job_id;
                Ok(())
            }
            Err(TrySendError::Full(RasterCommand::Compile(job))) => {
                Err(RasterSubmitError::Busy(job))
            }
            Err(TrySendError::Disconnected(RasterCommand::Compile(job))) => {
                self.commands = None;
                Err(RasterSubmitError::Disconnected(job))
            }
            Err(
                TrySendError::Full(RasterCommand::Shutdown)
                | TrySendError::Disconnected(RasterCommand::Shutdown),
            ) => {
                unreachable!("try_submit only sends compile commands")
            }
            #[cfg(test)]
            Err(
                TrySendError::Full(RasterCommand::Panic { .. })
                | TrySendError::Disconnected(RasterCommand::Panic { .. }),
            ) => {
                unreachable!("try_submit only sends compile commands")
            }
        }
    }

    #[cfg(test)]
    fn panic_for_test(&mut self, job_id: u64) {
        self.latest_epoch = job_id;
        self.cancel_epoch.store(job_id, Ordering::Release);
        self.in_flight = Some(job_id);
        self.last_accepted_job_id = job_id;
        self.commands
            .as_ref()
            .expect("test worker remains connected")
            .try_send(RasterCommand::Panic { job_id })
            .expect("test worker command mailbox is empty");
    }

    #[cfg(test)]
    fn exit_probe(&self) -> Arc<AtomicBool> {
        self.exited.clone()
    }

    pub(super) fn cancel_with_epoch(&mut self, epoch: u64) -> Result<(), RasterCancellationError> {
        if epoch == u64::MAX || epoch <= self.latest_epoch {
            return Err(RasterCancellationError::NonMonotonic);
        }
        self.latest_epoch = epoch;
        self.cancel_epoch.store(epoch, Ordering::Release);
        Ok(())
    }

    pub(super) fn try_recv(&mut self) -> Result<Option<RasterReply>, RasterWorkerDisconnected> {
        match self.replies.try_recv() {
            Ok(reply) => {
                if self.in_flight == Some(reply.job_id()) {
                    self.in_flight = None;
                }
                Ok(Some(reply))
            }
            Err(TryRecvError::Empty) => match self.done.try_recv() {
                Ok(WorkerExit::Panicked) => Err(RasterWorkerDisconnected::Panicked),
                Ok(WorkerExit::ReplyDisconnected) => Err(RasterWorkerDisconnected::ReplyMailbox),
                Ok(WorkerExit::Shutdown) => Err(RasterWorkerDisconnected::Stopped),
                Err(TryRecvError::Empty) => Ok(None),
                Err(TryRecvError::Disconnected) if self.commands.is_none() => {
                    Err(RasterWorkerDisconnected::Stopped)
                }
                Err(TryRecvError::Disconnected) => Ok(None),
            },
            Err(TryRecvError::Disconnected) => Err(RasterWorkerDisconnected::ReplyMailbox),
        }
    }
}

impl Drop for RasterWorker {
    fn drop(&mut self) {
        self.latest_epoch = self.latest_epoch.saturating_add(1);
        self.cancel_epoch
            .store(self.latest_epoch, Ordering::Release);
        if let Some(commands) = self.commands.take() {
            let _ = commands.try_send(RasterCommand::Shutdown);
            drop(commands);
        }
    }
}

fn run_worker(state: WorkerThreadState) -> WorkerExit {
    let identity = RasterWorkerThreadIdentity {
        is_main_thread: NSThread::currentThread().isMainThread(),
        cocoa_multithreaded: NSThread::isMultiThreaded(),
    };
    if state.started.try_send(identity).is_err() {
        return WorkerExit::Shutdown;
    }
    loop {
        let command = match state.commands.recv() {
            Ok(command) => command,
            Err(_) => return WorkerExit::Shutdown,
        };
        let (job_id, compiled) = match command {
            RasterCommand::Compile(job) => {
                let job_id = job.job_id;
                let started_at = Instant::now();
                let mut attempts = RasterAttemptCounts::default();
                let compiled = catch_unwind(AssertUnwindSafe(|| {
                    compile_job(
                        job,
                        state.cancel_epoch.as_ref(),
                        started_at,
                        &mut attempts,
                        identity.is_main_thread,
                    )
                }));
                (
                    job_id,
                    compiled.map_err(|payload| (payload, started_at, attempts)),
                )
            }
            #[cfg(test)]
            RasterCommand::Panic { job_id } => {
                let started_at = Instant::now();
                let panicked = catch_unwind(AssertUnwindSafe(|| -> RasterReply {
                    panic!("injected raster worker panic")
                }));
                let panicked = panicked
                    .map_err(|payload| (payload, started_at, RasterAttemptCounts::default()));
                (job_id, panicked)
            }
            RasterCommand::Shutdown => return WorkerExit::Shutdown,
        };
        let (reply, panicked) = match compiled {
            Ok(reply) => (reply, false),
            Err((_, started_at, attempts)) => (
                RasterReply::WorkerPanicked {
                    job_id,
                    timing: RasterWorkerTiming::since(started_at, attempts),
                },
                true,
            ),
        };
        if state.replies.try_send(reply).is_err() {
            return WorkerExit::ReplyDisconnected;
        }
        if panicked {
            return WorkerExit::Panicked;
        }
    }
}

fn compile_job(
    job: RasterJob,
    cancel_epoch: &AtomicU64,
    started_at: Instant,
    attempts: &mut RasterAttemptCounts,
    is_main_thread: bool,
) -> RasterReply {
    compile_job_with_checkpoint(
        job,
        cancel_epoch,
        started_at,
        attempts,
        is_main_thread,
        &mut |_| {},
    )
}

fn compile_job_with_checkpoint(
    job: RasterJob,
    cancel_epoch: &AtomicU64,
    started_at: Instant,
    attempts: &mut RasterAttemptCounts,
    is_main_thread: bool,
    after_glyph: &mut impl FnMut(u32),
) -> RasterReply {
    let job_id = job.job_id;
    if cancel_epoch.load(Ordering::Acquire) != job_id {
        return RasterReply::Cancelled {
            job_id,
            timing: RasterWorkerTiming::since(started_at, *attempts),
        };
    }
    #[cfg(test)]
    let preparation = if job.force_font_unavailable {
        CompiledRetainedResourcesPreparation::new_with_font_resolver(&job.manifest, &mut |_, _| {
            None
        })
    } else {
        CompiledRetainedResourcesPreparation::new(&job.manifest)
    };
    #[cfg(not(test))]
    let preparation = CompiledRetainedResourcesPreparation::new(&job.manifest);
    let mut preparation = match preparation {
        Ok(preparation) => preparation,
        Err(category) => {
            return RasterReply::Failed {
                job_id,
                category,
                timing: RasterWorkerTiming::since(started_at, *attempts),
            }
        }
    };
    loop {
        if cancel_epoch.load(Ordering::Acquire) != job_id {
            return RasterReply::Cancelled {
                job_id,
                timing: RasterWorkerTiming::since(started_at, *attempts),
            };
        }
        attempts.raster_calls = attempts.raster_calls.saturating_add(1);
        if is_main_thread {
            attempts.main_thread_raster_calls = attempts.main_thread_raster_calls.saturating_add(1);
        }
        let complete = match preparation.advance_one() {
            Ok(complete) => complete,
            Err(category) => {
                return RasterReply::Failed {
                    job_id,
                    category,
                    timing: RasterWorkerTiming::since(started_at, *attempts),
                }
            }
        };
        after_glyph(attempts.raster_calls);
        if complete {
            break;
        }
    }
    if cancel_epoch.load(Ordering::Acquire) != job_id {
        return RasterReply::Cancelled {
            job_id,
            timing: RasterWorkerTiming::since(started_at, *attempts),
        };
    }
    match preparation.finish() {
        Ok(resources) => RasterReply::Completed {
            job_id,
            resources,
            timing: RasterWorkerTiming::since(started_at, *attempts),
        },
        Err(category) => RasterReply::Failed {
            job_id,
            category,
            timing: RasterWorkerTiming::since(started_at, *attempts),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, MutexGuard};
    use std::time::{Duration, Instant};

    use super::{
        compile_job_with_checkpoint, RasterAttemptCounts, RasterJob, RasterReply,
        RasterSubmitError, RasterWorker,
    };
    use crate::companion::retained::resources::{
        CompiledRetainedResources, GlyphAtlasEntry, GlyphRepertoireManifest,
    };
    use crate::pet::generation::Species;
    use crate::round::smooth::CompanionContentIdentity;

    fn assert_send<T: Send>() {}

    static WORKER_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn worker_test_guard() -> MutexGuard<'static, ()> {
        WORKER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn manifest(species: Species, backing_scale: f64) -> GlyphRepertoireManifest {
        GlyphRepertoireManifest::for_active_pet(
            CompanionContentIdentity::for_pet(species),
            backing_scale,
        )
    }

    fn crystal_manifest() -> GlyphRepertoireManifest {
        manifest(Species::Crystal, 2.0)
    }

    fn wait_for_reply(worker: &mut RasterWorker) -> RasterReply {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match worker.try_recv().expect("worker remains connected") {
                Some(reply) => return reply,
                None if Instant::now() < deadline => std::thread::yield_now(),
                None => panic!("timed out waiting for raster worker"),
            }
        }
    }

    fn assert_entry_eq(worker: &GlyphAtlasEntry, synchronous: &GlyphAtlasEntry) {
        assert_eq!(worker.visible_uv, synchronous.visible_uv);
        assert_eq!(worker.ink_origin, synchronous.ink_origin);
        assert_eq!(worker.ink_size, synchronous.ink_size);
        assert_eq!(worker.baseline, synchronous.baseline);
        assert_eq!(worker.ascent, synchronous.ascent);
        assert_eq!(worker.descent, synchronous.descent);
        assert_eq!(worker.line_height, synchronous.line_height);
        assert_eq!(worker.advance, synchronous.advance);
        assert_eq!(worker.raster_size, synchronous.raster_size);
        assert_eq!(worker.safe_padding, synchronous.safe_padding);
        assert_eq!(worker.font_policy_id, synchronous.font_policy_id);
        assert_eq!(worker.kind, synchronous.kind);
    }

    fn assert_resources_eq(
        worker: &CompiledRetainedResources,
        synchronous: &CompiledRetainedResources,
    ) {
        assert_eq!(worker.generation(), synchronous.generation());
        let worker = worker.atlas();
        let synchronous = synchronous.atlas();
        assert_eq!(worker.width, synchronous.width);
        assert_eq!(worker.height, synchronous.height);
        assert_eq!(worker.rgba, synchronous.rgba);
        assert_eq!(worker.entries.len(), synchronous.entries.len());
        for (key, worker_entry) in &worker.entries {
            let synchronous_entry = synchronous.entries.get(key).expect("matching atlas key");
            assert_entry_eq(worker_entry, synchronous_entry);
        }
    }

    #[test]
    fn worker_job_and_reply_are_plain_rust_send_values() {
        let _guard = worker_test_guard();
        assert_send::<RasterJob>();
        assert_send::<RasterReply>();

        let manifest = crystal_manifest();
        assert_eq!(manifest.glyphs().len(), 242);
    }

    #[test]
    fn worker_runs_off_main_with_cocoa_multithreading_enabled() {
        let _guard = worker_test_guard();
        let worker = RasterWorker::launch().expect("NSThread worker launches");
        let identity = worker.thread_identity();
        assert!(!identity.is_main_thread);
        assert!(identity.cocoa_multithreaded);
        assert!(objc2_foundation::NSThread::isMultiThreaded());
    }

    #[test]
    fn worker_atlas_matches_synchronous_compile_for_all_species_and_scales() {
        let _guard = worker_test_guard();
        let mut worker = RasterWorker::launch().expect("NSThread worker launches");
        let mut job_id = 1;
        for species in Species::all() {
            for backing_scale in [1.0, 2.0] {
                let synchronous_first = job_id % 2 == 1;
                let synchronous = synchronous_first.then(|| {
                    CompiledRetainedResources::compile(&manifest(species, backing_scale)).unwrap()
                });
                worker
                    .try_submit(RasterJob::new(job_id, manifest(species, backing_scale)))
                    .expect("idle worker accepts one job");
                let RasterReply::Completed { resources, .. } = wait_for_reply(&mut worker) else {
                    panic!("worker did not complete atlas for {species:?}@{backing_scale}");
                };
                let synchronous = synchronous.unwrap_or_else(|| {
                    CompiledRetainedResources::compile(&manifest(species, backing_scale)).unwrap()
                });
                assert_resources_eq(&resources, &synchronous);
                job_id += 1;
            }
        }
    }

    #[test]
    fn completed_reply_carries_worker_owned_active_compile_time() {
        let _guard = worker_test_guard();
        let mut worker = RasterWorker::launch().expect("NSThread worker launches");
        worker
            .try_submit(RasterJob::new(1, crystal_manifest()))
            .expect("idle worker accepts one job");
        let RasterReply::Completed { timing, .. } = wait_for_reply(&mut worker) else {
            panic!("worker did not complete atlas");
        };
        assert!(timing.active_compile > Duration::ZERO);
        assert_eq!(timing.raster_calls, 242);
        assert_eq!(timing.main_thread_raster_calls, 0);
    }

    #[test]
    fn busy_worker_leaves_latest_job_owned_by_submitter() {
        let _guard = worker_test_guard();
        let mut worker = RasterWorker::launch().expect("NSThread worker launches");
        worker
            .try_submit(RasterJob::new(1, crystal_manifest()))
            .expect("idle worker accepts one job");
        let error = worker
            .try_submit(RasterJob::new(2, crystal_manifest()))
            .expect_err("one in-flight job bounds the command side");
        let RasterSubmitError::Busy(job) = error else {
            panic!("second job was not rejected as busy");
        };
        assert_eq!(job.job_id, 2);
        assert_eq!(job.manifest.glyphs().len(), 242);
        match wait_for_reply(&mut worker) {
            RasterReply::Cancelled { job_id: 1, timing } => assert!(timing.raster_calls <= 242),
            RasterReply::Completed { job_id: 1, timing, .. } => {
                assert_eq!(timing.raster_calls, 242)
            }
            _ => panic!("superseded in-flight job had an unexpected terminal reply"),
        }
        worker
            .try_submit(job)
            .expect("caller-owned latest job remains submit-ready");
        assert!(matches!(
            wait_for_reply(&mut worker),
            RasterReply::Completed { job_id: 2, .. }
        ));
    }

    #[test]
    fn cancellation_epoch_cancels_an_in_flight_compile() {
        let _guard = worker_test_guard();
        let mut worker = RasterWorker::launch().expect("NSThread worker launches");
        worker
            .try_submit(RasterJob::new(1, crystal_manifest()))
            .expect("idle worker accepts one job");
        worker
            .cancel_with_epoch(2)
            .expect("newer epoch cancels job");
        match wait_for_reply(&mut worker) {
            RasterReply::Cancelled { job_id: 1, timing } => {
                assert!(timing.raster_calls <= 242);
                assert_eq!(timing.main_thread_raster_calls, 0);
            }
            RasterReply::Completed { job_id: 1, timing, .. } => {
                assert_eq!(timing.raster_calls, 242);
                assert_eq!(timing.main_thread_raster_calls, 0);
            }
            _ => panic!("cancel race had an unexpected terminal reply"),
        }
    }

    #[test]
    fn cancellation_checkpoint_stops_after_exactly_one_glyph() {
        let _guard = worker_test_guard();
        let cancel_epoch = AtomicU64::new(1);
        let mut attempts = RasterAttemptCounts::default();
        let mut checkpoint = |completed: u32| {
            if completed == 1 {
                cancel_epoch.store(2, Ordering::Release);
            }
        };
        let reply = compile_job_with_checkpoint(
            RasterJob::new(1, crystal_manifest()),
            &cancel_epoch,
            Instant::now(),
            &mut attempts,
            false,
            &mut checkpoint,
        );
        let RasterReply::Cancelled { timing, .. } = reply else {
            panic!("checkpoint cancellation did not cancel");
        };
        assert_eq!(timing.raster_calls, 1);
        assert_eq!(timing.main_thread_raster_calls, 0);
    }

    #[test]
    fn worker_panic_has_a_distinct_terminal_reply() {
        let _guard = worker_test_guard();
        let mut worker = RasterWorker::launch().expect("NSThread worker launches");
        worker.panic_for_test(1);
        assert!(matches!(
            wait_for_reply(&mut worker),
            RasterReply::WorkerPanicked { job_id: 1, .. }
        ));
    }

    #[test]
    fn typed_font_failure_does_not_terminate_worker() {
        let _guard = worker_test_guard();
        let mut worker = RasterWorker::launch().expect("NSThread worker launches");
        worker
            .try_submit(RasterJob::with_unavailable_font_for_test(
                1,
                crystal_manifest(),
            ))
            .expect("idle worker accepts injected typed failure");
        assert!(matches!(
            wait_for_reply(&mut worker),
            RasterReply::Failed {
                job_id: 1,
                category: crate::companion::retained::RetainedFailureCategory::FontUnavailable,
                ..
            }
        ));

        worker
            .try_submit(RasterJob::new(2, crystal_manifest()))
            .expect("typed failure leaves worker available for another job");
        assert!(matches!(
            wait_for_reply(&mut worker),
            RasterReply::Completed { job_id: 2, .. }
        ));
    }

    #[test]
    fn drop_during_compile_is_nonblocking_and_worker_disconnect_is_bounded() {
        let _guard = worker_test_guard();
        let mut worker = RasterWorker::launch().expect("NSThread worker launches");
        let exited = worker.exit_probe();
        worker
            .try_submit(RasterJob::new(1, crystal_manifest()))
            .expect("idle worker accepts one job");

        let drop_started = Instant::now();
        drop(worker);
        assert!(
            drop_started.elapsed() < Duration::from_millis(50),
            "Drop blocked the caller for {:?}",
            drop_started.elapsed()
        );

        let deadline = Instant::now() + Duration::from_secs(2);
        while !exited.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(exited.load(Ordering::Acquire), "worker did not shut down");
    }
}
