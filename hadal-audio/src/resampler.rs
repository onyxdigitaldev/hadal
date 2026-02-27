//! High-quality audio resampling using Rubato.

use rubato::{
    FastFixedIn, PolynomialDegree, Resampler as RubatoResampler, SincFixedIn,
    SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use crate::error::{AudioError, AudioResult};

/// Resampling quality presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResamplerQuality {
    /// Fast linear interpolation (lowest quality, lowest CPU)
    Fast,

    /// Medium quality sinc resampling
    #[default]
    Medium,

    /// High quality sinc resampling (audiophile mode)
    Best,
}

/// Audio resampler for sample rate conversion.
///
/// Accumulates decoded frames in a per-channel staging buffer and feeds the
/// rubato resampler in exact `chunk_size`-frame slices.  Leftover frames
/// carry over to the next `process()` call so no data is dropped or
/// zero-padded mid-stream.
pub struct Resampler {
    inner: ResamplerImpl,

    input_rate: u32,
    output_rate: u32,
    channels: usize,
    chunk_size: usize,

    /// Per-channel accumulation buffer (deinterleaved).
    /// Grows as decoder chunks arrive; drained in chunk_size increments.
    accum: Vec<Vec<f32>>,

    /// Scratch space for one resampler output call (deinterleaved).
    output_buffer: Vec<Vec<f32>>,
}

enum ResamplerImpl {
    Fast(FastFixedIn<f32>),
    Sinc(SincFixedIn<f32>),
    Passthrough,
}

impl Resampler {
    /// Create a new resampler.
    pub fn new(
        input_rate: u32,
        output_rate: u32,
        channels: usize,
        quality: ResamplerQuality,
        chunk_size: usize,
    ) -> AudioResult<Self> {
        if input_rate == output_rate {
            return Ok(Self {
                inner: ResamplerImpl::Passthrough,
                input_rate,
                output_rate,
                channels,
                chunk_size,
                accum: vec![Vec::new(); channels],
                output_buffer: vec![Vec::new(); channels],
            });
        }

        let resample_ratio = output_rate as f64 / input_rate as f64;

        let inner = match quality {
            ResamplerQuality::Fast => {
                let r = FastFixedIn::new(
                    resample_ratio,
                    1.0,
                    PolynomialDegree::Cubic,
                    chunk_size,
                    channels,
                )
                .map_err(|e| AudioError::Resampler(e.to_string()))?;
                ResamplerImpl::Fast(r)
            }
            ResamplerQuality::Medium => {
                let params = SincInterpolationParameters {
                    sinc_len: 128,
                    f_cutoff: 0.925,
                    interpolation: SincInterpolationType::Linear,
                    oversampling_factor: 128,
                    window: WindowFunction::BlackmanHarris2,
                };
                let r = SincFixedIn::new(resample_ratio, 1.0, params, chunk_size, channels)
                    .map_err(|e| AudioError::Resampler(e.to_string()))?;
                ResamplerImpl::Sinc(r)
            }
            ResamplerQuality::Best => {
                let params = SincInterpolationParameters {
                    sinc_len: 256,
                    f_cutoff: 0.95,
                    interpolation: SincInterpolationType::Cubic,
                    oversampling_factor: 256,
                    window: WindowFunction::BlackmanHarris2,
                };
                let r = SincFixedIn::new(resample_ratio, 1.0, params, chunk_size, channels)
                    .map_err(|e| AudioError::Resampler(e.to_string()))?;
                ResamplerImpl::Sinc(r)
            }
        };

        let output_chunk = (chunk_size as f64 * resample_ratio).ceil() as usize + 16;

        Ok(Self {
            inner,
            input_rate,
            output_rate,
            channels,
            chunk_size,
            accum: vec![Vec::with_capacity(chunk_size * 2); channels],
            output_buffer: vec![vec![0.0; output_chunk]; channels],
        })
    }

    /// Resample interleaved audio data.
    ///
    /// Accepts a variable-length interleaved input buffer.  Internally
    /// accumulates frames and feeds the resampler in exact `chunk_size`
    /// slices.  Returns all resampled output produced so far (interleaved).
    /// Leftover input frames are retained for the next call.
    pub fn process(&mut self, input: &[f32]) -> AudioResult<Vec<f32>> {
        if matches!(self.inner, ResamplerImpl::Passthrough) {
            return Ok(input.to_vec());
        }

        let channels = self.channels;
        let frames = input.len() / channels;

        // Deinterleave into accumulation buffer
        for (i, sample) in input.iter().enumerate() {
            self.accum[i % channels].push(*sample);
        }

        let mut all_output: Vec<f32> = Vec::with_capacity(
            ((frames as f64 * self.output_rate as f64 / self.input_rate as f64) as usize + 64)
                * channels,
        );

        // Process as many full chunks as we have accumulated
        while self.accum[0].len() >= self.chunk_size {
            let out_len = self.resample_one_chunk()?;
            // Interleave into output
            for frame in 0..out_len {
                for ch in 0..channels {
                    all_output.push(self.output_buffer[ch][frame]);
                }
            }
        }

        Ok(all_output)
    }

    /// Feed exactly `chunk_size` frames from the front of `accum` into the
    /// resampler.  Drains consumed frames from `accum`.  Returns the number
    /// of output frames written to `self.output_buffer`.
    fn resample_one_chunk(&mut self) -> AudioResult<usize> {
        let cs = self.chunk_size;

        // Build input slice refs (first chunk_size frames of each channel)
        let input_refs: Vec<&[f32]> = self.accum.iter().map(|ch| &ch[..cs]).collect();

        // Prepare output buffer
        let output_frames = match &self.inner {
            ResamplerImpl::Fast(r) => r.output_frames_next(),
            ResamplerImpl::Sinc(r) => r.output_frames_next(),
            ResamplerImpl::Passthrough => unreachable!(),
        };
        for ch in self.output_buffer.iter_mut() {
            ch.resize(output_frames, 0.0);
        }
        let mut output_refs: Vec<&mut [f32]> =
            self.output_buffer.iter_mut().map(|v| v.as_mut_slice()).collect();

        let (_, out_len) = match &mut self.inner {
            ResamplerImpl::Fast(r) => r
                .process_into_buffer(&input_refs, &mut output_refs, None)
                .map_err(|e| AudioError::Resampler(e.to_string()))?,
            ResamplerImpl::Sinc(r) => r
                .process_into_buffer(&input_refs, &mut output_refs, None)
                .map_err(|e| AudioError::Resampler(e.to_string()))?,
            ResamplerImpl::Passthrough => unreachable!(),
        };

        // Drain consumed frames from accumulation buffer
        for ch in self.accum.iter_mut() {
            ch.drain(..cs);
        }

        Ok(out_len)
    }

    /// Flush any remaining accumulated frames through the resampler.
    ///
    /// Call this at end-of-stream to get the final partial chunk out.
    /// Pads with zeros to fill the last chunk.
    pub fn flush(&mut self) -> AudioResult<Vec<f32>> {
        if matches!(self.inner, ResamplerImpl::Passthrough) {
            return Ok(Vec::new());
        }

        let remaining = self.accum[0].len();
        if remaining == 0 {
            return Ok(Vec::new());
        }

        // Pad to chunk_size with zeros
        let pad = self.chunk_size - remaining;
        for ch in self.accum.iter_mut() {
            ch.extend(std::iter::repeat(0.0).take(pad));
        }

        let out_len = self.resample_one_chunk()?;

        // Only return the non-padded portion of output
        // (approximate: scale by the fraction of real input)
        let real_out_frames =
            (remaining as f64 * self.output_rate as f64 / self.input_rate as f64).ceil() as usize;
        let out_frames = real_out_frames.min(out_len);

        let channels = self.channels;
        let mut output = Vec::with_capacity(out_frames * channels);
        for frame in 0..out_frames {
            for ch in 0..channels {
                output.push(self.output_buffer[ch][frame]);
            }
        }

        Ok(output)
    }

    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }

    pub fn is_passthrough(&self) -> bool {
        matches!(self.inner, ResamplerImpl::Passthrough)
    }

    pub fn latency_samples(&self) -> usize {
        match &self.inner {
            ResamplerImpl::Passthrough => 0,
            ResamplerImpl::Fast(r) => r.output_delay(),
            ResamplerImpl::Sinc(r) => r.output_delay(),
        }
    }

    /// Clear any accumulated input data (call on track change / seek).
    pub fn reset(&mut self) {
        for ch in self.accum.iter_mut() {
            ch.clear();
        }
    }
}

impl std::fmt::Debug for Resampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resampler")
            .field("input_rate", &self.input_rate)
            .field("output_rate", &self.output_rate)
            .field("channels", &self.channels)
            .field("chunk_size", &self.chunk_size)
            .field("accumulated_frames", &self.accum.first().map(|v| v.len()).unwrap_or(0))
            .field("is_passthrough", &self.is_passthrough())
            .finish()
    }
}
