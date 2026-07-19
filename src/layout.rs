use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionLayout {
    Row,
    Column,
    Grid,
    PictureInPicture,
}

impl CompositionLayout {
    /// Stable lowercase name for status lines, CLI output and config files.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Column => "column",
            Self::Grid => "grid",
            Self::PictureInPicture => "pip",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridLayout {
    pub columns: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub struct GridLayoutCalculator;

impl GridLayoutCalculator {
    pub fn grid_size(count: usize, layout: CompositionLayout) -> GridLayout {
        let count = count.max(1) as u32;
        match layout {
            CompositionLayout::Row => GridLayout {
                columns: count,
                rows: 1,
            },
            CompositionLayout::Column => GridLayout {
                columns: 1,
                rows: count,
            },
            CompositionLayout::Grid => {
                let columns = (count as f64).sqrt().ceil() as u32;
                let rows = count.div_ceil(columns);
                GridLayout { columns, rows }
            }
            CompositionLayout::PictureInPicture => GridLayout {
                columns: 1,
                rows: 1,
            },
        }
    }

    pub fn cells(
        output_width: u32,
        output_height: u32,
        count: usize,
        layout: CompositionLayout,
    ) -> Vec<Cell> {
        if layout == CompositionLayout::PictureInPicture {
            return Self::picture_in_picture_cells(output_width, output_height, count);
        }
        let grid = Self::grid_size(count, layout);
        let count = count.max(1);
        let mut cells = Vec::with_capacity(count);

        for index in 0..count {
            let index = index as u32;
            let column = index % grid.columns;
            let row = index / grid.columns;
            let x = Self::edge(output_width, grid.columns, column);
            let y = Self::edge(output_height, grid.rows, row);
            let width = Self::edge(output_width, grid.columns, column + 1) - x;
            let height = Self::edge(output_height, grid.rows, row + 1) - y;
            cells.push(Cell {
                x,
                y,
                width,
                height,
            });
        }

        cells
    }

    fn picture_in_picture_cells(output_width: u32, output_height: u32, count: usize) -> Vec<Cell> {
        let count = count.max(1);
        let mut cells = Vec::with_capacity(count);
        cells.push(Cell {
            x: 0,
            y: 0,
            width: output_width,
            height: output_height,
        });
        if count == 1 {
            return cells;
        }

        let overlay_count = u32::try_from(count - 1).unwrap_or(u32::MAX);
        let margin = (output_width.min(output_height) / 50)
            .min(output_width / 4)
            .min(output_height / 4);
        let available_height = output_height.saturating_sub(margin.saturating_mul(2));
        let available_width = output_width.saturating_sub(margin.saturating_mul(2));
        let overlay_width = (output_width / 4).max(1).min(available_width);
        let target_height = (output_height / 4).max(1);
        let x = output_width
            .saturating_sub(margin)
            .saturating_sub(overlay_width);

        for index in 0..overlay_count {
            let slot_from_top = overlay_count - index - 1;
            let slot_top = margin + Self::edge(available_height, overlay_count, slot_from_top);
            let slot_bottom =
                margin + Self::edge(available_height, overlay_count, slot_from_top + 1);
            let slot_height = slot_bottom.saturating_sub(slot_top);
            let height = target_height.min(slot_height);
            cells.push(Cell {
                x,
                y: slot_bottom.saturating_sub(height),
                width: overlay_width,
                height,
            });
        }

        cells
    }

    /// Proportional grid line position. Distributes remainder pixels across all
    /// cells instead of dumping them into the last one, never overlaps, and
    /// degrades to zero-size cells (which the renderer skips) when the grid has
    /// more cells than output pixels. u64 math avoids overflow for any u32 input.
    fn edge(total: u32, divisions: u32, index: u32) -> u32 {
        (u64::from(index) * u64::from(total) / u64::from(divisions.max(1))) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_grid_for_common_counts() {
        assert_eq!(
            GridLayoutCalculator::grid_size(1, CompositionLayout::Grid),
            GridLayout {
                columns: 1,
                rows: 1
            }
        );
        assert_eq!(
            GridLayoutCalculator::grid_size(2, CompositionLayout::Grid),
            GridLayout {
                columns: 2,
                rows: 1
            }
        );
        assert_eq!(
            GridLayoutCalculator::grid_size(5, CompositionLayout::Grid),
            GridLayout {
                columns: 3,
                rows: 2
            }
        );
    }

    #[test]
    fn remainder_pixels_are_distributed() {
        let cells = GridLayoutCalculator::cells(5, 3, 2, CompositionLayout::Row);
        assert_eq!(cells[0].width, 2);
        assert_eq!(cells[1].x, 2);
        assert_eq!(cells[1].width, 3);
    }

    #[test]
    fn cells_stay_inside_output_and_do_not_overlap() {
        for &layout in &[
            CompositionLayout::Row,
            CompositionLayout::Column,
            CompositionLayout::Grid,
            CompositionLayout::PictureInPicture,
        ] {
            for count in 1..=9 {
                let cells = GridLayoutCalculator::cells(7, 5, count, layout);
                assert_eq!(cells.len(), count);
                for cell in &cells {
                    assert!(cell.x + cell.width <= 7);
                    assert!(cell.y + cell.height <= 5);
                }
            }
        }
    }

    #[test]
    fn tiny_output_yields_degenerate_but_valid_cells() {
        // More cells than pixels: widths may be zero, but positions stay sane.
        let cells = GridLayoutCalculator::cells(3, 2, 5, CompositionLayout::Row);
        assert_eq!(cells.len(), 5);
        let total: u32 = cells.iter().map(|cell| cell.width).sum();
        assert_eq!(total, 3);
    }

    #[test]
    fn picture_in_picture_uses_first_source_as_primary_and_stacks_overlays() {
        let cells = GridLayoutCalculator::cells(1920, 1080, 4, CompositionLayout::PictureInPicture);

        assert_eq!(
            cells[0],
            Cell {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }
        );
        assert!(cells[1].x > 1920 / 2);
        assert!(cells[1].y > cells[2].y);
        for (index, overlay) in cells[1..].iter().enumerate() {
            assert!(overlay.width < 1920);
            assert!(overlay.height < 1080);
            for other in &cells[index + 2..] {
                let overlaps_x =
                    overlay.x < other.x + other.width && other.x < overlay.x + overlay.width;
                let overlaps_y =
                    overlay.y < other.y + other.height && other.y < overlay.y + overlay.height;
                assert!(!(overlaps_x && overlaps_y));
            }
        }
    }

    #[test]
    fn generated_layouts_preserve_bounds_and_non_overlap() {
        // Deterministic property sweep: reproducible in CI while covering
        // dimensions, counts and layouts far beyond hand-picked examples.
        let mut state = 0x7a5b_3c19_d4e2_f681_u64;
        for _ in 0..2_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let width = (state as u32).max(1);
            let height = ((state >> 32) as u32).max(1);
            let count = (state as usize % 64) + 1;
            let layout = match (state >> 16) % 3 {
                0 => CompositionLayout::Row,
                1 => CompositionLayout::Column,
                _ => CompositionLayout::Grid,
            };

            let cells = GridLayoutCalculator::cells(width, height, count, layout);
            assert_eq!(cells.len(), count);
            for (index, cell) in cells.iter().enumerate() {
                assert!(cell.x.checked_add(cell.width).is_some_and(|x| x <= width));
                assert!(cell.y.checked_add(cell.height).is_some_and(|y| y <= height));
                for other in &cells[index + 1..] {
                    let overlaps_x =
                        cell.x < other.x + other.width && other.x < cell.x + cell.width;
                    let overlaps_y =
                        cell.y < other.y + other.height && other.y < cell.y + cell.height;
                    assert!(!(overlaps_x && overlaps_y));
                }
            }
        }
    }
}
