//! The treasure chest's ambient bubble: once per cycle the chest puffs a single
//! bubble that rises from just above it and either escapes past the top of the
//! frame or pops into a brief `·`. Lives in the ambient layer (individual cells,
//! dropped when they leave the area) rather than the all-or-nothing prop sprite,
//! so "float past the top" comes for free.

use crate::pet::animator::chest_bubble_cycle;
use crate::pet::palette::Rgb;
use crate::presentation::DrawCell;
use ratatui::layout::{Position, Rect};

/// The bubble climbs this many rows per second.
const RISE_ROWS_PER_SEC: f64 = 2.5;
/// The bubble is in flight only during the first part of each cycle; after this
/// the chest is closed and quiet until the next cycle.
const RISE_SECS: f64 = 8.0;
/// A popping bubble bursts somewhere in this band of rows climbed.
const POP_MIN_ROWS: f64 = 4.0;
const POP_ROW_SPREAD: f64 = 4.0;
/// The `·` burst lingers this many rows of rise-time before vanishing.
const POP_LINGER_ROWS: f64 = 2.5;
/// One bubble in roughly this many carries a treasure sparkle instead of `o`.
const TREASURE_EVERY: u64 = 5;

fn cycle_hash(cycle: u64, seed: u64) -> u64 {
    cycle
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(seed.wrapping_mul(0x85eb_ca6b).wrapping_add(0x27d4_eb2f))
        .wrapping_mul(0xc2b2_ae35_27d4_eb2f)
}

/// Draw cells for the treasure chest's rising bubble at `now`. The chest puffs
/// one bubble per cycle from just above `(chest_top_row, chest_col)`; deterministic
/// per cycle, it either rises out the top of `area` (escapes — yields nothing once
/// it clears the frame) or pops into a `·`. Occasionally the bubble is a `✦`
/// treasure. Empty for most of the cycle.
pub fn chest_bubble_cells(
    chest_top_row: u16,
    chest_col: u16,
    area: Rect,
    now: time::OffsetDateTime,
    seed: u64,
    color: Rgb,
) -> Vec<DrawCell> {
    let (cycle, t) = chest_bubble_cycle(now);
    if t >= RISE_SECS {
        return Vec::new(); // chest closed, no bubble in flight
    }
    let h = cycle_hash(cycle, seed);
    let is_pop = h & 1 == 1;
    let is_treasure = h.is_multiple_of(TREASURE_EVERY);
    let pop_rows = POP_MIN_ROWS + POP_ROW_SPREAD * (((h >> 8) % 16) as f64 / 15.0);

    let risen = t * RISE_ROWS_PER_SEC;
    // The bubble emerges one row above the chest's top edge.
    let origin = f64::from(chest_top_row) - 1.0;

    let (row_f, glyph) = if is_pop && risen >= pop_rows {
        if risen < pop_rows + POP_LINGER_ROWS {
            (origin - pop_rows, '·') // burst, held at the pop height
        } else {
            return Vec::new(); // popped and gone
        }
    } else {
        (origin - risen, if is_treasure { '✦' } else { 'o' })
    };

    if row_f < 0.0 {
        return Vec::new(); // escaped above the frame (guards the cast)
    }
    let row = row_f.round() as u16;
    if !area.contains(Position::new(chest_col, row)) {
        return Vec::new(); // outside the porthole
    }
    vec![DrawCell {
        row,
        col: chest_col,
        glyph: Some(glyph.to_string()),
        fg: Some(color),
        bg: None,
        bold: false,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::animator::CHEST_BUBBLE_CYCLE_SECS;

    const GOLD: Rgb = Rgb { r: 0xd8, g: 0xa4, b: 0x4c };

    fn at(cycle: u64, t: f64) -> time::OffsetDateTime {
        let secs = cycle as f64 * CHEST_BUBBLE_CYCLE_SECS + t;
        time::OffsetDateTime::from_unix_timestamp(secs as i64).unwrap()
    }

    fn first_cycle(seed: u64, pred: impl Fn(u64) -> bool) -> u64 {
        (0..500)
            .find(|&c| pred(cycle_hash(c, seed)))
            .expect("a matching cycle")
    }

    #[test]
    fn bubble_rises_then_escapes_past_the_top() {
        let area = Rect::new(0, 0, 30, 18);
        let (chest_top, col, seed) = (15u16, 15u16, 7u64);
        // an escape cycle (not a pop)
        let cycle = first_cycle(seed, |h| h & 1 == 0);

        let early = chest_bubble_cells(chest_top, col, area, at(cycle, 0.0), seed, GOLD);
        assert_eq!(early.len(), 1, "a bubble appears at the start of the cycle");
        let r0 = early[0].row;
        assert!(r0 <= chest_top, "it starts at/above the chest top");

        let later = chest_bubble_cells(chest_top, col, area, at(cycle, 2.0), seed, GOLD);
        assert_eq!(later.len(), 1);
        assert!(later[0].row < r0, "it rises (row decreases) over time");

        let gone = chest_bubble_cells(chest_top, col, area, at(cycle, 7.0), seed, GOLD);
        assert!(gone.is_empty(), "it escapes past the top of the frame");

        let quiet = chest_bubble_cells(chest_top, col, area, at(cycle, 12.0), seed, GOLD);
        assert!(quiet.is_empty(), "the chest is quiet for most of the cycle");
    }

    #[test]
    fn pop_cycles_produce_a_burst() {
        let area = Rect::new(0, 0, 30, 22);
        let (chest_top, col, seed) = (20u16, 15u16, 7u64);
        let cycle = first_cycle(seed, |h| h & 1 == 1); // a pop cycle

        let burst = (1..8).any(|t| {
            chest_bubble_cells(chest_top, col, area, at(cycle, f64::from(t)), seed, GOLD)
                .iter()
                .any(|c| c.glyph.as_deref() == Some("·"))
        });
        assert!(burst, "a pop cycle bursts into a ·");
    }

    #[test]
    fn treasure_cycles_carry_a_sparkle() {
        let area = Rect::new(0, 0, 30, 22);
        let (chest_top, col, seed) = (20u16, 15u16, 7u64);
        // treasure AND escape (so we see the rising glyph, not a pop)
        let cycle = first_cycle(seed, |h| h.is_multiple_of(TREASURE_EVERY) && h & 1 == 0);
        let cells = chest_bubble_cells(chest_top, col, area, at(cycle, 1.0), seed, GOLD);
        assert_eq!(cells.len(), 1);
        assert_eq!(
            cells[0].glyph.as_deref(),
            Some("✦"),
            "treasure bubble glints"
        );
    }
}
