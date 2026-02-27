//! Audio output via cpal — drains the AudioPlayer ring buffer to speakers.

use std::sync::Arc;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use hadal_audio::AudioPlayer;

/// Info about the default audio output device.
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub device_name: String,
}

/// An active audio output stream.
///
/// The stream runs in a background thread managed by cpal.
/// It calls `player.read_samples()` whenever the OS needs audio data.
/// Drop this to stop audio output.
pub struct OutputStream {
    _stream: cpal::Stream,
}

/// Probe the default audio output device and return its configuration.
///
/// Call this before creating the AudioPlayer so the pipeline can be
/// configured with the correct output sample rate.
pub fn probe_device() -> Result<DeviceConfig> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("No audio output device found")?;

    let supported = device
        .default_output_config()
        .context("No supported output config")?;

    let name = device.name().unwrap_or_else(|_| "unknown".into());

    tracing::info!(
        "Audio device: {} ({} ch, {}Hz, {:?})",
        name,
        supported.channels(),
        supported.sample_rate().0,
        supported.sample_format(),
    );

    Ok(DeviceConfig {
        sample_rate: supported.sample_rate().0,
        channels: supported.channels(),
        device_name: name,
    })
}

/// Set up a cpal output stream that drains samples from the player.
///
/// The player is shared via `Arc<AudioPlayer>`. The output callback only
/// contends on the inner pipeline mutex — no outer lock.
pub fn start(player: Arc<AudioPlayer>, device_config: &DeviceConfig) -> Result<OutputStream> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("No audio output device found")?;

    let config = cpal::StreamConfig {
        channels: device_config.channels,
        sample_rate: cpal::SampleRate(device_config.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                player.read_samples(data);
            },
            |err| {
                tracing::error!("Audio output error: {}", err);
            },
            None,
        )
        .context("Failed to build output stream")?;

    stream.play().context("Failed to start audio stream")?;

    Ok(OutputStream { _stream: stream })
}
