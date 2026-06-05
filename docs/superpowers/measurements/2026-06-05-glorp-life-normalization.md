# Glorp Life Normalization Measurement

Date: 2026-06-05

The first Branch 2 implementation uses a session-local reference pace:

- default reference: 60,000 effective tokens/minute
- minimum reference: 5,000 effective tokens/minute
- EMA alpha: 0.35
- display range: 0.0..=2.0
- burst range: 0.0..=1.5
- idle decay: multiply by 0.82 per idle minute

The curve is `2 * ratio / (1 + ratio)`, where `ratio = current_tokens_per_minute / reference`.

Representative static-curve outputs using the default 60,000 effective tokens/minute reference:

| Signal | Tokens | Elapsed | Pace | Approx level |
| --- | ---: | ---: | ---: | ---: |
| idle | 0 | 10s | 0/min | 0.00 |
| warm | 5,000 | 10s | 30,000/min | 0.67 |
| hot | 80,000 | 10s | 480,000/min | 1.78 |
| very hot | 200,000 | 10s | 1,200,000/min | 1.90 |

Returned profile values may differ from the static table because `LifeSignalState::observe` updates the session reference with EMA before calculating the returned activity and burst. Burst uses the same curve but is capped at 1.5 to match the profile contract.

This leaves visible room between warm/hot/very-hot without requiring persistent calibration. Cold-start, backfill, and diagnostics-only signals can update activity slowly if desired, but they do not create burst.
