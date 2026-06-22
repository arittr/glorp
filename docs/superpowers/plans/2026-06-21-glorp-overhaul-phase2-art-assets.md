# Glorp Overhaul — Phase 2: Art Assets (Validated Base Silhouettes + Mood Faces)

Status: **validated, ready to consume.** Date: 2026-06-21.

## What this is

This document is the machine-validated art payload consumed by Phase 2 of the glorp
overhaul: the six species' base silhouettes across the full growth arc (S0 → S6, seven
templates each) plus the four S4 mood faces per species (resting / happy / tired /
wilted). It is the hand-off artifact between art authoring and the `src/pet/art.rs`
template tables.

**The art is data, not code.** Each template is a fixed `[&str; 8]` grid of display
cells; `src/pet/render.rs` substitutes the `{eyes}`, `{mouth}`, `{pattern}`, and
`{accent}` slot markers at render time and wraps the 11×8 art in the particle frame.
Swapping a species' look — or adding a morph — is a change to these grids alone. No
renderer change is required, because the renderer is content-agnostic: it only knows
about slot positions and the 11×8 invariant.

### Invariant guarantees (independently re-checked for every one of the 42 base templates)

- **Geometry:** every template is exactly **11 display columns × 8 lines**. The 8th
  row exists in all stages; only S6 is required to fill all 8 rows with body mass.
- **Width-1 glyphs:** every glyph occupies exactly one terminal cell. Eye lenses use
  `◉` (U+25C9, EAW-Neutral) rather than the ambiguous `◇◆◈●○` family in the base
  bodies. Shade/quadrant blocks (`░▒▓█▘▝▙▟▀▄▌▐` etc.) and the Crystal/Mech facet
  glyphs (`◇◆◈◌`) are EAW-Ambiguous and rely on the documented **`ambiguous=narrow`**
  rendering assumption to stay width-1.
- **Growth ramp:** within each species the non-space cell count is **strictly
  increasing** S0 → S6, and every count sits inside its stage's **disjoint band**:
  S0 `[1,4]`, S1 `[5,10]`, S2 `[11,20]`, S3 `[21,34]`, S4 `[35,50]`, S5 `[51,66]`,
  S6 `[67,88]`. The bands are disjoint, so a strictly-increasing in-band ramp is the
  visible silhouette-mass progression the player reads as growth.
- **Slot contract:** `{eyes}` is a 3-column slot, `{mouth}` and `{pattern}` are
  baked-in or 1/3-column slots per species. Mood faces honor the same slot widths —
  no mood widens a slot, and no mood uses `x x` corpse eyes. Crystal bakes its lens
  pair directly into the silhouette (a 2-glyph `◆◆`/`◇◇`/`◌◌` facet pair across the
  apex and face rows) rather than using a separate `{eyes}` slot.

### Spot-check repairs applied during this final pass

Three transcription-level defects were caught and fixed minimally while re-deriving
the grids; none changed a silhouette's design intent or its validated cell count:

- **Fuzz S3** locket cell was transcribed as `┌`; restored to the validated locket
  glyph `◌`. Count unchanged at 33.
- **Mech S6** bottom-row right corner was transcribed as `▟`; restored to the
  validated `▙` so the base seats symmetrically. Count unchanged at 69.
- **Crystal S4–S6** were briefly templatized with a 3-column `{eyes}`/`{mouth}` slot,
  which is wrong for Crystal — its lens is a 2-glyph facet pair baked into the
  silhouette, not a slot. Restored to the validated literal grids. Counts unchanged
  at 39 / 53 / 73.

Every other grid validated unmodified. Final result: **all 42 base templates pass
11×8 / band-membership / strict-monotonicity**, and all 24 mood-face rows are 11
columns wide.

---

## 1. Contact sheet

Each species' S0 → S6 arc is shown as it renders at rest (slot markers substituted
with canonical resting fillers), with the verified non-space cell count and stage
label under each frame so the growth ramp reads at a glance. The four S4 mood faces
follow each arc.

### Fuzz · Hearthfloof

Hearthfloof — twin ear-cones over a plush figure-ground loaf; a worn locket that fills with age (◌ → ◆ → ◈◈◈).

**S0 · fluff — 4 cells** (band S0 `[1,4]`)
```
           
           
           
           
           
    ▒▒     
    ▟▙     
           
```
**S1 · fuzzling — 10 cells** (band S1 `[5,10]`)
```
           
           
    ▟▙     
    ▒▒     
    ▓▓     
    ▒▒     
    ▘▝     
           
```
**S2 · kit — 19 cells** (band S2 `[11,20]`)
```
           
           
    ▟▙     
   ▓▒▒▓    
  ▒◉ ◉▒    
   ▒w▒     
   ▓▒▒▓    
    ▘▝     
```
**S3 · pup — 33 cells** (band S3 `[21,34]`)
```
           
   ▟▙▟▙    
  ▓▒▒▒▒▓   
  ▓▒◉ ◉▒▓  
  ▓▒ w ▒▓  
  ▓▒◌▒▒▓   
   ▙▒▒▟    
   ▘  ▝    
```
**S4 · fuzz — 50 cells** (band S4 `[35,50]`)
```
   ▟▙ ▟▙   
  ▓▒▒▒▒▒▓  
  ▓▒◉ ◉▒▒▓ 
  ▓▒ w ▒▒▓ 
  ▓▒▒◆▒▒▒▓ 
  ▓▒▒▒▒▒▒▓ 
  ▙▒▒▒▒▒▒▟ 
   ▘    ▝  
```
**S5 · archfuzz — 59 cells** (band S5 `[51,66]`)
```
  ▟█▙ ▟█▙  
  ▓▓▒▒▒▒▓▓ 
  ▓▒◉ ◉▒▒▓ 
  ▓▒ w ▒▒▓ 
  ▓▒▒◆◆▒▒▓ 
  ▓█▒▒▒▒█▓ 
  ▓█▒▒▒▒█▓ 
  ▙█▒▘▝▒█▟ 
```
**S6 · mythic-fuzz — 69 cells** (band S6 `[67,88]`)
```
 ▟██▙▟██▙  
 ▓██▒▒▒██▓ 
 ▓█▒◉ ◉▒█▓ 
 ▓█▒ w ▒█▓ 
 ▓█▒◈◈◈▒█▓ 
 ▓██▒▒▒██▓ 
 ▓██▒▒▒██▓ 
 ▙██▒▘▝▒██▟
```

**S4 mood faces — Fuzz** (eyes / mouth slot pairs, seated in the S4 frame):

- `resting` — eyes `◉ ◉` · mouth `w`
```
  ▓▒◉ ◉▒▒▓ 
  ▓▒ w ▒▒▓ 
```
- `happy` — eyes `^ ^` · mouth `ω`
```
  ▓▒^ ^▒▒▓ 
  ▓▒ ω ▒▒▓ 
```
- `tired` — eyes `˘ ˘` · mouth `⌣`
```
  ▓▒˘ ˘▒▒▓ 
  ▓▒ ⌣ ▒▒▓ 
```
- `wilted` — eyes `ˇ ˇ` · mouth `⌢`
```
  ▓▒ˇ ˇ▒▒▓ 
  ▓▒ ⌢ ▒▒▓ 
```

Ramp: `[4, 10, 19, 33, 50, 59, 69]` — strictly increasing, every value in-band.

---

### Blob · Deep-Light Medusa

Deep-Light Medusa — a translucent bioluminescent jelly bell housing a glowing organ-core, with a tendril curtain that grows downward.

**S0 · droplet — 3 cells** (band S0 `[1,4]`)
```
           
           
    ▄▄     
    ▒      
           
           
           
           
```
**S1 · blip — 7 cells** (band S1 `[5,10]`)
```
           
    ▄▄     
   ▟▒▙     
    ◉      
    |      
           
           
           
```
**S2 · globule — 15 cells** (band S2 `[11,20]`)
```
           
    ▄▄▄    
   ▟▒▒▙    
   ◉ ◉     
   ▒~▒     
   |╎|     
           
           
```
**S3 · wee-blob — 33 cells** (band S3 `[21,34]`)
```
    ▄▄▄    
   ▟▒▒▙    
  (▒◉ ◉▒)  
  (▒▒~▒▒)  
   ▒▓▒     
   ▒▓▒     
   |╎|┊    
   ' ' '   
```
**S4 · blob — 38 cells** (band S4 `[35,50]`)
```
    ▄▄▄    
   ▟▒▒▙    
  (▒◉ ◉▒)  
  (▒▒~▒▒)  
  (░▓◆▓░)  
   ▒▓▓▒    
   |╎|┊    
   ' ' '   
```
**S5 · mega-blob — 55 cells** (band S5 `[51,66]`)
```
   ▄▄▄▄    
  ▟▒▒▒▒▙   
 (▒▒◉ ◉▒▒) 
 (▒▒▒~▒▒▒) 
 (░▓◆◉◆▓░) 
 (░▒▓▓▓▒░) 
  |┊|╎|┊   
  ' ' ' '  
```
**S6 · primordial — 78 cells** (band S6 `[67,88]`)
```
 ▄▄▄▄▄▄▄▄▄ 
▟▒▒▒▒▒▒▒▒▒▙
▐▒▓███▓▒▒▒▌
▐▒◉ ◉▒▓▒░▌ 
(▒▒~▒▒▓▒░) 
(◆▓◉◆◉▓◆▒) 
▝▒░▒░▒░▒░▘ 
 |┊|╎|┊|╎  
```

**S4 mood faces — Blob** (eyes / mouth slot pairs, seated in the S4 frame):

- `resting` — eyes `◉ ◉` · mouth `~`
```
  (▒◉ ◉▒)  
  (▒▒~▒▒)  
```
- `happy` — eyes `^ ^` · mouth `ω`
```
  (▒^ ^▒)  
  (▒▒ω▒▒)  
```
- `tired` — eyes `- -` · mouth `-`
```
  (▒- -▒)  
  (▒▒-▒▒)  
```
- `wilted` — eyes `,_,` · mouth `_`
```
  (▒,_,▒)  
  (▒▒_▒▒)  
```

Ramp: `[3, 7, 15, 33, 38, 55, 78]` — strictly increasing, every value in-band.

---

### Ghost · The Pall

The Pall — a dense ▒▓█ shroud with a living face at rest; sealed wavering hem, S6 fills all 8 rows.

**S0 · whisper — 3 cells** (band S0 `[1,4]`)
```
           
           
           
           
           
           
    ▒▒▒    
           
```
**S1 · wisp — 9 cells** (band S1 `[5,10]`)
```
           
           
           
           
    ▄▄▄    
    ▒▒▒    
    \_/    
           
```
**S2 · shade — 18 cells** (band S2 `[11,20]`)
```
           
           
    ▄▄▄    
   ▒o o▒   
   ▒░o░▒   
    ▒▒▒    
    \_/    
           
```
**S3 · phantom-pup — 31 cells** (band S3 `[21,34]`)
```
    ╭─╮    
   ╭╯ ╰╮   
   ▒▒▒▒▒   
   ▒o o▒   
   ▒░o░▒   
   ▒▒▒▒▒   
   \_/\_   
           
```
**S4 · ghost — 44 cells** (band S4 `[35,50]`)
```
   ╭───╮   
  ╭╯   ╰╮  
  ░▒▒▒▒▒░  
  ░▒o o▒░  
  ░▒░o░▒░  
  ░▒ ▒ ▒░  
   ▒▒▒▒▒   
   \_/\_   
```
**S5 · wraith — 63 cells** (band S5 `[51,66]`)
```
  ╭╯───╰╮  
  ▒▓▓▓▓▓▒  
 ░▒▓o o▓▒░ 
 ░▒▓░o░▓▒░ 
 ░▒▓ ▒ ▓▒░ 
 ░▒▓▓▓▓▓▒░ 
  ▒▓▓▓▓▓▒  
 \_/\_/\_/ 
```
**S6 · revenant — 79 cells** (band S6 `[67,88]`)
```
 ░▒▓▓▓▓▓▒░ 
▒▓███████▓▒
▓███o o███▓
▓███░o░███▓
▒▓██ ▒ ██▓▒
▒▓███████▓▒
 ▓███████▓ 
 ░▒▓█▓█▓▒░ 
```

**S4 mood faces — Ghost** (eyes / mouth slot pairs, seated in the S4 frame):

- `resting` — eyes `o o` · mouth `o`
```
  ░▒o o▒░  
  ░▒░o░▒░  
```
- `happy` — eyes `^.^` · mouth `ω`
```
  ░▒^.^▒░  
  ░▒░ω░▒░  
```
- `tired` — eyes `-.-` · mouth `-`
```
  ░▒-.-▒░  
  ░▒░-░▒░  
```
- `wilted` — eyes `,_,` · mouth `_`
```
  ░▒,_,▒░  
  ░▒░_░▒░  
```

Ramp: `[3, 9, 18, 31, 44, 63, 79]` — strictly increasing, every value in-band.

---

### Glitch · Packet Daemon

Packet Daemon — a boxed packet daemon with a living lens face at rest; S5 dense-inset, S6 edge-to-edge.

**S0 · bit — 3 cells** (band S0 `[1,4]`)
```
           
           
           
     ◉     
    ▟▙     
           
           
           
```
**S1 · byte — 10 cells** (band S1 `[5,10]`)
```
           
           
    ▛▀▜    
    ▌◉▐    
    ▙▄▟    
     ▚     
           
           
```
**S2 · packet — 19 cells** (band S2 `[11,20]`)
```
           
   ▛▀▀▀▜   
   ▌◉ ◉▐   
   ▌ ▀ ▐   
   ▙▄▄▄▟   
    ▚ ▞    
           
           
```
**S3 · thread — 32 cells** (band S3 `[21,34]`)
```
           
  ▛▀▀▀▀▀▜  
  ▌ ◉ ◉ ▐  
  ▌  ▀  ▐  
  ▌ ░▄░ ▐  
  ▙▄▄▄▄▄▟  
   ▚▞ ▚▞   
    ▘  ▝   
```
**S4 · glitch — 44 cells** (band S4 `[35,50]`)
```
 ▛▀▀▀▀▀▀▀▜ 
 ▌  ◉ ◉  ▐ 
 ▌   ▀   ▐ 
 ▌  ▒▓▒  ▐ 
 ▌  ░▒░  ▐ 
 ▙▄▄▄▄▄▄▄▟ 
  ▚▞▙ ▟▚▞  
   ▘ ▝ ▘   
```
**S5 · daemon — 62 cells** (band S5 `[51,66]`)
```
 ▛▀▀▀▀▀▀▀▜ 
 ▌▒ ◉ ◉ ▒▐ 
 ▌░▄▄▄▄▄░▐ 
 ▌▒░ █ ░▒▐ 
 ▌▓░▒▒▒░▓▐ 
 ▌░▒ ▒ ▒░▐ 
 ▙▄▄▄▄▄▄▄▟ 
  ▝▟▙ ▟▙▘  
```
**S6 · kernel — 85 cells** (band S6 `[67,88]`)
```
▛▀▀▀▀▀▀▀▀▀▜
▌▓▒ ◉ ◉ ▒▓▐
▌▒░█▀▀▀█░▒▐
▌▓▒░▓▓▓░▒▓▐
▌█▒░▒▒▒░▒█▐
▌▓▒░▒▒▒░▒▓▐
▙▟▙▟▙▟▙▟▙▟▟
▝▟▙▟▙▟▙▟▙▟▘
```

**S4 mood faces — Glitch** (eyes / mouth slot pairs, seated in the S4 frame):

- `resting` — eyes `◉ ◉` · mouth `▀`
```
 ▌  ◉ ◉  ▐ 
 ▌   ▀   ▐ 
```
- `happy` — eyes `^.^` · mouth `ω`
```
 ▌  ^.^  ▐ 
 ▌   ω   ▐ 
```
- `tired` — eyes `-.-` · mouth `-`
```
 ▌  -.-  ▐ 
 ▌   -   ▐ 
```
- `wilted` — eyes `,_,` · mouth `_`
```
 ▌  ,_,  ▐ 
 ▌   _   ▐ 
```

Ramp: `[3, 10, 19, 32, 44, 62, 85]` — strictly increasing, every value in-band.

---

### Crystal · The Caged Lumen

The Caged Lumen — a sealed faceted prism caging a luminous core; diamond-lens eyes fill with age (◇ → ◆ → ◈).

**S0 · grain — 4 cells** (band S0 `[1,4]`)
```
           
           
           
     /\    
     \/    
           
           
           
```
**S1 · shard — 10 cells** (band S1 `[5,10]`)
```
           
           
     /\    
    /◇\    
    \▒/    
     \/    
           
           
```
**S2 · facet — 20 cells** (band S2 `[11,20]`)
```
           
    /\     
   /◇◇\    
  /▒▓▒\    
  \▒▒/     
   \▓/     
    \/     
           
```
**S3 · cluster — 31 cells** (band S3 `[21,34]`)
```
    /\     
   /◆◆\    
  /▒▓▒\    
 /▒▓▓▒\    
 \▒▓▒/     
  \▓▓/     
   \▓/     
    \/     
```
**S4 · crystal — 39 cells** (band S4 `[35,50]`)
```
    /\     
   /◆◆\    
  /▒◆◆▒\   
 /▒▓▿▓▒\   
 \▒▓█▓▒/   
  \▒█▒/    
  \▓█▓/    
   \▼/     
```
**S5 · spire — 53 cells** (band S5 `[51,66]`)
```
  /\/\/\   
 /◈▒◈▒◈\   
/▒▓█▓█▓▒\  
\▒▓███▓▒/  
 \▒▓█▓▒/   
 \▒▓█▓▒/   
  \▓█▓/    
   \▼/     
```
**S6 · lodestar — 73 cells** (band S6 `[67,88]`)
```
/\/\/\/\/\ 
▒██◈██◈██▒ 
▒█▓██▓██▓▒ 
▒█▓█▿█▓█▓▒ 
▒█▓█████▓▒ 
▒▓█▓███▓█▒ 
 \▒▓█▓█▒/  
  \▼▼▼/    
```

**S4 mood faces — Crystal** (eyes / mouth slot pairs, seated in the S4 frame):

- `resting` — eyes `◆◆◆` · mouth `▿`
```
   /◆◆\    
  /▒◆◆▒\   
 /▒▓▿▓▒\   
```
- `happy` — eyes `◆◆◆` · mouth `▴`
```
   /◆◆\    
  /▒◆◆▒\   
 /▒▓▴▓▒\   
```
- `tired` — eyes `◇◇◇` · mouth `▾`
```
   /◇◇\    
  /▒◇◆▒\   
 /▒▓▾▓▒\   
```
- `wilted` — eyes `◌◌◌` · mouth `·`
```
   /◌◌\    
  /▒◌◆▒\   
 /▒▓·▓▒\   
```

Ramp: `[4, 10, 20, 31, 39, 53, 73]` — strictly increasing, every value in-band.

---

### Mech · Bulwark

Bulwark — a closed box-draw chassis with a living optic face; █-dense armor mass at S5/S6, no ░-only bodies.

**S0 · chip — 4 cells** (band S0 `[1,4]`)
```
           
           
           
           
     ▄     
    ▐◉▌    
           
           
```
**S1 · bolt — 10 cells** (band S1 `[5,10]`)
```
           
           
           
    ▗▄▖    
    ▌◉◉▐   
    ▝▀▘    
           
           
```
**S2 · rivet — 20 cells** (band S2 `[11,20]`)
```
           
     ╷     
    ┌───┐  
    │◉ ◉│  
    │ ═ │  
    └┬─┬┘  
     ╨ ╨   
           
```
**S3 · drone — 31 cells** (band S3 `[21,34]`)
```
    ╷ ╷    
   ┌───┐   
   │◉ ◉│   
   │ ═ │   
   ├───┤   
   │▒▓▒│   
   └┬─┬┘   
    ╨ ╨    
```
**S4 · mech — 44 cells** (band S4 `[35,50]`)
```
    ╷╷╷    
   ┌───┐   
   │◉ ◉│   
  ┌┴─═─┴┐  
  ║▓███▓║  
  ║▓▒▒▒▓║  
  ╜└┬─┬┘╙  
   ██ ██   
```
**S5 · archmech — 52 cells** (band S5 `[51,66]`)
```
   ╲╷╷╷╱   
   ┌───┐   
   │◉ ◉│   
  ┌┴─═─┴┐  
 ▟█▌███▐█▙ 
 ▝█▌▒◈▒▐█▘ 
  ║▌▓─▓▐║  
  ▟█▙ ▟█▙  
```
**S6 · titan — 69 cells** (band S6 `[67,88]`)
```
 ██┌───┐██ 
 ██│◉ ◉│██ 
 ██│ ═ │██ 
 ██▙▓◆▓▟██ 
 █████████ 
 ███▒◈▒███ 
 ██▙┬─┬▟██ 
 ▟███████▙ 
```

**S4 mood faces — Mech** (eyes / mouth slot pairs, seated in the S4 frame):

- `resting` — eyes `◉ ◉` · mouth `═`
```
   │◉ ◉│   
  ┌┴─═─┴┐  
```
- `happy` — eyes `◉ ◉` · mouth `◡`
```
   │◉ ◉│   
  ┌┴─◡─┴┐  
```
- `tired` — eyes `▰ ▰` · mouth `╴`
```
   │▰ ▰│   
  ┌┴─╴─┴┐  
```
- `wilted` — eyes `▱ ▱` · mouth `▾`
```
   │▱ ▱│   
  ┌┴─▾─┴┐  
```

Ramp: `[4, 10, 20, 31, 44, 52, 69]` — strictly increasing, every value in-band.

---

## 2. Rust constants (paste into `src/pet/art.rs`)

Each stage is a `[&str; 8]` literal carrying the **exact validated grid** with slot
markers intact. Backslashes are escaped for Rust string literals (`\\`). Width is
counted on the *rendered* grid after slot substitution — the literal template string
is wider on disk because `{eyes}` is 6 source characters occupying a 3-cell slot.

Mood faces are given as `(eyes, mouth)` slot-string pairs to feed
`expression_for` / the mood substitution path; Crystal's mood pair is the facet-color
identity baked into its literal grid, listed for reference.

### Fuzz · Hearthfloof

```rust
const FUZZ_S0: [&str; 8] = [
    "           ",
    "           ",
    "           ",
    "           ",
    "           ",
    "    ▒▒     ",
    "    ▟▙     ",
    "           ",
];
const FUZZ_S1: [&str; 8] = [
    "           ",
    "           ",
    "    ▟▙     ",
    "    ▒▒     ",
    "    ▓▓     ",
    "    ▒▒     ",
    "    ▘▝     ",
    "           ",
];
const FUZZ_S2: [&str; 8] = [
    "           ",
    "           ",
    "    ▟▙     ",
    "   ▓▒▒▓    ",
    "  ▒{eyes}▒    ",
    "   ▒{mouth}▒     ",
    "   ▓▒▒▓    ",
    "    ▘▝     ",
];
const FUZZ_S3: [&str; 8] = [
    "           ",
    "   ▟▙▟▙    ",
    "  ▓▒▒▒▒▓   ",
    "  ▓▒{eyes}▒▓  ",
    "  ▓▒ {mouth} ▒▓  ",
    "  ▓▒◌▒▒▓   ",
    "   ▙▒▒▟    ",
    "   ▘  ▝    ",
];
const FUZZ_S4: [&str; 8] = [
    "   ▟▙ ▟▙   ",
    "  ▓▒▒▒▒▒▓  ",
    "  ▓▒{eyes}▒▒▓ ",
    "  ▓▒ {mouth} ▒▒▓ ",
    "  ▓▒▒◆▒▒▒▓ ",
    "  ▓▒▒▒▒▒▒▓ ",
    "  ▙▒▒▒▒▒▒▟ ",
    "   ▘    ▝  ",
];
const FUZZ_S5: [&str; 8] = [
    "  ▟█▙ ▟█▙  ",
    "  ▓▓▒▒▒▒▓▓ ",
    "  ▓▒{eyes}▒▒▓ ",
    "  ▓▒ {mouth} ▒▒▓ ",
    "  ▓▒▒◆◆▒▒▓ ",
    "  ▓█▒▒▒▒█▓ ",
    "  ▓█▒▒▒▒█▓ ",
    "  ▙█▒▘▝▒█▟ ",
];
const FUZZ_S6: [&str; 8] = [
    " ▟██▙▟██▙  ",
    " ▓██▒▒▒██▓ ",
    " ▓█▒{eyes}▒█▓ ",
    " ▓█▒ {mouth} ▒█▓ ",
    " ▓█▒◈◈◈▒█▓ ",
    " ▓██▒▒▒██▓ ",
    " ▓██▒▒▒██▓ ",
    " ▙██▒▘▝▒██▟",
];
```

Mood faces (S4) — `(eyes, mouth)`:

```rust
// resting  FUZZ: eyes "◉ ◉", mouth "w"
// happy    FUZZ: eyes "^ ^", mouth "ω"
// tired    FUZZ: eyes "˘ ˘", mouth "⌣"
// wilted   FUZZ: eyes "ˇ ˇ", mouth "⌢"
```

---

### Blob · Deep-Light Medusa

```rust
const BLOB_S0: [&str; 8] = [
    "           ",
    "           ",
    "    ▄▄     ",
    "    ▒      ",
    "           ",
    "           ",
    "           ",
    "           ",
];
const BLOB_S1: [&str; 8] = [
    "           ",
    "    ▄▄     ",
    "   ▟▒▙     ",
    "    ◉      ",
    "    |      ",
    "           ",
    "           ",
    "           ",
];
const BLOB_S2: [&str; 8] = [
    "           ",
    "    ▄▄▄    ",
    "   ▟▒▒▙    ",
    "   ◉ ◉     ",
    "   ▒~▒     ",
    "   |╎|     ",
    "           ",
    "           ",
];
const BLOB_S3: [&str; 8] = [
    "    ▄▄▄    ",
    "   ▟▒▒▙    ",
    "  (▒◉ ◉▒)  ",
    "  (▒▒~▒▒)  ",
    "   ▒▓▒     ",
    "   ▒▓▒     ",
    "   |╎|┊    ",
    "   ' ' '   ",
];
const BLOB_S4: [&str; 8] = [
    "    ▄▄▄    ",
    "   ▟▒▒▙    ",
    "  (▒◉ ◉▒)  ",
    "  (▒▒~▒▒)  ",
    "  (░▓◆▓░)  ",
    "   ▒▓▓▒    ",
    "   |╎|┊    ",
    "   ' ' '   ",
];
const BLOB_S5: [&str; 8] = [
    "   ▄▄▄▄    ",
    "  ▟▒▒▒▒▙   ",
    " (▒▒◉ ◉▒▒) ",
    " (▒▒▒~▒▒▒) ",
    " (░▓◆◉◆▓░) ",
    " (░▒▓▓▓▒░) ",
    "  |┊|╎|┊   ",
    "  ' ' ' '  ",
];
const BLOB_S6: [&str; 8] = [
    " ▄▄▄▄▄▄▄▄▄ ",
    "▟▒▒▒▒▒▒▒▒▒▙",
    "▐▒▓███▓▒▒▒▌",
    "▐▒◉ ◉▒▓▒░▌ ",
    "(▒▒~▒▒▓▒░) ",
    "(◆▓◉◆◉▓◆▒) ",
    "▝▒░▒░▒░▒░▘ ",
    " |┊|╎|┊|╎  ",
];
```

Mood faces (S4) — `(eyes, mouth)`:

```rust
// resting  BLOB: eyes "◉ ◉", mouth "~"
// happy    BLOB: eyes "^ ^", mouth "ω"
// tired    BLOB: eyes "- -", mouth "-"
// wilted   BLOB: eyes ",_,", mouth "_"
```

---

### Ghost · The Pall

```rust
const GHOST_S0: [&str; 8] = [
    "           ",
    "           ",
    "           ",
    "           ",
    "           ",
    "           ",
    "    ▒▒▒    ",
    "           ",
];
const GHOST_S1: [&str; 8] = [
    "           ",
    "           ",
    "           ",
    "           ",
    "    ▄▄▄    ",
    "    ▒▒▒    ",
    "    \\_/    ",
    "           ",
];
const GHOST_S2: [&str; 8] = [
    "           ",
    "           ",
    "    ▄▄▄    ",
    "   ▒{eyes}▒   ",
    "   ▒░{mouth}░▒   ",
    "    ▒▒▒    ",
    "    \\_/    ",
    "           ",
];
const GHOST_S3: [&str; 8] = [
    "    ╭─╮    ",
    "   ╭╯ ╰╮   ",
    "   ▒▒▒▒▒   ",
    "   ▒{eyes}▒   ",
    "   ▒░{mouth}░▒   ",
    "   ▒▒▒▒▒   ",
    "   \\_/\\_   ",
    "           ",
];
const GHOST_S4: [&str; 8] = [
    "   ╭───╮   ",
    "  ╭╯   ╰╮  ",
    "  ░▒▒▒▒▒░  ",
    "  ░▒{eyes}▒░  ",
    "  ░▒░{mouth}░▒░  ",
    "  ░▒{pattern}▒░  ",
    "   ▒▒▒▒▒   ",
    "   \\_/\\_   ",
];
const GHOST_S5: [&str; 8] = [
    "  ╭╯───╰╮  ",
    "  ▒▓▓▓▓▓▒  ",
    " ░▒▓{eyes}▓▒░ ",
    " ░▒▓░{mouth}░▓▒░ ",
    " ░▒▓{pattern}▓▒░ ",
    " ░▒▓▓▓▓▓▒░ ",
    "  ▒▓▓▓▓▓▒  ",
    " \\_/\\_/\\_/ ",
];
const GHOST_S6: [&str; 8] = [
    " ░▒▓▓▓▓▓▒░ ",
    "▒▓███████▓▒",
    "▓███{eyes}███▓",
    "▓███░{mouth}░███▓",
    "▒▓██{pattern}██▓▒",
    "▒▓███████▓▒",
    " ▓███████▓ ",
    " ░▒▓█▓█▓▒░ ",
];
```

Mood faces (S4) — `(eyes, mouth)`:

```rust
// resting  GHOST: eyes "o o", mouth "o"
// happy    GHOST: eyes "^.^", mouth "ω"
// tired    GHOST: eyes "-.-", mouth "-"
// wilted   GHOST: eyes ",_,", mouth "_"
```

---

### Glitch · Packet Daemon

```rust
const GLITCH_S0: [&str; 8] = [
    "           ",
    "           ",
    "           ",
    "     ◉     ",
    "    ▟▙     ",
    "           ",
    "           ",
    "           ",
];
const GLITCH_S1: [&str; 8] = [
    "           ",
    "           ",
    "    ▛▀▜    ",
    "    ▌◉▐    ",
    "    ▙▄▟    ",
    "     ▚     ",
    "           ",
    "           ",
];
const GLITCH_S2: [&str; 8] = [
    "           ",
    "   ▛▀▀▀▜   ",
    "   ▌{eyes}▐   ",
    "   ▌ ▀ ▐   ",
    "   ▙▄▄▄▟   ",
    "    ▚ ▞    ",
    "           ",
    "           ",
];
const GLITCH_S3: [&str; 8] = [
    "           ",
    "  ▛▀▀▀▀▀▜  ",
    "  ▌ {eyes} ▐  ",
    "  ▌  ▀  ▐  ",
    "  ▌ ░▄░ ▐  ",
    "  ▙▄▄▄▄▄▟  ",
    "   ▚▞ ▚▞   ",
    "    ▘  ▝   ",
];
const GLITCH_S4: [&str; 8] = [
    " ▛▀▀▀▀▀▀▀▜ ",
    " ▌  {eyes}  ▐ ",
    " ▌   ▀   ▐ ",
    " ▌  ▒▓▒  ▐ ",
    " ▌  ░▒░  ▐ ",
    " ▙▄▄▄▄▄▄▄▟ ",
    "  ▚▞▙ ▟▚▞  ",
    "   ▘ ▝ ▘   ",
];
const GLITCH_S5: [&str; 8] = [
    " ▛▀▀▀▀▀▀▀▜ ",
    " ▌▒ {eyes} ▒▐ ",
    " ▌░▄▄▄▄▄░▐ ",
    " ▌▒░ █ ░▒▐ ",
    " ▌▓░▒▒▒░▓▐ ",
    " ▌░▒ ▒ ▒░▐ ",
    " ▙▄▄▄▄▄▄▄▟ ",
    "  ▝▟▙ ▟▙▘  ",
];
const GLITCH_S6: [&str; 8] = [
    "▛▀▀▀▀▀▀▀▀▀▜",
    "▌▓▒ {eyes} ▒▓▐",
    "▌▒░█▀▀▀█░▒▐",
    "▌▓▒░▓▓▓░▒▓▐",
    "▌█▒░▒▒▒░▒█▐",
    "▌▓▒░▒▒▒░▒▓▐",
    "▙▟▙▟▙▟▙▟▙▟▟",
    "▝▟▙▟▙▟▙▟▙▟▘",
];
```

Mood faces (S4) — `(eyes, mouth)`:

```rust
// resting  GLITCH: eyes "◉ ◉", mouth "▀"
// happy    GLITCH: eyes "^.^", mouth "ω"
// tired    GLITCH: eyes "-.-", mouth "-"
// wilted   GLITCH: eyes ",_,", mouth "_"
```

---

### Crystal · The Caged Lumen

```rust
const CRYSTAL_S0: [&str; 8] = [
    "           ",
    "           ",
    "           ",
    "     /\\    ",
    "     \\/    ",
    "           ",
    "           ",
    "           ",
];
const CRYSTAL_S1: [&str; 8] = [
    "           ",
    "           ",
    "     /\\    ",
    "    /◇\\    ",
    "    \\▒/    ",
    "     \\/    ",
    "           ",
    "           ",
];
const CRYSTAL_S2: [&str; 8] = [
    "           ",
    "    /\\     ",
    "   /◇◇\\    ",
    "  /▒▓▒\\    ",
    "  \\▒▒/     ",
    "   \\▓/     ",
    "    \\/     ",
    "           ",
];
const CRYSTAL_S3: [&str; 8] = [
    "    /\\     ",
    "   /◆◆\\    ",
    "  /▒▓▒\\    ",
    " /▒▓▓▒\\    ",
    " \\▒▓▒/     ",
    "  \\▓▓/     ",
    "   \\▓/     ",
    "    \\/     ",
];
const CRYSTAL_S4: [&str; 8] = [
    "    /\\     ",
    "   /◆◆\\    ",
    "  /▒◆◆▒\\   ",
    " /▒▓▿▓▒\\   ",
    " \\▒▓█▓▒/   ",
    "  \\▒█▒/    ",
    "  \\▓█▓/    ",
    "   \\▼/     ",
];
const CRYSTAL_S5: [&str; 8] = [
    "  /\\/\\/\\   ",
    " /◈▒◈▒◈\\   ",
    "/▒▓█▓█▓▒\\  ",
    "\\▒▓███▓▒/  ",
    " \\▒▓█▓▒/   ",
    " \\▒▓█▓▒/   ",
    "  \\▓█▓/    ",
    "   \\▼/     ",
];
const CRYSTAL_S6: [&str; 8] = [
    "/\\/\\/\\/\\/\\ ",
    "▒██◈██◈██▒ ",
    "▒█▓██▓██▓▒ ",
    "▒█▓█▿█▓█▓▒ ",
    "▒█▓█████▓▒ ",
    "▒▓█▓███▓█▒ ",
    " \\▒▓█▓█▒/  ",
    "  \\▼▼▼/    ",
];
```

Mood faces (S4) — `(eyes, mouth)`:

```rust
// resting  CRYSTAL: eyes "◆◆◆", mouth "▿"
// happy    CRYSTAL: eyes "◆◆◆", mouth "▴"
// tired    CRYSTAL: eyes "◇◇◇", mouth "▾"
// wilted   CRYSTAL: eyes "◌◌◌", mouth "·"
```

---

### Mech · Bulwark

```rust
const MECH_S0: [&str; 8] = [
    "           ",
    "           ",
    "           ",
    "           ",
    "     ▄     ",
    "    ▐◉▌    ",
    "           ",
    "           ",
];
const MECH_S1: [&str; 8] = [
    "           ",
    "           ",
    "           ",
    "    ▗▄▖    ",
    "    ▌◉◉▐   ",
    "    ▝▀▘    ",
    "           ",
    "           ",
];
const MECH_S2: [&str; 8] = [
    "           ",
    "     ╷     ",
    "    ┌───┐  ",
    "    │{eyes}│  ",
    "    │ {mouth} │  ",
    "    └┬─┬┘  ",
    "     ╨ ╨   ",
    "           ",
];
const MECH_S3: [&str; 8] = [
    "    ╷ ╷    ",
    "   ┌───┐   ",
    "   │{eyes}│   ",
    "   │ {mouth} │   ",
    "   ├───┤   ",
    "   │▒▓▒│   ",
    "   └┬─┬┘   ",
    "    ╨ ╨    ",
];
const MECH_S4: [&str; 8] = [
    "    ╷╷╷    ",
    "   ┌───┐   ",
    "   │{eyes}│   ",
    "  ┌┴─{mouth}─┴┐  ",
    "  ║▓███▓║  ",
    "  ║▓▒▒▒▓║  ",
    "  ╜└┬─┬┘╙  ",
    "   ██ ██   ",
];
const MECH_S5: [&str; 8] = [
    "   ╲╷╷╷╱   ",
    "   ┌───┐   ",
    "   │◉ ◉│   ",
    "  ┌┴─═─┴┐  ",
    " ▟█▌███▐█▙ ",
    " ▝█▌▒◈▒▐█▘ ",
    "  ║▌▓─▓▐║  ",
    "  ▟█▙ ▟█▙  ",
];
const MECH_S6: [&str; 8] = [
    " ██┌───┐██ ",
    " ██│◉ ◉│██ ",
    " ██│ ═ │██ ",
    " ██▙▓◆▓▟██ ",
    " █████████ ",
    " ███▒◈▒███ ",
    " ██▙┬─┬▟██ ",
    " ▟███████▙ ",
];
```

Mood faces (S4) — `(eyes, mouth)`:

```rust
// resting  MECH: eyes "◉ ◉", mouth "═"
// happy    MECH: eyes "◉ ◉", mouth "◡"
// tired    MECH: eyes "▰ ▰", mouth "╴"
// wilted   MECH: eyes "▱ ▱", mouth "▾"
```

---

## 3. Per-species cell-count table

Rendered non-space cell count per stage, with the stage band in the header. Every
cell is in-band; every row is strictly increasing left to right.

| Species | S0 `[1,4]` | S1 `[5,10]` | S2 `[11,20]` | S3 `[21,34]` | S4 `[35,50]` | S5 `[51,66]` | S6 `[67,88]` | Strictly ↑ |
|---|---|---|---|---|---|---|---|---|
| **Fuzz** | 4 | 10 | 19 | 33 | 50 | 59 | 69 | yes |
| **Blob** | 3 | 7 | 15 | 33 | 38 | 55 | 78 | yes |
| **Ghost** | 3 | 9 | 18 | 31 | 44 | 63 | 79 | yes |
| **Glitch** | 3 | 10 | 19 | 32 | 44 | 62 | 85 | yes |
| **Crystal** | 4 | 10 | 20 | 31 | 39 | 53 | 73 | yes |
| **Mech** | 4 | 10 | 20 | 31 | 44 | 52 | 69 | yes |

All six ramps are strictly increasing, every entry lands inside its disjoint stage
band, and no two stages share a band — so the count column for any stage is a clean
proof that mass grows monotonically across the arc.

### Band-membership proof (rendered count ∈ stage band, all 42)

```
Fuzz     S0  cells=  4  band[ 1, 4]  OK
Fuzz     S1  cells= 10  band[ 5,10]  OK
Fuzz     S2  cells= 19  band[11,20]  OK
Fuzz     S3  cells= 33  band[21,34]  OK
Fuzz     S4  cells= 50  band[35,50]  OK
Fuzz     S5  cells= 59  band[51,66]  OK
Fuzz     S6  cells= 69  band[67,88]  OK
Blob     S0  cells=  3  band[ 1, 4]  OK
Blob     S1  cells=  7  band[ 5,10]  OK
Blob     S2  cells= 15  band[11,20]  OK
Blob     S3  cells= 33  band[21,34]  OK
Blob     S4  cells= 38  band[35,50]  OK
Blob     S5  cells= 55  band[51,66]  OK
Blob     S6  cells= 78  band[67,88]  OK
Ghost    S0  cells=  3  band[ 1, 4]  OK
Ghost    S1  cells=  9  band[ 5,10]  OK
Ghost    S2  cells= 18  band[11,20]  OK
Ghost    S3  cells= 31  band[21,34]  OK
Ghost    S4  cells= 44  band[35,50]  OK
Ghost    S5  cells= 63  band[51,66]  OK
Ghost    S6  cells= 79  band[67,88]  OK
Glitch   S0  cells=  3  band[ 1, 4]  OK
Glitch   S1  cells= 10  band[ 5,10]  OK
Glitch   S2  cells= 19  band[11,20]  OK
Glitch   S3  cells= 32  band[21,34]  OK
Glitch   S4  cells= 44  band[35,50]  OK
Glitch   S5  cells= 62  band[51,66]  OK
Glitch   S6  cells= 85  band[67,88]  OK
Crystal  S0  cells=  4  band[ 1, 4]  OK
Crystal  S1  cells= 10  band[ 5,10]  OK
Crystal  S2  cells= 20  band[11,20]  OK
Crystal  S3  cells= 31  band[21,34]  OK
Crystal  S4  cells= 39  band[35,50]  OK
Crystal  S5  cells= 53  band[51,66]  OK
Crystal  S6  cells= 73  band[67,88]  OK
Mech     S0  cells=  4  band[ 1, 4]  OK
Mech     S1  cells= 10  band[ 5,10]  OK
Mech     S2  cells= 20  band[11,20]  OK
Mech     S3  cells= 31  band[21,34]  OK
Mech     S4  cells= 44  band[35,50]  OK
Mech     S5  cells= 52  band[51,66]  OK
Mech     S6  cells= 69  band[67,88]  OK
```

All 42 base templates: 11×8 geometry confirmed, band membership confirmed, strict
monotonicity confirmed. The art payload is validated and ready to consume.
