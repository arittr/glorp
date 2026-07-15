# Glorp Retained Renderer Fail-Visible Behavior — design

**Date:** 2026-07-15
**Status:** Behavior approved; ready for implementation planning
**Scope:** Native macOS companion renderer selection and terminal retained-renderer failures

## Decision

Once the companion selects the retained renderer, it must never automatically
replace it with Smooth. Smooth remains available only through an explicit
renderer selection for comparison and debugging.

The retained runtime keeps its existing bounded recovery for failures it can
repair within the retained renderer, such as rebuilding a surface or device.
When that recovery is exhausted, the host enters a terminal frozen state:

- keep the retained view and its last successfully presented frame;
- stop further retained scene updates and presents for that process;
- record the exact sanitized failure category once;
- do not create, restore, paint, or acknowledge a Smooth fallback.

This makes a renderer failure visible as a frozen retained scene instead of
silently changing the product being tested.

## Startup Behavior

If retained initialization fails before any frame can be presented, companion
startup fails with the retained failure category. It must not open or continue
with a Smooth scene. The existing explicit Smooth launch path remains the way to
run without retained rendering.

## Runtime State Contract

Renderer selection and renderer health are separate state:

- selection remains `Retained` for the lifetime of the process;
- health may transition from active to terminally failed;
- the terminal state retains the failure category and is idempotent;
- later ticks, resize events, display moves, and fullscreen events do not retry
  or change renderer after a terminal failure;
- relaunch is the recovery boundary after a terminal failure.

Transient recovery owned by the retained scene runtime is unchanged. Only the
current escalation from retained failure to host-level Smooth fallback is
removed.

## Diagnostics

The first terminal failure writes one boundary diagnostic containing the
sanitized `RetainedFailureCategory`. Repeated event-loop callbacks must not spam
the same diagnostic. Existing capability reporting continues to say the
effective renderer and scene route are retained/direct; renderer failure is
reported independently rather than disguised as an effective-renderer change.

## Testing Contract

Coverage must prove:

1. retained initialization failure returns an error and never selects Smooth;
2. a terminal runtime failure preserves retained selection and records the
   failure category;
3. repeated failure handling is idempotent;
4. post-failure rendering does not produce a Smooth paint or mutate the frozen
   retained frame;
5. explicit `--renderer smooth` behavior is unchanged;
6. existing bounded retained surface/device recovery still passes its tests.

Fault-injection and soak assertions that currently treat a Smooth paint as a
successful fallback must instead assert the terminal retained failure state.

## Non-Goals

- Removing the Smooth renderer or its explicit development launch path.
- Reworking retained surface/device recovery policy.
- Adding an in-window error overlay or automatic retry loop.
- Changing scene composition, animation, resize, display, or fullscreen logic.
