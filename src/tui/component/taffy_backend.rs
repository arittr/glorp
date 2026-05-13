use ratatui::layout::Rect;
use taffy::prelude::{
    AvailableSpace, Dimension, FlexDirection, Size as TaffySize, Style, TaffyTree,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnSpec {
    Fixed(u16),
    Fill(u16),
}

impl ColumnSpec {
    pub const fn fixed(width: u16) -> Self {
        Self::Fixed(width)
    }

    pub const fn fill(weight: u16) -> Self {
        Self::Fill(weight)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowSpec {
    Fixed(u16),
    Fill(u16),
}

impl RowSpec {
    pub const fn fixed(height: u16) -> Self {
        Self::Fixed(height)
    }

    pub const fn fill(weight: u16) -> Self {
        Self::Fill(weight)
    }
}

pub fn allocate_columns(area: Rect, specs: &[ColumnSpec]) -> Vec<Rect> {
    let specs = specs
        .iter()
        .map(|spec| match *spec {
            ColumnSpec::Fixed(width) => AxisSpec::Fixed(width),
            ColumnSpec::Fill(weight) => AxisSpec::Fill(weight),
        })
        .collect::<Vec<_>>();

    allocate_axis(area.x, area.width, area.height, Axis::Horizontal, &specs)
        .into_iter()
        .map(|(x, width)| Rect::new(x, area.y, width, area.height))
        .collect()
}

pub fn allocate_stack(area: Rect, specs: &[RowSpec]) -> Vec<Rect> {
    let specs = specs
        .iter()
        .map(|spec| match *spec {
            RowSpec::Fixed(height) => AxisSpec::Fixed(height),
            RowSpec::Fill(weight) => AxisSpec::Fill(weight),
        })
        .collect::<Vec<_>>();

    allocate_axis(area.y, area.height, area.width, Axis::Vertical, &specs)
        .into_iter()
        .map(|(y, height)| Rect::new(area.x, y, area.width, height))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxisSpec {
    Fixed(u16),
    Fill(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

fn allocate_axis(
    start: u16,
    span: u16,
    cross_span: u16,
    axis: Axis,
    specs: &[AxisSpec],
) -> Vec<(u16, u16)> {
    let taffy_sizes = taffy_flex_sizes(span, cross_span, axis, specs);
    let sizes = integer_sizes(span, specs, &taffy_sizes);
    let mut offset = 0u16;

    sizes
        .into_iter()
        .map(|size| {
            let position = start.saturating_add(offset);
            offset = offset.saturating_add(size);
            (position, size)
        })
        .collect()
}

fn taffy_flex_sizes(span: u16, cross_span: u16, axis: Axis, specs: &[AxisSpec]) -> Vec<f32> {
    let mut taffy: TaffyTree<()> = TaffyTree::new();
    taffy.disable_rounding();

    let children = specs
        .iter()
        .map(|spec| {
            taffy
                .new_leaf(child_style(*spec, axis))
                .expect("taffy child node should be constructible")
        })
        .collect::<Vec<_>>();

    let root = taffy
        .new_with_children(root_style(span, cross_span, axis), &children)
        .expect("taffy root node should be constructible");
    taffy
        .compute_layout(root, available_space(span, cross_span, axis))
        .expect("taffy flex layout should be computable");

    children
        .iter()
        .map(|child| {
            let layout = taffy
                .layout(*child)
                .expect("taffy child layout should be available");
            match axis {
                Axis::Horizontal => layout.size.width,
                Axis::Vertical => layout.size.height,
            }
        })
        .collect()
}

fn root_style(span: u16, cross_span: u16, axis: Axis) -> Style {
    Style {
        flex_direction: match axis {
            Axis::Horizontal => FlexDirection::Row,
            Axis::Vertical => FlexDirection::Column,
        },
        size: taffy_size(span, cross_span, axis),
        ..Default::default()
    }
}

fn child_style(spec: AxisSpec, axis: Axis) -> Style {
    match spec {
        AxisSpec::Fixed(size) => Style {
            flex_shrink: 0.0,
            size: taffy_size(size, 0, axis),
            ..Default::default()
        },
        AxisSpec::Fill(weight) => Style {
            flex_basis: Dimension::length(0.0),
            flex_grow: f32::from(weight),
            size: taffy_size(0, 0, axis),
            ..Default::default()
        },
    }
}

fn taffy_size(main: u16, cross: u16, axis: Axis) -> TaffySize<Dimension> {
    match axis {
        Axis::Horizontal => TaffySize {
            width: Dimension::length(f32::from(main)),
            height: Dimension::length(f32::from(cross)),
        },
        Axis::Vertical => TaffySize {
            width: Dimension::length(f32::from(cross)),
            height: Dimension::length(f32::from(main)),
        },
    }
}

fn available_space(main: u16, cross: u16, axis: Axis) -> TaffySize<AvailableSpace> {
    match axis {
        Axis::Horizontal => TaffySize {
            width: AvailableSpace::Definite(f32::from(main)),
            height: AvailableSpace::Definite(f32::from(cross)),
        },
        Axis::Vertical => TaffySize {
            width: AvailableSpace::Definite(f32::from(cross)),
            height: AvailableSpace::Definite(f32::from(main)),
        },
    }
}

fn integer_sizes(span: u16, specs: &[AxisSpec], taffy_sizes: &[f32]) -> Vec<u16> {
    let parent = u32::from(span);
    let fixed_total = specs
        .iter()
        .map(|spec| match *spec {
            AxisSpec::Fixed(size) => u32::from(size),
            AxisSpec::Fill(_) => 0,
        })
        .sum::<u32>();

    if fixed_total >= parent {
        let mut remaining = parent;
        return specs
            .iter()
            .map(|spec| match *spec {
                AxisSpec::Fixed(size) => {
                    let size = u32::from(size).min(remaining);
                    remaining = remaining.saturating_sub(size);
                    size as u16
                }
                AxisSpec::Fill(_) => 0,
            })
            .collect();
    }

    let mut sizes = specs
        .iter()
        .map(|spec| match *spec {
            AxisSpec::Fixed(size) => u32::from(size),
            AxisSpec::Fill(_) => 0,
        })
        .collect::<Vec<_>>();
    let fill_indices = specs
        .iter()
        .enumerate()
        .filter_map(|(index, spec)| match *spec {
            AxisSpec::Fill(weight) if weight > 0 => Some(index),
            AxisSpec::Fixed(_) | AxisSpec::Fill(_) => None,
        })
        .collect::<Vec<_>>();
    let Some((&last_fill, earlier_fills)) = fill_indices.split_last() else {
        return sizes.into_iter().map(|size| size as u16).collect();
    };

    let fill_space = parent - fixed_total;
    let mut assigned_fill = 0u32;

    for index in earlier_fills {
        let taffy_size = taffy_sizes.get(*index).copied().unwrap_or_default();
        let size = if taffy_size.is_finite() && taffy_size > 0.0 {
            taffy_size.floor() as u32
        } else {
            0
        };
        let size = size.min(fill_space.saturating_sub(assigned_fill));
        sizes[*index] = size;
        assigned_fill += size;
    }

    sizes[last_fill] = fill_space.saturating_sub(assigned_fill);
    sizes.into_iter().map(|size| size as u16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn taffy_backend_conserves_sibling_widths_with_odd_remainders() {
        let allocated = allocate_columns(
            Rect::new(0, 0, 101, 10),
            &[
                ColumnSpec::fixed(40),
                ColumnSpec::fixed(4),
                ColumnSpec::fill(1),
            ],
        );
        assert_eq!(allocated[0], Rect::new(0, 0, 40, 10));
        assert_eq!(allocated[1], Rect::new(40, 0, 4, 10));
        assert_eq!(allocated[2], Rect::new(44, 0, 57, 10));
        assert_eq!(allocated.iter().map(|r| r.width).sum::<u16>(), 101);
    }

    #[test]
    fn taffy_backend_clips_children_to_parent() {
        let allocated = allocate_stack(
            Rect::new(5, 5, 20, 6),
            &[RowSpec::fixed(10), RowSpec::fill(1)],
        );
        assert!(allocated.iter().all(|rect| rect.x >= 5));
        assert!(allocated.iter().all(|rect| rect.y >= 5));
        assert!(allocated.iter().all(|rect| rect.y + rect.height <= 11));
    }
}
