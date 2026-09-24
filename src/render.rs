use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::config::VideoFormat;
use crate::error::CameraManError;
use crate::frame::{CapturedFrame, Frame, PixelFormat};
use crate::layout::{Cell, CompositionLayout, GridLayoutCalculator};
use crate::media_contract::{
    AlphaMode, ColorRange, Colorimetry, FrameContract, PixelAspectRatio, Rotation,
};
use crate::performance::{CopyStage, copy_ledger};
use crate::source_transform::{SourceFit, SourceTransform, TRANSFORM_SCALE};
use serde::{Deserialize, Serialize};

#[cfg(feature = "parallel-compositor")]
use rayon::prelude::*;

pub const PARALLEL_RESIZE_PIXEL_THRESHOLD: u64 = 512 * 512;
pub const PARALLEL_NEAREST_RESIZE_PIXEL_THRESHOLD: u64 = 768 * 768;

/// Resampling algorithm used when an input does not match its destination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalingFilter {
    /// Fast nearest-neighbor sampling; the application default.
    #[default]
    Nearest,
    /// Four-sample interpolation for smoother resized output.
    Bilinear,
}

/// Pure-Rust aspect-fit compositor for tightly packed BGRA frames.
#[derive(Debug, Clone)]
pub struct Compositor {
    format: VideoFormat,
    background: [u8; 4],
    empty_cell: [u8; 4],
    scaling_filter: ScalingFilter,
    sampling_cache: Arc<Mutex<SamplingCache>>,
}

impl Compositor {
    pub fn new(format: VideoFormat) -> Self {
        Self {
            format,
            background: [0, 0, 0, 255],
            empty_cell: [32, 32, 32, 255],
            scaling_filter: ScalingFilter::Nearest,
            sampling_cache: Arc::new(Mutex::new(SamplingCache::default())),
        }
    }

    pub const fn format(&self) -> VideoFormat {
        self.format
    }

    pub const fn scaling_filter(&self) -> ScalingFilter {
        self.scaling_filter
    }

    pub fn with_scaling_filter(mut self, scaling_filter: ScalingFilter) -> Self {
        self.scaling_filter = scaling_filter;
        self
    }

    pub fn set_scaling_filter(&mut self, scaling_filter: ScalingFilter) {
        self.scaling_filter = scaling_filter;
    }

    pub fn compose(
        &self,
        frames: &[Option<Frame>],
        layout: CompositionLayout,
    ) -> Result<Frame, CameraManError> {
        let mut output = self.allocate_output()?;
        self.compose_into(frames, layout, &mut output)?;
        Ok(output)
    }

    pub fn compose_into(
        &self,
        frames: &[Option<Frame>],
        layout: CompositionLayout,
        output: &mut Frame,
    ) -> Result<(), CameraManError> {
        self.compose_with(frames.len(), layout, output, |index| {
            frames[index]
                .as_ref()
                .map(|frame| (frame, SourceTransform::default()))
        })
    }

    pub fn compose_into_with_transforms(
        &self,
        frames: &[Option<Frame>],
        transforms: &[SourceTransform],
        layout: CompositionLayout,
        output: &mut Frame,
    ) -> Result<(), CameraManError> {
        self.compose_with(frames.len(), layout, output, |index| {
            frames[index]
                .as_ref()
                .map(|frame| (frame, transforms.get(index).copied().unwrap_or_default()))
        })
    }

    pub fn compose_borrowed_into(
        &self,
        frames: &[Option<&Frame>],
        layout: CompositionLayout,
        output: &mut Frame,
    ) -> Result<(), CameraManError> {
        self.compose_with(frames.len(), layout, output, |index| {
            frames[index].map(|frame| (frame, SourceTransform::default()))
        })
    }

    fn compose_with<'a>(
        &self,
        frame_count: usize,
        layout: CompositionLayout,
        output: &mut Frame,
        mut frame_at: impl FnMut(usize) -> Option<(&'a Frame, SourceTransform)>,
    ) -> Result<(), CameraManError> {
        if frame_count == 0 {
            return Err(CameraManError::EmptyInput);
        }
        self.validate_output(output)?;

        let cells =
            GridLayoutCalculator::cells(self.format.width, self.format.height, frame_count, layout);
        if !cells_cover_output(&cells, self.format.width, self.format.height) {
            fill_cell(
                output,
                Cell {
                    x: 0,
                    y: 0,
                    width: self.format.width,
                    height: self.format.height,
                },
                self.background,
            );
        }

        for (index, cell) in cells.into_iter().enumerate() {
            if let Some((frame, transform)) = frame_at(index) {
                if !frame_fully_overwrites_cell(frame, cell, transform)? {
                    fill_cell(output, cell, self.empty_cell);
                }
                self.paste_aspect_fit(output, frame, cell, transform)?;
            } else {
                fill_cell(output, cell, self.empty_cell);
            }
        }
        copy_ledger().record_copy(CopyStage::CompositionOutput, output.data().len());
        Ok(())
    }

    fn validate_output(&self, output: &Frame) -> Result<(), CameraManError> {
        if output.width() != self.format.width || output.height() != self.format.height {
            return Err(CameraManError::InvalidDimensions {
                width: output.width(),
                height: output.height(),
            });
        }
        if output.pixel_format() != PixelFormat::Bgra8
            || self.format.pixel_format != PixelFormat::Bgra8
        {
            return Err(CameraManError::UnsupportedPixelFormat);
        }
        Ok(())
    }

    fn allocate_output(&self) -> Result<Frame, CameraManError> {
        let output = Frame::solid_bgra(self.format.width, self.format.height, self.background)?;
        copy_ledger().record_allocation(CopyStage::CompositionOutput, output.data().len());
        Ok(output)
    }

    fn paste_aspect_fit(
        &self,
        output: &mut Frame,
        input: &Frame,
        cell: Cell,
        transform: SourceTransform,
    ) -> Result<(), CameraManError> {
        if input.pixel_format() != PixelFormat::Bgra8 {
            return Err(CameraManError::UnsupportedPixelFormat);
        }
        paste_aspect_fit(
            output,
            input,
            cell,
            self.scaling_filter,
            &self.sampling_cache,
            transform,
        )
    }

    pub fn compose_captured(
        &self,
        frames: &[Option<CapturedFrame>],
        layout: CompositionLayout,
    ) -> Result<Frame, CameraManError> {
        let mut output = self.allocate_output()?;
        self.compose_captured_into(frames, layout, &mut output)?;
        Ok(output)
    }

    pub fn compose_captured_into(
        &self,
        frames: &[Option<CapturedFrame>],
        layout: CompositionLayout,
        output: &mut Frame,
    ) -> Result<(), CameraManError> {
        self.compose_with(frames.len(), layout, output, |index| {
            frames[index]
                .as_ref()
                .map(|frame| (frame.frame(), SourceTransform::default()))
        })
    }

    pub fn compose_captured_into_with_transforms(
        &self,
        frames: &[Option<CapturedFrame>],
        transforms: &[SourceTransform],
        layout: CompositionLayout,
        output: &mut Frame,
    ) -> Result<(), CameraManError> {
        self.compose_with(frames.len(), layout, output, |index| {
            frames[index].as_ref().map(|frame| {
                (
                    frame.frame(),
                    transforms.get(index).copied().unwrap_or_default(),
                )
            })
        })
    }

    pub fn compose_captured_with_transforms(
        &self,
        frames: &[Option<CapturedFrame>],
        transforms: &[SourceTransform],
        layout: CompositionLayout,
    ) -> Result<Frame, CameraManError> {
        let mut output = self.allocate_output()?;
        self.compose_captured_into_with_transforms(frames, transforms, layout, &mut output)?;
        Ok(output)
    }
}

const MAX_SAMPLING_MAPS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SamplingKey {
    source_width: u32,
    source_height: u32,
    dest_width: u32,
    dest_height: u32,
    scaling_filter: ScalingFilter,
}

#[derive(Debug)]
enum SamplingMap {
    Nearest {
        x_offsets: Vec<usize>,
        y_rows: Vec<usize>,
    },
    Bilinear {
        x_samples: Vec<LinearSample>,
        y_samples: Vec<LinearSample>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TransformedSamplingKey {
    contract: FrameContract,
    source_x: u32,
    source_y: u32,
    source_width: u32,
    source_height: u32,
    dest_width: u32,
    dest_height: u32,
    scaling_filter: ScalingFilter,
}

#[derive(Debug)]
enum TransformedSamplingMap {
    Nearest {
        x_coordinates: Vec<usize>,
        y_coordinates: Vec<usize>,
    },
    Bilinear {
        x_samples: Vec<LinearSample>,
        y_samples: Vec<LinearSample>,
    },
}

#[derive(Debug, Default)]
struct SamplingCache {
    maps: HashMap<SamplingKey, SamplingMap>,
    transformed_maps: HashMap<TransformedSamplingKey, TransformedSamplingMap>,
}

impl SamplingCache {
    fn get(&mut self, key: SamplingKey) -> &SamplingMap {
        if !self.maps.contains_key(&key) {
            if self.maps.len() + self.transformed_maps.len() >= MAX_SAMPLING_MAPS {
                self.maps.clear();
                self.transformed_maps.clear();
            }
            let map = match key.scaling_filter {
                ScalingFilter::Nearest => SamplingMap::Nearest {
                    x_offsets: (0..key.dest_width)
                        .map(|x| {
                            nearest_source_coordinate(x, key.source_width, key.dest_width) as usize
                                * 4
                        })
                        .collect(),
                    y_rows: (0..key.dest_height)
                        .map(|y| {
                            nearest_source_coordinate(y, key.source_height, key.dest_height)
                                as usize
                                * key.source_width as usize
                                * 4
                        })
                        .collect(),
                },
                ScalingFilter::Bilinear => SamplingMap::Bilinear {
                    x_samples: (0..key.dest_width)
                        .map(|x| linear_sample(x, key.source_width, key.dest_width))
                        .collect(),
                    y_samples: (0..key.dest_height)
                        .map(|y| linear_sample(y, key.source_height, key.dest_height))
                        .collect(),
                },
            };
            self.maps.insert(key, map);
        }
        self.maps.get(&key).expect("sampling map inserted above")
    }

    fn get_transformed(&mut self, key: TransformedSamplingKey) -> &TransformedSamplingMap {
        if !self.transformed_maps.contains_key(&key) {
            if self.maps.len() + self.transformed_maps.len() >= MAX_SAMPLING_MAPS {
                self.maps.clear();
                self.transformed_maps.clear();
            }
            let (display_width, display_height) = key.contract.display_dimensions();
            let map = match key.scaling_filter {
                ScalingFilter::Nearest => TransformedSamplingMap::Nearest {
                    x_coordinates: (0..key.dest_width)
                        .map(|x| {
                            let coordinate = key.source_x
                                + nearest_source_coordinate(x, key.source_width, key.dest_width);
                            mirrored_coordinate(
                                coordinate,
                                display_width,
                                key.contract.transform.mirror_horizontal,
                            )
                        })
                        .collect(),
                    y_coordinates: (0..key.dest_height)
                        .map(|y| {
                            let coordinate = key.source_y
                                + nearest_source_coordinate(y, key.source_height, key.dest_height);
                            mirrored_coordinate(
                                coordinate,
                                display_height,
                                key.contract.transform.mirror_vertical,
                            )
                        })
                        .collect(),
                },
                ScalingFilter::Bilinear => TransformedSamplingMap::Bilinear {
                    x_samples: (0..key.dest_width)
                        .map(|x| {
                            mirrored_linear_sample(
                                shifted_linear_sample(
                                    x,
                                    key.source_width,
                                    key.dest_width,
                                    key.source_x,
                                ),
                                display_width,
                                key.contract.transform.mirror_horizontal,
                            )
                        })
                        .collect(),
                    y_samples: (0..key.dest_height)
                        .map(|y| {
                            mirrored_linear_sample(
                                shifted_linear_sample(
                                    y,
                                    key.source_height,
                                    key.dest_height,
                                    key.source_y,
                                ),
                                display_height,
                                key.contract.transform.mirror_vertical,
                            )
                        })
                        .collect(),
                },
            };
            self.transformed_maps.insert(key, map);
        }
        self.transformed_maps
            .get(&key)
            .expect("transformed sampling map inserted above")
    }
}

fn cells_cover_output(cells: &[Cell], width: u32, height: u32) -> bool {
    cells
        .iter()
        .any(|cell| cell.x == 0 && cell.y == 0 && cell.width == width && cell.height == height)
        || cells
            .iter()
            .map(|cell| u64::from(cell.width) * u64::from(cell.height))
            .sum::<u64>()
            == u64::from(width) * u64::from(height)
}

fn fill_cell(output: &mut Frame, cell: Cell, bgra: [u8; 4]) {
    if cell.width == 0 || cell.height == 0 {
        return;
    }
    let output_width = output.width() as usize;
    let data = output.data_mut();
    let first_row_start = (cell.y as usize * output_width + cell.x as usize) * 4;
    let first_row_end = first_row_start + cell.width as usize * 4;
    for pixel in data[first_row_start..first_row_end].chunks_exact_mut(4) {
        pixel.copy_from_slice(&bgra);
    }
    for y in cell.y + 1..cell.y + cell.height {
        let row_start = (y as usize * output_width + cell.x as usize) * 4;
        data.copy_within(first_row_start..first_row_end, row_start);
    }
}

fn frame_fully_overwrites_cell(
    input: &Frame,
    cell: Cell,
    transform: SourceTransform,
) -> Result<bool, CameraManError> {
    if cell.width == 0 || cell.height == 0 {
        return Ok(true);
    }
    transform.validate()?;
    let contract = input.contract();
    contract.validate(input.width(), input.height())?;
    if contract.colorimetry.alpha != AlphaMode::Opaque
        || transform.opacity_per_mille != TRANSFORM_SCALE
    {
        return Ok(false);
    }
    let contract = transform.apply_to_contract(contract, input.width(), input.height())?;
    let placement = transformed_placement(contract, cell, transform);
    Ok(placement.dest_x == cell.x
        && placement.dest_y == cell.y
        && placement.dest_width == cell.width
        && placement.dest_height == cell.height)
}

fn paste_aspect_fit(
    output: &mut Frame,
    input: &Frame,
    cell: Cell,
    scaling_filter: ScalingFilter,
    sampling_cache: &Arc<Mutex<SamplingCache>>,
    transform: SourceTransform,
) -> Result<(), CameraManError> {
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

    transform.validate()?;
    let contract = input.contract();
    contract.validate(input.width(), input.height())?;
    if !transform.is_identity() || !contract.is_canonical_bgra(input.width(), input.height()) {
        let contract = transform.apply_to_contract(contract, input.width(), input.height())?;
        return paste_contract_transformed(
            output,
            input,
            cell,
            scaling_filter,
            sampling_cache,
            contract,
            transform,
        );
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

    if dest_width == input.width() && dest_height == input.height() {
        paste_unscaled(output, input, dest_x, dest_y, dest_width, dest_height);
        return Ok(());
    }

    let key = SamplingKey {
        source_width: input.width(),
        source_height: input.height(),
        dest_width,
        dest_height,
        scaling_filter,
    };
    let mut cache = sampling_cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match cache.get(key) {
        SamplingMap::Nearest { x_offsets, y_rows } => paste_nearest(
            output,
            input,
            dest_x,
            dest_y,
            dest_height,
            x_offsets,
            y_rows,
        ),
        SamplingMap::Bilinear {
            x_samples,
            y_samples,
        } => paste_bilinear(
            output,
            input,
            dest_x,
            dest_y,
            dest_height,
            x_samples,
            y_samples,
        ),
    }

    Ok(())
}

fn paste_contract_transformed(
    output: &mut Frame,
    input: &Frame,
    cell: Cell,
    scaling_filter: ScalingFilter,
    sampling_cache: &Arc<Mutex<SamplingCache>>,
    contract: FrameContract,
    transform: SourceTransform,
) -> Result<(), CameraManError> {
    let placement = transformed_placement(contract, cell, transform);
    if contract.colorimetry == Colorimetry::BT709_FULL_OPAQUE
        && contract.pixel_aspect_ratio == PixelAspectRatio::SQUARE
    {
        paste_opaque_contract(
            output,
            input,
            scaling_filter,
            sampling_cache,
            contract,
            transform.opacity_per_mille,
            placement,
        );
        return Ok(());
    }
    paste_contract_transformed_generic(
        output,
        input,
        scaling_filter,
        contract,
        transform.opacity_per_mille,
        placement,
    );
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct TransformedPlacement {
    dest_width: u32,
    dest_height: u32,
    dest_x: u32,
    dest_y: u32,
    source_x: u32,
    source_y: u32,
    source_width: u32,
    source_height: u32,
}

fn transformed_placement(
    contract: FrameContract,
    cell: Cell,
    transform: SourceTransform,
) -> TransformedPlacement {
    let (display_width, display_height) = contract.display_dimensions();
    match transform.fit {
        SourceFit::Fit => {
            let scale_x = cell.width as f32 / display_width as f32;
            let scale_y = cell.height as f32 / display_height as f32;
            let scale = scale_x.min(scale_y);
            let dest_width = ((display_width as f32 * scale).round() as u32)
                .max(1)
                .min(cell.width);
            let dest_height = ((display_height as f32 * scale).round() as u32)
                .max(1)
                .min(cell.height);
            TransformedPlacement {
                dest_width,
                dest_height,
                dest_x: cell.x
                    + positioned_offset(cell.width - dest_width, transform.position_x_per_mille),
                dest_y: cell.y
                    + positioned_offset(cell.height - dest_height, transform.position_y_per_mille),
                source_x: 0,
                source_y: 0,
                source_width: display_width,
                source_height: display_height,
            }
        }
        SourceFit::Fill => {
            let source_is_wider = u64::from(display_width) * u64::from(cell.height)
                > u64::from(display_height) * u64::from(cell.width);
            let (source_width, source_height) = if source_is_wider {
                (
                    (u64::from(display_height) * u64::from(cell.width) / u64::from(cell.height))
                        .max(1)
                        .min(u64::from(display_width)) as u32,
                    display_height,
                )
            } else {
                (
                    display_width,
                    (u64::from(display_width) * u64::from(cell.height) / u64::from(cell.width))
                        .max(1)
                        .min(u64::from(display_height)) as u32,
                )
            };
            TransformedPlacement {
                dest_width: cell.width,
                dest_height: cell.height,
                dest_x: cell.x,
                dest_y: cell.y,
                source_x: positioned_offset(
                    display_width - source_width,
                    transform.position_x_per_mille,
                ),
                source_y: positioned_offset(
                    display_height - source_height,
                    transform.position_y_per_mille,
                ),
                source_width,
                source_height,
            }
        }
    }
}

fn paste_contract_transformed_generic(
    output: &mut Frame,
    input: &Frame,
    scaling_filter: ScalingFilter,
    contract: FrameContract,
    opacity_per_mille: u16,
    placement: TransformedPlacement,
) {
    let output_width = output.width() as usize;
    let output_data = output.data_mut();

    for y in 0..placement.dest_height {
        for x in 0..placement.dest_width {
            let source = match scaling_filter {
                ScalingFilter::Nearest => {
                    let display_x = placement.source_x
                        + nearest_source_coordinate(
                            x,
                            placement.source_width,
                            placement.dest_width,
                        );
                    let display_y = placement.source_y
                        + nearest_source_coordinate(
                            y,
                            placement.source_height,
                            placement.dest_height,
                        );
                    sample_premultiplied(input, contract, display_x, display_y)
                }
                ScalingFilter::Bilinear => {
                    let x_sample = shifted_linear_sample(
                        x,
                        placement.source_width,
                        placement.dest_width,
                        placement.source_x,
                    );
                    let y_sample = shifted_linear_sample(
                        y,
                        placement.source_height,
                        placement.dest_height,
                        placement.source_y,
                    );
                    sample_contract_bilinear(input, contract, x_sample, y_sample)
                }
            };
            let source = apply_opacity(source, opacity_per_mille);
            let offset = ((placement.dest_y + y) as usize * output_width
                + (placement.dest_x + x) as usize)
                * 4;
            blend_over_opaque(&mut output_data[offset..offset + 4], source);
        }
    }
}

fn paste_opaque_contract(
    output: &mut Frame,
    input: &Frame,
    scaling_filter: ScalingFilter,
    sampling_cache: &Arc<Mutex<SamplingCache>>,
    contract: FrameContract,
    opacity_per_mille: u16,
    placement: TransformedPlacement,
) {
    let key = TransformedSamplingKey {
        contract,
        source_x: placement.source_x,
        source_y: placement.source_y,
        source_width: placement.source_width,
        source_height: placement.source_height,
        dest_width: placement.dest_width,
        dest_height: placement.dest_height,
        scaling_filter,
    };
    let mut cache = sampling_cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match cache.get_transformed(key) {
        TransformedSamplingMap::Nearest {
            x_coordinates,
            y_coordinates,
        } => paste_transformed_nearest(
            output,
            input,
            contract,
            opacity_per_mille,
            placement,
            x_coordinates,
            y_coordinates,
        ),
        TransformedSamplingMap::Bilinear {
            x_samples,
            y_samples,
        } => paste_transformed_bilinear(
            output,
            input,
            contract,
            opacity_per_mille,
            placement,
            x_samples,
            y_samples,
        ),
    }
}

fn paste_transformed_nearest(
    output: &mut Frame,
    input: &Frame,
    contract: FrameContract,
    opacity_per_mille: u16,
    placement: TransformedPlacement,
    x_coordinates: &[usize],
    y_coordinates: &[usize],
) {
    const BILINEAR_SCALE: u64 = 256 * 256;
    let input_width = input.width() as usize;
    let output_width = output.width() as usize;
    let input_data = input.data();
    let output_data = output.data_mut();
    let opacity = u64::from(opacity_per_mille.min(TRANSFORM_SCALE));

    for (y, &source_y) in y_coordinates.iter().enumerate() {
        let destination_row = (placement.dest_y as usize + y) * output_width * 4;
        for (x, &source_x) in x_coordinates.iter().enumerate() {
            let source_offset =
                transformed_source_offset(contract, input_width, source_x, source_y);
            let destination_offset = destination_row + (placement.dest_x as usize + x) * 4;
            let destination = &mut output_data[destination_offset..destination_offset + 4];
            if opacity_per_mille == TRANSFORM_SCALE {
                destination[..3].copy_from_slice(&input_data[source_offset..source_offset + 3]);
            } else {
                for channel in 0..3 {
                    let source_weighted = u64::from(input_data[source_offset + channel])
                        .saturating_mul(BILINEAR_SCALE);
                    destination[channel] =
                        blend_weighted_channel(source_weighted, destination[channel], opacity);
                }
            }
            destination[3] = 255;
        }
    }
}

fn paste_transformed_bilinear(
    output: &mut Frame,
    input: &Frame,
    contract: FrameContract,
    opacity_per_mille: u16,
    placement: TransformedPlacement,
    x_samples: &[LinearSample],
    y_samples: &[LinearSample],
) {
    const WEIGHT_SCALE: u64 = 256;
    let input_width = input.width() as usize;
    let output_width = output.width() as usize;
    let input_data = input.data();
    let output_data = output.data_mut();
    let opacity = u64::from(opacity_per_mille.min(TRANSFORM_SCALE));

    for (y, y_sample) in y_samples.iter().copied().enumerate() {
        let low_y_weight = WEIGHT_SCALE - u64::from(y_sample.high_weight);
        let destination_row = (placement.dest_y as usize + y) * output_width * 4;
        for (x, x_sample) in x_samples.iter().copied().enumerate() {
            let low_x_weight = WEIGHT_SCALE - u64::from(x_sample.high_weight);
            let offsets = [
                transformed_source_offset(contract, input_width, x_sample.low, y_sample.low),
                transformed_source_offset(contract, input_width, x_sample.high, y_sample.low),
                transformed_source_offset(contract, input_width, x_sample.low, y_sample.high),
                transformed_source_offset(contract, input_width, x_sample.high, y_sample.high),
            ];
            let destination_offset = destination_row + (placement.dest_x as usize + x) * 4;
            let destination = &mut output_data[destination_offset..destination_offset + 4];
            for channel in 0..3 {
                let top = u64::from(input_data[offsets[0] + channel]) * low_x_weight
                    + u64::from(input_data[offsets[1] + channel]) * u64::from(x_sample.high_weight);
                let bottom = u64::from(input_data[offsets[2] + channel]) * low_x_weight
                    + u64::from(input_data[offsets[3] + channel]) * u64::from(x_sample.high_weight);
                let source_weighted = top * low_y_weight + bottom * u64::from(y_sample.high_weight);
                destination[channel] =
                    blend_weighted_channel(source_weighted, destination[channel], opacity);
            }
            destination[3] = 255;
        }
    }
}

#[inline]
fn transformed_source_offset(
    contract: FrameContract,
    input_width: usize,
    transformed_x: usize,
    transformed_y: usize,
) -> usize {
    let aperture = contract.clean_aperture;
    let aperture_width = aperture.width as usize;
    let aperture_height = aperture.height as usize;
    let (source_x, source_y) = match contract.transform.rotation {
        Rotation::Degrees0 => (transformed_x, transformed_y),
        Rotation::Degrees90 => (
            transformed_y,
            aperture_height
                .saturating_sub(1)
                .saturating_sub(transformed_x),
        ),
        Rotation::Degrees180 => (
            aperture_width
                .saturating_sub(1)
                .saturating_sub(transformed_x),
            aperture_height
                .saturating_sub(1)
                .saturating_sub(transformed_y),
        ),
        Rotation::Degrees270 => (
            aperture_width
                .saturating_sub(1)
                .saturating_sub(transformed_y),
            transformed_x,
        ),
    };
    ((aperture.y as usize + source_y) * input_width + aperture.x as usize + source_x) * 4
}

#[inline]
fn blend_weighted_channel(source_weighted: u64, destination: u8, opacity: u64) -> u8 {
    const BILINEAR_SCALE: u64 = 256 * 256;
    let opacity_scale = u64::from(TRANSFORM_SCALE);
    let denominator = BILINEAR_SCALE * opacity_scale;
    let numerator = source_weighted * opacity
        + u64::from(destination) * BILINEAR_SCALE * (opacity_scale - opacity);
    ((numerator + denominator / 2) / denominator) as u8
}

fn mirrored_coordinate(coordinate: u32, size: u32, mirror: bool) -> usize {
    let coordinate = coordinate.min(size.saturating_sub(1));
    if mirror {
        size.saturating_sub(1).saturating_sub(coordinate) as usize
    } else {
        coordinate as usize
    }
}

fn mirrored_linear_sample(mut sample: LinearSample, size: u32, mirror: bool) -> LinearSample {
    if mirror {
        let last = size.saturating_sub(1) as usize;
        sample.low = last.saturating_sub(sample.low);
        sample.high = last.saturating_sub(sample.high);
    }
    sample
}

fn positioned_offset(space: u32, position_per_mille: i16) -> u32 {
    let normalized = i32::from(position_per_mille.clamp(-1_000, 1_000)) + 1_000;
    (u64::from(space) * normalized as u64 / 2_000) as u32
}

fn shifted_linear_sample(
    dest: u32,
    source_size: u32,
    dest_size: u32,
    source_offset: u32,
) -> LinearSample {
    let sample = linear_sample(dest, source_size, dest_size);
    LinearSample {
        low: sample.low + source_offset as usize,
        high: sample.high + source_offset as usize,
        high_weight: sample.high_weight,
    }
}

#[derive(Debug, Clone, Copy)]
struct PremultipliedPixel {
    channels: [f32; 3],
    alpha: f32,
}

fn apply_opacity(mut pixel: PremultipliedPixel, opacity_per_mille: u16) -> PremultipliedPixel {
    let opacity = f32::from(opacity_per_mille.min(TRANSFORM_SCALE)) / f32::from(TRANSFORM_SCALE);
    pixel.alpha *= opacity;
    for channel in &mut pixel.channels {
        *channel *= opacity;
    }
    pixel
}

fn sample_premultiplied(
    input: &Frame,
    contract: FrameContract,
    display_x: u32,
    display_y: u32,
) -> PremultipliedPixel {
    let (source_x, source_y) = contract.source_coordinate(display_x, display_y);
    let pixel = input
        .bgra_at(source_x, source_y)
        .expect("media contract coordinates were validated");
    let alpha = match contract.colorimetry.alpha {
        AlphaMode::Opaque => 1.0,
        AlphaMode::Straight | AlphaMode::Premultiplied => f32::from(pixel[3]) / 255.0,
    };
    let mut channels = [0.0; 3];
    for (destination, channel) in channels.iter_mut().zip(pixel[..3].iter().copied()) {
        let normalized = match contract.colorimetry.range {
            ColorRange::Full => f32::from(channel) / 255.0,
            ColorRange::Limited => ((f32::from(channel) - 16.0) / 219.0).clamp(0.0, 1.0),
        };
        *destination = match contract.colorimetry.alpha {
            AlphaMode::Straight => normalized * alpha,
            AlphaMode::Opaque | AlphaMode::Premultiplied => normalized,
        };
    }
    PremultipliedPixel { channels, alpha }
}

fn sample_contract_bilinear(
    input: &Frame,
    contract: FrameContract,
    x: LinearSample,
    y: LinearSample,
) -> PremultipliedPixel {
    let x_weight = x.high_weight as f32 / 256.0;
    let y_weight = y.high_weight as f32 / 256.0;
    let samples = [
        sample_premultiplied(input, contract, x.low as u32, y.low as u32),
        sample_premultiplied(input, contract, x.high as u32, y.low as u32),
        sample_premultiplied(input, contract, x.low as u32, y.high as u32),
        sample_premultiplied(input, contract, x.high as u32, y.high as u32),
    ];
    let weights = [
        (1.0 - x_weight) * (1.0 - y_weight),
        x_weight * (1.0 - y_weight),
        (1.0 - x_weight) * y_weight,
        x_weight * y_weight,
    ];
    let mut result = PremultipliedPixel {
        channels: [0.0; 3],
        alpha: 0.0,
    };
    for (sample, weight) in samples.into_iter().zip(weights) {
        result.alpha += sample.alpha * weight;
        for channel in 0..3 {
            result.channels[channel] += sample.channels[channel] * weight;
        }
    }
    result
}

fn blend_over_opaque(destination: &mut [u8], source: PremultipliedPixel) {
    for (channel, source_channel) in source.channels.into_iter().enumerate() {
        let background = f32::from(destination[channel]) / 255.0;
        destination[channel] =
            ((source_channel + background * (1.0 - source.alpha)).clamp(0.0, 1.0) * 255.0).round()
                as u8;
    }
    destination[3] = 255;
}

fn paste_unscaled(
    output: &mut Frame,
    input: &Frame,
    dest_x: u32,
    dest_y: u32,
    dest_width: u32,
    dest_height: u32,
) {
    let input_stride = input.width() as usize * 4;
    let output_stride = output.width() as usize * 4;
    let row_bytes = dest_width as usize * 4;
    let input_data = input.data();
    let output_data = output.data_mut();
    for row in 0..dest_height as usize {
        let source_start = row * input_stride;
        let destination_start = (dest_y as usize + row) * output_stride + dest_x as usize * 4;
        output_data[destination_start..destination_start + row_bytes]
            .copy_from_slice(&input_data[source_start..source_start + row_bytes]);
    }
}

fn paste_nearest(
    output: &mut Frame,
    input: &Frame,
    dest_x: u32,
    dest_y: u32,
    dest_height: u32,
    source_x_offsets: &[usize],
    source_rows: &[usize],
) {
    let output_width = output.width() as usize;
    let output_stride = output_width * 4;
    let input_data = input.data();
    let output_data = output.data_mut();
    let destination_start = dest_y as usize * output_stride;
    let destination_end = destination_start + dest_height as usize * output_stride;
    let destination_rows = &mut output_data[destination_start..destination_end];

    #[cfg(feature = "parallel-compositor")]
    if should_parallelize(
        source_x_offsets.len(),
        dest_height as usize,
        PARALLEL_NEAREST_RESIZE_PIXEL_THRESHOLD,
    ) {
        destination_rows
            .par_chunks_mut(output_stride)
            .enumerate()
            .for_each(|(y, row)| {
                paste_nearest_row(
                    row,
                    input_data,
                    source_rows[y],
                    dest_x as usize,
                    source_x_offsets,
                );
            });
        return;
    }

    for (y, row) in destination_rows.chunks_mut(output_stride).enumerate() {
        paste_nearest_row(
            row,
            input_data,
            source_rows[y],
            dest_x as usize,
            source_x_offsets,
        );
    }
}

fn paste_nearest_row(
    destination_row: &mut [u8],
    input_data: &[u8],
    source_row_offset: usize,
    dest_x: usize,
    source_x_offsets: &[usize],
) {
    for (x, source_x_offset) in source_x_offsets.iter().enumerate() {
        let source_offset = source_row_offset + source_x_offset;
        let destination_offset = (dest_x + x) * 4;
        destination_row[destination_offset..destination_offset + 4]
            .copy_from_slice(&input_data[source_offset..source_offset + 4]);
    }
}

#[derive(Debug, Clone, Copy)]
struct LinearSample {
    low: usize,
    high: usize,
    high_weight: u32,
}

fn paste_bilinear(
    output: &mut Frame,
    input: &Frame,
    dest_x: u32,
    dest_y: u32,
    dest_height: u32,
    x_samples: &[LinearSample],
    y_samples: &[LinearSample],
) {
    let source_width = input.width() as usize;
    let output_width = output.width() as usize;
    let output_stride = output_width * 4;
    let input_data = input.data();
    let output_data = output.data_mut();
    let destination_start = dest_y as usize * output_stride;
    let destination_end = destination_start + dest_height as usize * output_stride;
    let destination_rows = &mut output_data[destination_start..destination_end];

    #[cfg(feature = "parallel-compositor")]
    if should_parallelize(
        x_samples.len(),
        dest_height as usize,
        PARALLEL_RESIZE_PIXEL_THRESHOLD,
    ) {
        destination_rows
            .par_chunks_mut(output_stride)
            .enumerate()
            .for_each(|(y, row)| {
                paste_bilinear_row(
                    row,
                    input_data,
                    source_width,
                    dest_x as usize,
                    x_samples,
                    y_samples[y],
                );
            });
        return;
    }

    for (y, row) in destination_rows.chunks_mut(output_stride).enumerate() {
        paste_bilinear_row(
            row,
            input_data,
            source_width,
            dest_x as usize,
            x_samples,
            y_samples[y],
        );
    }
}

fn paste_bilinear_row(
    destination_row: &mut [u8],
    input_data: &[u8],
    source_width: usize,
    dest_x: usize,
    x_samples: &[LinearSample],
    y_sample: LinearSample,
) {
    const WEIGHT_SCALE: u32 = 256;

    let low_row = y_sample.low * source_width * 4;
    let high_row = y_sample.high * source_width * 4;
    let low_y_weight = WEIGHT_SCALE - y_sample.high_weight;
    for (x, x_sample) in x_samples.iter().enumerate() {
        let low_x_weight = WEIGHT_SCALE - x_sample.high_weight;
        let low_x = x_sample.low * 4;
        let high_x = x_sample.high * 4;
        let destination_offset = (dest_x + x) * 4;

        for channel in 0..4 {
            let top = u32::from(input_data[low_row + low_x + channel]) * low_x_weight
                + u32::from(input_data[low_row + high_x + channel]) * x_sample.high_weight;
            let bottom = u32::from(input_data[high_row + low_x + channel]) * low_x_weight
                + u32::from(input_data[high_row + high_x + channel]) * x_sample.high_weight;
            destination_row[destination_offset + channel] = ((top * low_y_weight
                + bottom * y_sample.high_weight
                + WEIGHT_SCALE * WEIGHT_SCALE / 2)
                / (WEIGHT_SCALE * WEIGHT_SCALE))
                as u8;
        }
    }
}

#[cfg(feature = "parallel-compositor")]
fn should_parallelize(width: usize, height: usize, pixel_threshold: u64) -> bool {
    (width as u64).saturating_mul(height as u64) >= pixel_threshold
        && rayon::current_num_threads() > 1
}

fn linear_sample(dest: u32, source_size: u32, dest_size: u32) -> LinearSample {
    if source_size <= 1 {
        return LinearSample {
            low: 0,
            high: 0,
            high_weight: 0,
        };
    }

    let position = ((f64::from(dest) + 0.5) * f64::from(source_size) / f64::from(dest_size.max(1))
        - 0.5)
        .clamp(0.0, f64::from(source_size - 1));
    let low = position.floor() as u32;
    let high = (low + 1).min(source_size - 1);
    let high_weight = ((position - f64::from(low)) * 256.0).round() as u32;

    LinearSample {
        low: low as usize,
        high: high as usize,
        high_weight: high_weight.min(256),
    }
}

/// Nearest-neighbor source coordinate for a destination coordinate.
/// Computed in u64 so `dest * source_size` cannot overflow, and clamped so the
/// result never reaches `source_size` even with degenerate inputs.
///
/// This is the one definition of nearest scaling in the crate: the compositor
/// and the CoreVideo upload path both go through it, so the bound proven below
/// covers every producer of output pixels.
pub fn nearest_source_coordinate(dest: u32, source_size: u32, dest_size: u32) -> u32 {
    let coordinate =
        (u64::from(dest) * u64::from(source_size) / u64::from(dest_size.max(1))) as u32;
    coordinate.min(source_size.saturating_sub(1))
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn nearest_coordinate_stays_inside_nonempty_source() {
        let dest = u32::from(kani::any::<u16>());
        let source_size = u32::from(kani::any::<u16>());
        let dest_size = u32::from(kani::any::<u16>());
        kani::assume(source_size > 0 && dest_size > 0 && dest < dest_size);
        assert!(nearest_source_coordinate(dest, source_size, dest_size) < source_size);
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;
    use crate::media_contract::{CleanAperture, Rotation};

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
    fn row_composition_matches_golden_image() {
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

        assert_eq!(
            frame_as_ascii_ppm(&output),
            include_str!("../tests/golden/compositor-row.ppm")
        );
    }

    #[test]
    fn bilinear_filter_smooths_an_upscaled_edge() {
        let compositor = Compositor::new(VideoFormat {
            width: 4,
            height: 2,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        })
        .with_scaling_filter(ScalingFilter::Bilinear);
        let edge = Frame::new_checked(
            2,
            1,
            PixelFormat::Bgra8,
            vec![0, 0, 0, 255, 255, 255, 255, 255],
        )
        .unwrap();

        let output = compositor
            .compose(&[Some(edge)], CompositionLayout::Grid)
            .unwrap();

        assert_eq!(compositor.scaling_filter(), ScalingFilter::Bilinear);
        assert_eq!(output.bgra_at(0, 0), Some([0, 0, 0, 255]));
        assert_eq!(output.bgra_at(1, 0), Some([64, 64, 64, 255]));
        assert_eq!(output.bgra_at(2, 0), Some([191, 191, 191, 255]));
        assert_eq!(output.bgra_at(3, 0), Some([255, 255, 255, 255]));
    }

    #[test]
    fn compose_into_reuses_output_storage_and_sampling_map() {
        let compositor = Compositor::new(VideoFormat {
            width: 8,
            height: 4,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let input = Frame::solid_bgra(2, 2, [3, 4, 5, 255]).unwrap();
        let frames = [Some(input)];
        let mut output = Frame::solid_bgra(8, 4, [0, 0, 0, 255]).unwrap();
        let storage = output.data().as_ptr();

        compositor
            .compose_into(&frames, CompositionLayout::Grid, &mut output)
            .unwrap();
        compositor
            .compose_into(&frames, CompositionLayout::Grid, &mut output)
            .unwrap();

        assert_eq!(storage, output.data().as_ptr());
        assert_eq!(compositor.sampling_cache.lock().unwrap().maps.len(), 1);
    }

    #[test]
    fn incomplete_grid_clears_the_uncovered_recycled_region() {
        let compositor = Compositor::new(VideoFormat {
            width: 6,
            height: 4,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let frames = vec![None; 5];
        let mut output = Frame::solid_bgra(6, 4, [99, 88, 77, 255]).unwrap();

        compositor
            .compose_into(&frames, CompositionLayout::Grid, &mut output)
            .unwrap();

        assert_eq!(output.bgra_at(0, 0), Some([32, 32, 32, 255]));
        assert_eq!(output.bgra_at(5, 3), Some([0, 0, 0, 255]));
    }

    #[test]
    fn composes_five_seven_and_eight_sources() {
        for count in [5_usize, 7, 8] {
            let colors = (0..count)
                .map(|index| [index as u8 + 1, 100, 200, 255])
                .collect::<Vec<_>>();
            let frames = colors
                .iter()
                .map(|color| Some(Frame::solid_bgra(1, 1, *color).unwrap()))
                .collect::<Vec<_>>();
            let compositor = Compositor::new(VideoFormat {
                width: 64,
                height: 64,
                fps: 30,
                pixel_format: PixelFormat::Bgra8,
            });

            let output = compositor
                .compose(&frames, CompositionLayout::Grid)
                .unwrap();
            let cells = GridLayoutCalculator::cells(64, 64, count, CompositionLayout::Grid);

            for (cell, color) in cells.iter().zip(colors) {
                assert_eq!(
                    output.bgra_at(cell.x + cell.width / 2, cell.y + cell.height / 2),
                    Some(color)
                );
            }
        }
    }

    #[test]
    fn picture_in_picture_draws_primary_then_overlay() {
        let compositor = Compositor::new(VideoFormat {
            width: 160,
            height: 90,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let primary = Frame::solid_bgra(16, 9, [10, 20, 30, 255]).unwrap();
        let overlay = Frame::solid_bgra(16, 9, [40, 50, 60, 255]).unwrap();

        let output = compositor
            .compose(
                &[Some(primary), Some(overlay)],
                CompositionLayout::PictureInPicture,
            )
            .unwrap();
        let overlay_cell =
            GridLayoutCalculator::cells(160, 90, 2, CompositionLayout::PictureInPicture)[1];

        assert_eq!(output.bgra_at(0, 0), Some([10, 20, 30, 255]));
        assert_eq!(
            output.bgra_at(
                overlay_cell.x + overlay_cell.width / 2,
                overlay_cell.y + overlay_cell.height / 2,
            ),
            Some([40, 50, 60, 255])
        );
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

    #[test]
    fn opaque_fill_fully_replaces_recycled_cell_pixels() {
        let compositor = Compositor::new(VideoFormat {
            width: 4,
            height: 4,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let input = Frame::solid_bgra(4, 2, [1, 2, 3, 255]).unwrap();
        let frames = [Some(input)];
        let transforms = [SourceTransform {
            fit: SourceFit::Fill,
            ..SourceTransform::default()
        }];
        let mut recycled = Frame::solid_bgra(4, 4, [99, 88, 77, 66]).unwrap();
        compositor
            .compose_into_with_transforms(
                &frames,
                &transforms,
                CompositionLayout::Grid,
                &mut recycled,
            )
            .unwrap();

        assert!(
            recycled
                .data()
                .chunks_exact(4)
                .all(|pixel| pixel == [1, 2, 3, 255])
        );
    }

    #[test]
    fn applies_rotation_metadata_once_during_composition() {
        let compositor = Compositor::new(VideoFormat {
            width: 1,
            height: 2,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let mut input =
            Frame::new_checked(2, 1, PixelFormat::Bgra8, vec![1, 2, 3, 255, 4, 5, 6, 255]).unwrap();
        let mut contract = input.contract();
        contract.transform.rotation = Rotation::Degrees90;
        input.set_contract(contract).unwrap();

        let output = compositor
            .compose(&[Some(input)], CompositionLayout::Grid)
            .unwrap();

        assert_eq!(output.bgra_at(0, 0), Some([1, 2, 3, 255]));
        assert_eq!(output.bgra_at(0, 1), Some([4, 5, 6, 255]));
        assert!(
            output
                .contract()
                .is_canonical_bgra(output.width(), output.height())
        );
    }

    #[test]
    fn clean_aperture_crops_without_rewriting_capture_bytes() {
        let compositor = Compositor::new(VideoFormat {
            width: 1,
            height: 1,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let mut input = Frame::new_checked(
            3,
            1,
            PixelFormat::Bgra8,
            vec![1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255],
        )
        .unwrap();
        let mut contract = input.contract();
        contract.clean_aperture = CleanAperture {
            x: 1,
            y: 0,
            width: 1,
            height: 1,
        };
        input.set_contract(contract).unwrap();

        let output = compositor
            .compose(&[Some(input)], CompositionLayout::Grid)
            .unwrap();

        assert_eq!(output.bgra_at(0, 0), Some([2, 0, 0, 255]));
    }

    #[test]
    fn opaque_transform_fast_path_matches_generic_reference() {
        let mut pixels = Vec::new();
        for y in 0..5_u8 {
            for x in 0..7_u8 {
                pixels.extend_from_slice(&[
                    x.wrapping_mul(31).wrapping_add(y * 3),
                    y.wrapping_mul(43).wrapping_add(x * 5),
                    x.wrapping_mul(11).wrapping_add(y * 17),
                    255,
                ]);
            }
        }
        let input = Frame::new_checked(7, 5, PixelFormat::Bgra8, pixels).unwrap();
        let cell = Cell {
            x: 0,
            y: 0,
            width: 17,
            height: 11,
        };
        for filter in [ScalingFilter::Nearest, ScalingFilter::Bilinear] {
            for fit in [SourceFit::Fit, SourceFit::Fill] {
                for rotation in [
                    Rotation::Degrees0,
                    Rotation::Degrees90,
                    Rotation::Degrees180,
                    Rotation::Degrees270,
                ] {
                    for mirror_horizontal in [false, true] {
                        for mirror_vertical in [false, true] {
                            for opacity_per_mille in [TRANSFORM_SCALE, 731] {
                                for cropped in [false, true] {
                                    for anchored_at_end in [false, true] {
                                        let transform = SourceTransform {
                                            crop: if cropped {
                                                crate::source_transform::CropInsets {
                                                    left_per_mille: 100,
                                                    top_per_mille: 200,
                                                    right_per_mille: 100,
                                                    bottom_per_mille: 0,
                                                }
                                            } else {
                                                crate::source_transform::CropInsets::default()
                                            },
                                            rotation,
                                            mirror_horizontal,
                                            mirror_vertical,
                                            fit,
                                            position_x_per_mille: if anchored_at_end {
                                                1_000
                                            } else {
                                                0
                                            },
                                            position_y_per_mille: if anchored_at_end {
                                                0
                                            } else {
                                                1_000
                                            },
                                            opacity_per_mille,
                                        };
                                        let contract = transform
                                            .apply_to_contract(
                                                input.contract(),
                                                input.width(),
                                                input.height(),
                                            )
                                            .unwrap();
                                        let placement =
                                            transformed_placement(contract, cell, transform);
                                        let mut generic =
                                            Frame::solid_bgra(17, 11, [32, 32, 32, 255]).unwrap();
                                        paste_contract_transformed_generic(
                                            &mut generic,
                                            &input,
                                            filter,
                                            contract,
                                            transform.opacity_per_mille,
                                            placement,
                                        );

                                        let mut fast =
                                            Frame::solid_bgra(17, 11, [32, 32, 32, 255]).unwrap();
                                        paste_opaque_contract(
                                            &mut fast,
                                            &input,
                                            filter,
                                            &Arc::new(Mutex::new(SamplingCache::default())),
                                            contract,
                                            transform.opacity_per_mille,
                                            placement,
                                        );

                                        assert_eq!(
                                            fast.data(),
                                            generic.data(),
                                            "mismatch for {filter:?} with {transform:?}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn straight_alpha_is_premultiplied_before_blending() {
        let compositor = Compositor::new(VideoFormat {
            width: 1,
            height: 1,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let mut input = Frame::new_checked(1, 1, PixelFormat::Bgra8, vec![200, 0, 0, 128]).unwrap();
        let mut contract = input.contract();
        contract.colorimetry.alpha = AlphaMode::Straight;
        input.set_contract(contract).unwrap();

        let output = compositor
            .compose(&[Some(input)], CompositionLayout::Grid)
            .unwrap();

        assert_eq!(output.bgra_at(0, 0), Some([116, 16, 16, 255]));
    }

    fn frame_as_ascii_ppm(frame: &Frame) -> String {
        let mut output = format!("P3\n{} {}\n255\n", frame.width(), frame.height());
        for y in 0..frame.height() {
            for x in 0..frame.width() {
                if x > 0 {
                    output.push(' ');
                }
                let [blue, green, red, _] = frame.bgra_at(x, y).unwrap();
                write!(output, "{red} {green} {blue}").unwrap();
            }
            output.push('\n');
        }
        output
    }
}
