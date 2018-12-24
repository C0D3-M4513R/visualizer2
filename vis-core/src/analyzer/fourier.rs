use super::Sample;
use crate::analyzer;

pub mod window {
    pub fn blackman(size: usize) -> Vec<f32> {
        apodize::blackman_iter(size).map(|f| f as f32).collect()
    }

    pub fn hamming(size: usize) -> Vec<f32> {
        apodize::hamming_iter(size).map(|f| f as f32).collect()
    }

    pub fn hanning(size: usize) -> Vec<f32> {
        apodize::hanning_iter(size).map(|f| f as f32).collect()
    }

    pub fn none(size: usize) -> Vec<f32> {
        vec![1.0; size]
    }

    pub fn nuttall(size: usize) -> Vec<f32> {
        apodize::nuttall_iter(size).map(|f| f as f32).collect()
    }

    pub fn sine(size: usize) -> Vec<f32> {
        (0..size)
            .map(|i| (i as f32 / (size - 1) as f32 * std::f32::consts::PI).sin())
            .collect()
    }

    pub fn triangular(size: usize) -> Vec<f32> {
        apodize::triangular_iter(size).map(|f| f as f32).collect()
    }

    pub fn from_str(name: &str) -> Option<fn(usize) -> Vec<f32>> {
        match name {
            "blackman" => Some(blackman),
            "hamming" => Some(hamming),
            "hanning" => Some(hanning),
            "none" => Some(none),
            "nuttall" => Some(nuttall),
            "sine" => Some(sine),
            "triangular" => Some(triangular),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct FourierBuilder {
    pub length: Option<usize>,
    pub window: Option<fn(usize) -> Vec<f32>>,
    pub downsample: Option<usize>,
}

impl FourierBuilder {
    pub fn new() -> FourierBuilder {
        Default::default()
    }

    pub fn length(&mut self, length: usize) -> &mut FourierBuilder {
        self.length = Some(length);
        self
    }

    pub fn window(&mut self, f: fn(usize) -> Vec<f32>) -> &mut FourierBuilder {
        self.window = Some(f);
        self
    }

    pub fn downsample(&mut self, factor: usize) -> &mut FourierBuilder {
        self.downsample = Some(factor);
        self
    }

    pub fn plan(&mut self) -> FourierAnalyzer {
        let length = self.length.unwrap_or(1024);
        let window = (self.window.unwrap_or(window::none))(length);
        let downsample = self.downsample.unwrap_or(1);

        FourierAnalyzer::new(length, window, downsample)
    }
}

pub struct FourierAnalyzer {
    length: usize,
    window: Vec<Sample>,
    downsample: usize,

    fft: std::sync::Arc<rustfft::FFT<Sample>>,

    input: [Vec<rustfft::num_complex::Complex<Sample>>; 2],
    output: Vec<rustfft::num_complex::Complex<Sample>>,
}

impl FourierAnalyzer {
    fn new(length: usize, window: Vec<f32>, downsample: usize) -> FourierAnalyzer {
        use rustfft::num_traits::Zero;

        let fft = rustfft::FFTplanner::new(false).plan_fft(length);

        FourierAnalyzer {
            length,
            window,
            downsample,

            fft,

            input: [Vec::with_capacity(length), Vec::with_capacity(length)],
            output: vec![rustfft::num_complex::Complex::zero(); length],
        }
    }

    pub fn analyze(&mut self, buf: &analyzer::SampleBuffer) {
        // Copy samples to left and right buffer
        self.input[0].clear();
        self.input[1].clear();
        for ([l, r], window) in buf
            .iter(self.length, self.downsample)
            .zip(self.window.iter())
        {
            self.input[0].push(rustfft::num_complex::Complex::new(l * window, 0.0));
            self.input[1].push(rustfft::num_complex::Complex::new(r * window, 0.0));
        }

        debug_assert_eq!(self.input[0].len(), self.window.len());
        debug_assert_eq!(self.input[1].len(), self.window.len());

        self.fft.process(&mut self.input[0], &mut self.output);
        self.fft.process(&mut self.input[1], &mut self.output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let analyzer = FourierBuilder::new()
            .length(512)
            .window(window::from_str("nuttall").unwrap())
            .downsample(8)
            .plan();
    }

    #[test]
    fn test_analyze() {
        let mut analyzer = FourierBuilder::new()
            .length(512)
            .window(window::from_str("nuttall").unwrap())
            .downsample(2)
            .plan();

        let buf = crate::analyzer::SampleBuffer::new(1024, 8000);

        buf.push(&[[1.0; 2]; 1024]);

        analyzer.analyze(&buf);
    }
}
