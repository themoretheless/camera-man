use serde::{Deserialize, Serialize};

use crate::error::CameraManError;
use crate::frame::{Frame, PixelFormat};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub mean_squared_error: f64,
    pub psnr_db: f64,
    pub ssim: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QualityGate {
    pub minimum_ssim: f64,
    pub minimum_psnr_db: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QualityConfidenceInterval {
    pub confidence_level: f64,
    pub estimate: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QualitySummary {
    pub sample_count: usize,
    pub ssim: QualityConfidenceInterval,
    pub psnr_db: QualityConfidenceInterval,
}

impl QualityGate {
    pub const CONTROLLED_FIXTURE: Self = Self {
        minimum_ssim: 0.995,
        minimum_psnr_db: 40.0,
    };

    pub fn accepts(self, metrics: QualityMetrics) -> bool {
        metrics.ssim >= self.minimum_ssim && metrics.psnr_db >= self.minimum_psnr_db
    }
}

/// Computes RGB MSE, PSNR and a global SSIM score for controlled fixtures.
/// Production acceptance uses stable fixtures; VMAF remains an offline video
/// tool because it is not a per-frame runtime dependency.
pub fn compare_frames(
    reference: &Frame,
    candidate: &Frame,
) -> Result<QualityMetrics, CameraManError> {
    if reference.width() != candidate.width() || reference.height() != candidate.height() {
        return Err(CameraManError::InvalidDimensions {
            width: candidate.width(),
            height: candidate.height(),
        });
    }
    if reference.pixel_format() != PixelFormat::Bgra8
        || candidate.pixel_format() != PixelFormat::Bgra8
    {
        return Err(CameraManError::UnsupportedPixelFormat);
    }

    let mut squared_error = 0.0;
    let mut reference_luma = Vec::with_capacity(reference.data().len() / 4);
    let mut candidate_luma = Vec::with_capacity(reference_luma.capacity());
    let mut channel_samples = 0_u64;
    for (reference_pixel, candidate_pixel) in reference
        .data()
        .chunks_exact(4)
        .zip(candidate.data().chunks_exact(4))
    {
        for channel in 0..3 {
            let difference =
                f64::from(reference_pixel[channel]) - f64::from(candidate_pixel[channel]);
            squared_error += difference * difference;
            channel_samples += 1;
        }
        reference_luma.push(luma(reference_pixel));
        candidate_luma.push(luma(candidate_pixel));
    }
    let mean_squared_error = squared_error / channel_samples.max(1) as f64;
    let psnr_db = psnr_from_mse(mean_squared_error);

    Ok(QualityMetrics {
        mean_squared_error,
        psnr_db,
        ssim: global_ssim(&reference_luma, &candidate_luma),
    })
}

/// Deterministic percentile-bootstrap intervals over representative frames.
/// The seed and iteration count belong in the persisted quality report.
pub fn summarize_quality(
    samples: &[QualityMetrics],
    bootstrap_iterations: usize,
    seed: u64,
) -> Result<QualitySummary, CameraManError> {
    if samples.is_empty() {
        return Err(CameraManError::EmptyInput);
    }
    if bootstrap_iterations < 100 {
        return Err(CameraManError::InvalidMediaContract(
            "quality bootstrap requires at least 100 iterations",
        ));
    }
    if samples.iter().any(|sample| {
        !sample.ssim.is_finite()
            || !sample.mean_squared_error.is_finite()
            || sample.mean_squared_error < 0.0
            || sample.psnr_db.is_nan()
    }) {
        return Err(CameraManError::InvalidMediaContract(
            "quality samples contain a non-finite score",
        ));
    }
    let ssim = samples.iter().map(|sample| sample.ssim).collect::<Vec<_>>();
    let mean_squared_error = samples
        .iter()
        .map(|sample| sample.mean_squared_error)
        .collect::<Vec<_>>();
    let mut random = XorShift64::new(seed);
    Ok(QualitySummary {
        sample_count: samples.len(),
        ssim: bootstrap_mean_interval(&ssim, bootstrap_iterations, &mut random),
        psnr_db: bootstrap_psnr_interval(&mean_squared_error, bootstrap_iterations, &mut random),
    })
}

/// Odd-sized deterministic camera-like fixture containing text glyphs,
/// one-pixel edges, diagonals, gradients, skin-like tones and chroma detail.
pub fn representative_quality_fixture(width: u32, height: u32) -> Result<Frame, CameraManError> {
    if width < 64 || height < 48 {
        return Err(CameraManError::InvalidDimensions { width, height });
    }
    let mut frame = Frame::solid_bgra(width, height, [0, 0, 0, 255])?;
    let pixels = frame.data_mut();
    for y in 0..height {
        for x in 0..width {
            let offset = (y * width + x) as usize * 4;
            let mut pixel = [
                (u64::from(x) * 255 / u64::from(width - 1)) as u8,
                (u64::from(y) * 255 / u64::from(height - 1)) as u8,
                (u64::from(x + y) * 255 / u64::from(width + height - 2)) as u8,
                255,
            ];
            if x < width / 4 && y < height / 2 {
                let value = if x.is_multiple_of(2) { 245 } else { 18 };
                pixel = [value, value, value, 255];
            } else if x >= width * 3 / 4 && y < height / 2 {
                pixel = if (x / 2 + y / 2).is_multiple_of(2) {
                    [230, 28, 218, 255]
                } else {
                    [26, 218, 32, 255]
                };
            } else if x < width / 2 && y >= height * 2 / 3 {
                let variation = ((x + y) % 17) as u8;
                pixel = [92 + variation / 2, 132 + variation, 184 + variation, 255];
            }
            if (x + y).is_multiple_of(19) || x.abs_diff(y) % 23 == 0 {
                pixel = [250, 250, 250, 255];
            }
            if x == 3 || y == 5 {
                pixel = [210, 32, 230, 255];
            }
            pixels[offset..offset + 4].copy_from_slice(&pixel);
        }
    }
    draw_fixture_text(pixels, width, height, 8, height - 12);
    Ok(frame)
}

fn bootstrap_mean_interval(
    values: &[f64],
    iterations: usize,
    random: &mut XorShift64,
) -> QualityConfidenceInterval {
    let estimate = mean(values);
    if values.iter().all(|value| value.is_infinite()) {
        return QualityConfidenceInterval {
            confidence_level: 0.95,
            estimate,
            lower_bound: estimate,
            upper_bound: estimate,
        };
    }
    let mut means = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let total = (0..values.len())
            .map(|_| values[random.index(values.len())])
            .sum::<f64>();
        means.push(total / values.len() as f64);
    }
    means.sort_by(|left, right| left.total_cmp(right));
    let last = means.len() - 1;
    QualityConfidenceInterval {
        confidence_level: 0.95,
        estimate,
        lower_bound: means[last * 25 / 1_000],
        upper_bound: means[last * 975 / 1_000],
    }
}

fn bootstrap_psnr_interval(
    mean_squared_errors: &[f64],
    iterations: usize,
    random: &mut XorShift64,
) -> QualityConfidenceInterval {
    let estimate = psnr_from_mse(mean(mean_squared_errors));
    let mut scores = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let total = (0..mean_squared_errors.len())
            .map(|_| mean_squared_errors[random.index(mean_squared_errors.len())])
            .sum::<f64>();
        scores.push(psnr_from_mse(total / mean_squared_errors.len() as f64));
    }
    scores.sort_by(|left, right| left.total_cmp(right));
    let last = scores.len() - 1;
    QualityConfidenceInterval {
        confidence_level: 0.95,
        estimate,
        lower_bound: scores[last * 25 / 1_000],
        upper_bound: scores[last * 975 / 1_000],
    }
}

fn psnr_from_mse(mean_squared_error: f64) -> f64 {
    if mean_squared_error == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0_f64 * 255.0 / mean_squared_error).log10()
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

struct XorShift64(u64);

impl XorShift64 {
    const fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn index(&mut self, upper_bound: usize) -> usize {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value as usize % upper_bound
    }
}

fn draw_fixture_text(pixels: &mut [u8], width: u32, height: u32, x: u32, baseline: u32) {
    const GLYPHS: [[[u8; 5]; 7]; 3] = [
        [
            *b"11110", *b"10000", *b"10000", *b"10000", *b"10000", *b"10000", *b"11110",
        ],
        [
            *b"01110", *b"10001", *b"10001", *b"11111", *b"10001", *b"10001", *b"10001",
        ],
        [
            *b"10001", *b"11011", *b"10101", *b"10101", *b"10001", *b"10001", *b"10001",
        ],
    ];
    for (glyph_index, glyph) in GLYPHS.iter().enumerate() {
        for (row, bits) in glyph.iter().enumerate() {
            for (column, bit) in bits.iter().enumerate() {
                if *bit != b'1' {
                    continue;
                }
                let pixel_x = x + glyph_index as u32 * 7 + column as u32;
                let pixel_y = baseline.saturating_sub(7) + row as u32;
                if pixel_x < width && pixel_y < height {
                    let offset = (pixel_y * width + pixel_x) as usize * 4;
                    pixels[offset..offset + 4].copy_from_slice(&[8, 8, 8, 255]);
                }
            }
        }
    }
}

/// Reference implementation for quality experiments. It decodes BT.709 RGB
/// to linear light, interpolates there and applies the BT.709 OETF again.
#[cfg(feature = "linear-light-experiment")]
pub fn resize_bilinear_linear_light(
    input: &Frame,
    width: u32,
    height: u32,
) -> Result<Frame, CameraManError> {
    if input.pixel_format() != PixelFormat::Bgra8 {
        return Err(CameraManError::UnsupportedPixelFormat);
    }
    if width == 0 || height == 0 {
        return Err(CameraManError::InvalidDimensions { width, height });
    }
    let mut output = Frame::solid_bgra(width, height, [0, 0, 0, 255])?;
    let pixels = output.data_mut();
    for y in 0..height {
        let (low_y, high_y, weight_y) = linear_coordinates(y, input.height(), height);
        for x in 0..width {
            let (low_x, high_x, weight_x) = linear_coordinates(x, input.width(), width);
            let offsets = [
                (low_y * input.width() + low_x) as usize * 4,
                (low_y * input.width() + high_x) as usize * 4,
                (high_y * input.width() + low_x) as usize * 4,
                (high_y * input.width() + high_x) as usize * 4,
            ];
            let weights = [
                (1.0 - weight_x) * (1.0 - weight_y),
                weight_x * (1.0 - weight_y),
                (1.0 - weight_x) * weight_y,
                weight_x * weight_y,
            ];
            let destination = (y * width + x) as usize * 4;
            for channel in 0..3 {
                let linear = offsets
                    .iter()
                    .zip(weights)
                    .map(|(offset, weight)| {
                        bt709_to_linear(input.data()[offset + channel]) * weight
                    })
                    .sum::<f64>();
                pixels[destination + channel] = linear_to_bt709(linear);
            }
            pixels[destination + 3] = 255;
        }
    }
    Ok(output)
}

fn luma(pixel: &[u8]) -> f64 {
    0.0722 * f64::from(pixel[0]) + 0.7152 * f64::from(pixel[1]) + 0.2126 * f64::from(pixel[2])
}

fn global_ssim(reference: &[f64], candidate: &[f64]) -> f64 {
    let count = reference.len().max(1) as f64;
    let reference_mean = reference.iter().sum::<f64>() / count;
    let candidate_mean = candidate.iter().sum::<f64>() / count;
    let mut reference_variance = 0.0;
    let mut candidate_variance = 0.0;
    let mut covariance = 0.0;
    for (reference, candidate) in reference.iter().zip(candidate) {
        reference_variance += (reference - reference_mean).powi(2);
        candidate_variance += (candidate - candidate_mean).powi(2);
        covariance += (reference - reference_mean) * (candidate - candidate_mean);
    }
    let denominator = (reference.len().saturating_sub(1)).max(1) as f64;
    reference_variance /= denominator;
    candidate_variance /= denominator;
    covariance /= denominator;
    let c1 = (0.01_f64 * 255.0).powi(2);
    let c2 = (0.03_f64 * 255.0).powi(2);
    ((2.0 * reference_mean * candidate_mean + c1) * (2.0 * covariance + c2))
        / ((reference_mean.powi(2) + candidate_mean.powi(2) + c1)
            * (reference_variance + candidate_variance + c2))
}

#[cfg(feature = "linear-light-experiment")]
fn linear_coordinates(
    destination: u32,
    source_size: u32,
    destination_size: u32,
) -> (u32, u32, f64) {
    let position = ((f64::from(destination) + 0.5) * f64::from(source_size)
        / f64::from(destination_size)
        - 0.5)
        .clamp(0.0, f64::from(source_size - 1));
    let low = position.floor() as u32;
    let high = (low + 1).min(source_size - 1);
    (low, high, position - f64::from(low))
}

#[cfg(feature = "linear-light-experiment")]
fn bt709_to_linear(encoded: u8) -> f64 {
    let value = f64::from(encoded) / 255.0;
    if value < 0.081 {
        value / 4.5
    } else {
        ((value + 0.099) / 1.099).powf(1.0 / 0.45)
    }
}

#[cfg(feature = "linear-light-experiment")]
fn linear_to_bt709(linear: f64) -> u8 {
    let linear = linear.clamp(0.0, 1.0);
    let encoded = if linear < 0.018 {
        4.5 * linear
    } else {
        1.099 * linear.powf(0.45) - 0.099
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "linear-light-experiment")]
    use crate::{CompositionLayout, Compositor, VideoFormat};

    #[test]
    fn exact_frames_pass_the_perceptual_gate() {
        let frame = Frame::solid_bgra(2, 2, [10, 20, 30, 255]).unwrap();
        let metrics = compare_frames(&frame, &frame).unwrap();
        assert_eq!(metrics.ssim, 1.0);
        assert!(metrics.psnr_db.is_infinite());
        assert!(QualityGate::CONTROLLED_FIXTURE.accepts(metrics));
    }

    #[cfg(feature = "linear-light-experiment")]
    #[test]
    fn linear_light_resize_is_closer_to_linear_reference_than_gamma_bilinear() {
        let input = Frame::new_checked(
            2,
            1,
            PixelFormat::Bgra8,
            vec![0, 0, 0, 255, 255, 255, 255, 255],
        )
        .unwrap();
        let reference = resize_bilinear_linear_light(&input, 1, 1).unwrap();
        let gamma = Compositor::new(VideoFormat {
            width: 1,
            height: 1,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        })
        .with_scaling_filter(crate::ScalingFilter::Bilinear)
        .compose(&[Some(input)], CompositionLayout::Grid)
        .unwrap();

        let linear_metrics = compare_frames(&reference, &reference).unwrap();
        let gamma_metrics = compare_frames(&reference, &gamma).unwrap();
        assert!(linear_metrics.ssim > gamma_metrics.ssim);
        assert_eq!(reference.bgra_at(0, 0), Some([180, 180, 180, 255]));
        assert_eq!(gamma.bgra_at(0, 0), Some([128, 128, 128, 255]));
    }

    #[test]
    fn representative_fixture_and_bootstrap_summary_are_deterministic() {
        let reference = representative_quality_fixture(127, 73).unwrap();
        assert_eq!(reference.width(), 127);
        assert_eq!(reference.height(), 73);
        assert_eq!(reference.bgra_at(3, 20), Some([210, 32, 230, 255]));

        let mut samples = Vec::new();
        for stride in 31..=38 {
            let mut candidate = reference.clone();
            for pixel in candidate.data_mut().chunks_exact_mut(4).step_by(stride) {
                pixel[0] = pixel[0].saturating_add(2);
            }
            samples.push(compare_frames(&reference, &candidate).unwrap());
        }
        let first = summarize_quality(&samples, 1_000, 0xCAFE_BABE).unwrap();
        let second = summarize_quality(&samples, 1_000, 0xCAFE_BABE).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.sample_count, 8);
        assert!(first.ssim.lower_bound <= first.ssim.upper_bound);
        assert!(first.psnr_db.lower_bound <= first.psnr_db.upper_bound);
        assert!(first.psnr_db.lower_bound <= first.psnr_db.estimate);
        assert!(first.psnr_db.estimate <= first.psnr_db.upper_bound);
    }

    #[test]
    fn psnr_summary_remains_finite_when_only_some_frames_are_exact() {
        let exact = QualityMetrics {
            mean_squared_error: 0.0,
            psnr_db: f64::INFINITY,
            ssim: 1.0,
        };
        let changed = QualityMetrics {
            mean_squared_error: 4.0,
            psnr_db: psnr_from_mse(4.0),
            ssim: 0.99,
        };

        let summary = summarize_quality(&[exact, changed], 1_000, 7).unwrap();

        assert!(summary.psnr_db.estimate.is_finite());
        assert!(summary.psnr_db.lower_bound.is_finite());
        assert!(summary.psnr_db.upper_bound.is_infinite());
    }

    #[test]
    fn representative_fixture_rejects_huge_dimensions_before_allocation() {
        assert!(representative_quality_fixture(u32::MAX, 48).is_err());
    }
}
