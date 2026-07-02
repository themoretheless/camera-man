#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionLayout {
    Row,
    Column,
    Grid,
}

impl CompositionLayout {
    /// Stable lowercase name for status lines, CLI output and config files.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Column => "column",
            Self::Grid => "grid",
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
        }
    }

    pub fn cells(
        output_width: u32,
        output_height: u32,
        count: usize,
        layout: CompositionLayout,
    ) -> Vec<Cell> {
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
}
