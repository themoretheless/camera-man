use crate::config::VideoFormat;
use crate::error::CameraManError;
use crate::frame::{CapturedFrame, Frame, PixelFormat};
use crate::layout::{Cell, CompositionLayout, GridLayoutCalculator};

#[derive(Debug, Clone)]
pub struct Compositor {
    format: VideoFormat,
    background: [u8; 4],
    empty_cell: [u8; 4],
}

impl Compositor {
    pub const fn new(format: VideoFormat) -> Self {
        Self {
            format,
            background: [0, 0, 0, 255],
            empty_cell: [32, 32, 32, 255],
        }
    }

    pub const fn format(&self) -> VideoFormat {
        self.format
    }

    pub fn compose(
        &self,
        frames: &[Option<Frame>],
        layout: CompositionLayout,
    ) -> Result<Frame, CameraManError> {
        if frames.is_empty() {
            return Err(CameraManError::EmptyInput);
        }
        if self.format.pixel_format != PixelFormat::Bgra8 {
            return Err(CameraManError::UnsupportedPixelFormat);
        }

        let mut output = Frame::solid_bgra(self.format.width, self.format.height, self.background)?;
        let cells = GridLayoutCalculator::cells(
            self.format.width,
            self.format.height,
            frames.len(),
            layout,
        );

        for (cell, frame) in cells.iter().zip(frames.iter()) {
            fill_cell(&mut output, *cell, self.empty_cell);
            if let Some(frame) = frame {
                paste_aspect_fit(&mut output, frame, *cell)?;
            }
        }

        Ok(output)
    }

    pub fn compose_captured(
        &self,
        frames: &[Option<CapturedFrame>],
        layout: CompositionLayout,
    ) -> Result<Frame, CameraManError> {
        let raw_frames = frames
            .iter()
            .map(|frame| frame.as_ref().map(|captured| captured.frame().clone()))
            .collect::<Vec<_>>();
        self.compose(&raw_frames, layout)
    }
}

fn fill_cell(output: &mut Frame, cell: Cell, bgra: [u8; 4]) {
    for y in cell.y..cell.y + cell.height {
        for x in cell.x..cell.x + cell.width {
            output.set_bgra(x, y, bgra);
        }
    }
}

fn paste_aspect_fit(output: &mut Frame, input: &Frame, cell: Cell) -> Result<(), CameraManError> {
    if input.pixel_format() != PixelFormat::Bgra8 {
        return Err(CameraManError::UnsupportedPixelFormat);
    }
    if input.width() == 0 || input.height() == 0 {
        return Err(CameraManError::EmptyFrame);
    }
    // A cell can legitimately be zero-sized when there are more sources than
    // output pixels along one axis; there is simply nothing to draw.
    if cell.width == 0 || cell.height == 0 {
        return Ok(());
    }

    let scale_x = cell.width as f32 / input.width() as f32;
    let scale_y = cell.height as f32 / input.height() as f32;
    let scale = scale_x.min(scale_y);
    // Clamp to the cell: rounding (and the min-1-pixel floor) must never push
    // the destination outside the cell, otherwise the centering subtraction
    // below underflows u32 and panics.
    let dest_width = ((input.width() as f32 * scale).round() as u32)
        .max(1)
        .min(cell.width);
    let dest_height = ((input.height() as f32 * scale).round() as u32)
        .max(1)
        .min(cell.height);
    let dest_x = cell.x + (cell.width - dest_width) / 2;
    let dest_y = cell.y + (cell.height - dest_height) / 2;

    for y in 0..dest_height {
        // u64 math: y * input.height() overflows u32 for tall inputs (e.g. 2x2_000_000).
        let source_y = sample_coordinate(y, input.height(), dest_height);
        for x in 0..dest_width {
            let source_x = sample_coordinate(x, input.width(), dest_width);
            if let Some(pixel) = input.bgra_at(source_x, source_y) {
                output.set_bgra(dest_x + x, dest_y + y, pixel);
            }
        }
    }

    Ok(())
}

/// Nearest-neighbor source coordinate for a destination coordinate.
/// Computed in u64 so `dest * source_size` cannot overflow, and clamped so the
/// result never reaches `source_size` even with degenerate inputs.
fn sample_coordinate(dest: u32, source_size: u32, dest_size: u32) -> u32 {
    let coordinate =
        (u64::from(dest) * u64::from(source_size) / u64::from(dest_size.max(1))) as u32;
    coordinate.min(source_size.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_two_frames_in_a_row() {
        let compositor = Compositor::new(VideoFormat {
            width: 4,
            height: 2,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let left = Frame::solid_bgra(1, 1, [255, 0, 0, 255]).unwrap();
        let right = Frame::solid_bgra(1, 1, [0, 255, 0, 255]).unwrap();

        let output = compositor
            .compose(&[Some(left), Some(right)], CompositionLayout::Row)
            .unwrap();

        assert_eq!(output.bgra_at(0, 0), Some([255, 0, 0, 255]));
        assert_eq!(output.bgra_at(3, 0), Some([0, 255, 0, 255]));
    }

    #[test]
    fn more_sources_than_row_pixels_does_not_panic() {
        // Regression: zero-width cells caused a u32 underflow panic in debug builds.
        let compositor = Compositor::new(VideoFormat {
            width: 3,
            height: 2,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let frame = Frame::solid_bgra(2, 2, [9, 9, 9, 255]).unwrap();
        let frames = vec![Some(frame); 5];
        compositor.compose(&frames, CompositionLayout::Row).unwrap();
    }

    #[test]
    fn more_sources_than_column_pixels_does_not_panic() {
        // Regression: same underflow on the vertical axis for Column layout.
        let compositor = Compositor::new(VideoFormat {
            width: 4,
            height: 3,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let frame = Frame::solid_bgra(2, 2, [9, 9, 9, 255]).unwrap();
        let frames = vec![Some(frame); 5];
        compositor
            .compose(&frames, CompositionLayout::Column)
            .unwrap();
    }

    #[test]
    fn very_tall_source_does_not_overflow_sampling() {
        // Regression: y * input.height() overflowed u32 during nearest-neighbor
        // sampling for extreme aspect ratios.
        let compositor = Compositor::new(VideoFormat {
            width: 8,
            height: 8,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let tall = Frame::solid_bgra(2, 2_000_000, [1, 2, 3, 255]).unwrap();
        compositor
            .compose(&[Some(tall)], CompositionLayout::Grid)
            .unwrap();
    }

    #[test]
    fn preserves_aspect_ratio_with_empty_bars() {
        let compositor = Compositor::new(VideoFormat {
            width: 4,
            height: 4,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let wide = Frame::solid_bgra(4, 2, [1, 2, 3, 255]).unwrap();
        let output = compositor
            .compose(&[Some(wide)], CompositionLayout::Grid)
            .unwrap();

        assert_eq!(output.bgra_at(0, 0), Some([32, 32, 32, 255]));
        assert_eq!(output.bgra_at(0, 1), Some([1, 2, 3, 255]));
    }
}
