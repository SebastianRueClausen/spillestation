#![allow(dead_code)]

use splst_core::AudioOutput;

use thiserror::Error;
use cpal::traits::{DeviceTrait, StreamTrait, HostTrait};

use std::sync::mpsc;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("no available audio device")]
    NoDevice,
    #[error("no ouput config")]
    NoConfig,
    #[error("failed to build audio stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),
    #[error("failed to play audio stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError)
}

pub struct AudioStream {
    sender: mpsc::Sender<[i16; 2]>,
    stream: cpal::Stream,
}

impl AudioStream {
    pub fn new() -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioError::NoDevice)?;
        let config = device
            .default_output_config()
            .map_err(|_| AudioError::NoConfig)?;
        let (sender, receiver) = mpsc::channel::<[i16; 2]>();
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config.config(), receiver)?,
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config.config(), receiver)?,
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config.config(), receiver)?,
        };
        stream.play()?;
        Ok(AudioStream { sender, stream })
    }
}

fn build_stream<T: cpal::Sample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    receiver: mpsc::Receiver<[i16; 2]>,
) -> Result<cpal::Stream, AudioError> {
    let mut write_ctx = WriteContext {
        last_samples: [0, 0],
        sample_rate: config.sample_rate.0,
        channel_count: config.channels,
        receiver,
    };

    let stream = device.build_output_stream(
        &config,
        move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
            write_ctx.write_sample(output)
        },
        |err| error!("error building output audio stream: {err}"),
    )?;
    
    Ok(stream)
}

impl AudioOutput for AudioStream {
    fn send_audio(&mut self, samples: [i16; 2]) {
        self.sender.send(samples).expect("failed to send audio sampls");
    }
}

struct WriteContext {
    receiver: mpsc::Receiver<[i16; 2]>,
    last_samples: [i16; 2],
    channel_count: u16,
    sample_rate: u32,
}

impl WriteContext {
    fn write_sample<T: cpal::Sample>(&mut self, output: &mut [T]) {
        for frame in output.chunks_mut(self.channel_count as usize) {
            let samples = self.receiver.try_recv().unwrap_or_else(|_| {
                self.last_samples
            });

            self.last_samples = samples;

            for (frame, sample) in frame.iter_mut().zip(samples.iter()) {
                *frame = cpal::Sample::from(sample);
            }
        }
    }
}    
