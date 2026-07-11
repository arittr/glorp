# Glorp Renderer Energy Qualification

**Date:** 2026-07-10
**Gate:** corrected wgpu energy no worse than Smooth
**Status:** privileged measurement stopped by decision owner after 2 of 12 corrected runs; energy gate unresolved
**Decision owner:** repository owner/user
**Measurement host:** same arm64 MacBook Pro (`Mac17,9`, Apple M5 Pro) for both candidates

The repository owner approved privileged `powermetrics` measurement on 2026-07-10. The real one-sample schema probe passed (one 1.035975625-second plist record, 533,099 bytes), and the frozen parser verified the observed CPU/GPU millijoule and milliwatt relation within 2%. The final non-overwriting orchestrator is `target/renderer-spikes/wgpu-qualified-energy/run_powermetrics_matrix.py`; it is pinned to final arm64 SHA-256 `3356eb23876ab4bb388ac8767e1b14dc10b5c940b097d75c387125e0fd9b6c79`, performs the declared rotation, uses one 30-second warmup plus 300 one-second samples per run, validates renderer artifacts immediately, and bounds every root collector.

The first full invocation stopped after its first Smooth run. The renderer itself completed 5,025 samples with `functional-pass`, cleanup passed, and `powermetrics` produced all 300 records (174,417,619 bytes; 317.538770202 seconds). The orchestration script had incorrectly placed the machine-wide raw plist inside the renderer-owned output directory before renderer finalization. The renderer privacy scanner correctly encountered the forbidden synthetic/external token `diagnostic`, rejected finalization, and did not write `run-manifest.json`; xtask therefore exited 1. This is an owned measurement-orchestration defect, not a renderer or energy result. The failed root and exact defect record are preserved under `target/renderer-spikes/wgpu-qualified-energy/final-3356eb23/` and are ineligible for final evidence.

The minimal correction stores collector data under root-level `raw/` and parsed results under root-level `parsed/`, outside each renderer-owned run directory. No renderer source or frozen binary changed. The corrected runner targets the fresh non-overwriting root `target/renderer-spikes/wgpu-qualified-energy/final-3356eb23-corrected/`. Syntax, parser replay against both the schema probe and the preserved 300-record failed collection, protocol assertions, uncertainty self-test, diff check, and exact-executable cleanup check pass.

The decision owner stopped the corrected matrix on 2026-07-10 because completing the approximately 75-minute protocol was not worth delaying real-pet renderer integration. Two accepted records exist—360 block 1 Smooth and 360 block 1 wgpu—and a third raw collection was interrupted before validation. This is only 2 of the predeclared 12 accepted runs, so no aggregate, uncertainty estimate, or energy pass/fail claim is valid. `progress.json` remains incomplete by design. Any future energy qualification must use a fresh non-overwriting root and rerun the complete matrix from the beginning; neither the invalid first root nor this partial corrected root may be reused as qualified evidence.

## Available methods

`powermetrics` is present at `/usr/bin/powermetrics` and supports machine-readable plist output, CPU/GPU power samplers, and per-process energy-impact fields. On this host it requires superuser execution.

Three unprivileged observations agree:

- `target/renderer-spikes/energy-check/powermetrics.stderr.txt`: `powermetrics must be invoked as the superuser`;
- `target/renderer-spikes/software-energy/powermetrics.stderr.txt`: the same result;
- a bounded one-sample probe on 2026-07-10 exited status 1 with the same message and an empty output file.

Xcode Instruments Energy Log is not available as a reviewed alternative: `xcrun xctrace list templates` cannot find the developer tool in the active environment. `top`, `ps`, process CPU, and frame CPU are not energy counters and will not be substituted.

## Proposed measured protocol

Use `/usr/bin/powermetrics` consistently for both candidates after Task 52 freezes the exact final startup-corrected arm64 binary. Do not use the superseded `e5033840f4b3e2e62e45550dff4ba90f82b82ed4e498a9c957b2104c792e38cf` binary.

### Conditions

For each accepted run:

- AC power for every candidate in every block;
- battery state and charge recorded before and after;
- same display, backing scale, logical size, frontmost state, and visibility;
- no occlusion and no unrelated foreground workload;
- 30-second workload warmup followed by a five-minute measured ambient interval;
- one-second power samples;
- same exact binary identity for Smooth and wgpu;
- output directory must not already exist;
- immediate artifact validation and process cleanup.

Run both logical sizes with the predeclared rotation:

```text
360 block 1: smooth, wgpu
360 block 2: wgpu, smooth
360 block 3: smooth, wgpu
720 block 1: smooth, wgpu
720 block 2: wgpu, smooth
720 block 3: smooth, wgpu
```

Use the same cooldown policy as final matched qualification. Reject and rerun a whole paired block for power-source changes, thermal pressure divergence, visibility loss, wrong binary identity, failed privacy/cleanup, missing samples, or a collection failure.

### Privileged collector

For each measured five-minute workload, run one bounded collector alongside the renderer process:

```bash
sudo /usr/bin/powermetrics \
  --samplers cpu_power,gpu_power,thermal,battery,tasks \
  --show-process-energy \
  --show-process-gpu \
  --show-process-samp-norm \
  --handle-invalid-values \
  --sample-rate 1000 \
  --sample-count 300 \
  --format plist \
  --buffer-size 1 \
  --output-file RAW.plist
```

The command is bounded to 300 samples. It must be invoked once after permission/environment is established, through an owned orchestration script that starts the warmed renderer and collector, records both exit statuses, and terminates either side on timeout. No indefinite root process is allowed.

Retain under `target/renderer-spikes/wgpu-qualified-energy/`:

- exact script and sanitized command transcript;
- frozen binary identity and qualification protocol;
- environment/power/frontmost/thermal evidence;
- raw NUL-separated plist stream;
- workload stdout/stderr and renderer artifacts;
- parser source and parsed per-sample JSON/CSV;
- aggregate JSON;
- validation, privacy, and cleanup evidence.

### Normalization and comparison

Parse every plist record without dropping invalid samples silently. Report separately:

- CPU power, GPU power, and their sum for each valid sample;
- process energy-impact field as diagnostic only;
- sample count, invalid/missing count, median, arithmetic mean, nearest-rank p95, minimum, and maximum;
- energy over the measured interval as the time integral of CPU+GPU power (`sum(power_W × sample_seconds)`), with units recorded from the plist schema;
- candidate-to-Smooth percent delta within each paired block;
- median of the three paired percent deltas for each size;
- between-block spread and uncertainty.

The primary gate is same-machine, same-size CPU+GPU energy over the measured interval. wgpu passes only if its result is no worse than Smooth within the predeclared measurement uncertainty at both 360 and 720. Process CPU is reported separately and is not used as the energy result.

Before executing the full matrix, inspect one privileged one-sample plist to lock the actual field names and units, then freeze and test the parser. That schema probe is part of the single approved measurement setup, not a renderer result.

## Exception proposal if privileged measurement is not approved or cannot run

**Gate:** corrected wgpu energy no worse than Smooth
**Evidence:** unavailable; the reviewed counter requires superuser execution and no reviewed unprivileged energy export is available
**Decision owner:** repository owner/user
**Approval date:** pending
**Decision:** pending — APPROVE TEMPORARY EXCEPTION / REJECT
**Affected scope:** backend selection and any renderer-enabled macOS release; all CPU/frame/resource gates remain mandatory
**Expiry:** the earlier of 2026-08-10 or the next renderer-enabled release qualification
**Risk:** lower process CPU does not prove lower CPU+GPU or whole-SoC energy. wgpu could increase GPU/package power, battery drain, heat, or thermal pressure despite passing CPU/frame gates.
**Required closure:** execute the complete same-machine `powermetrics` protocol above against the exact final corrected binary, preserve raw samples/parser/aggregates, and obtain a no-worse-than-Smooth result at both 360 and 720.
**Invalidation:** any renderer, wgpu version/features, shader, cadence, fixture, or hardware/power-method change requires new evidence.
**Fallback:** if neither privileged measurement nor this exception is explicitly approved before the decision ceiling, select no backend. Smooth remains default.

The implementation agent recommends measured evidence over the exception and cannot approve its own exception.
