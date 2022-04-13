extern crate anyhow;
extern crate clap;
extern crate cpal;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

type AbsoluteFrequency = f32;
type RelativeFrequency = f32;

static A_IN_HZ: AbsoluteFrequency = 440.0;

fn main() -> anyhow::Result<()> {
    let stream = stream_setup_for(sample_next)?;
    stream.play()?;
    std::thread::sleep(std::time::Duration::from_millis(3000));
    Ok(())
}

pub enum ToneLength {
    Four,
    FourDot,
    Two,
    TwoDot,
    Full,
    FullDot,
    Quarter,
    QuarterDot,
    Octet,
}

pub struct Note {
    pub pitch_relative_to_a: RelativeFrequency,
    pub length: ToneLength,
}

fn sample_next(o: &mut SampleRequestOptions) -> f32 {
    o.tick();

    o.tone()
}

pub struct SampleRequestOptions {
    pub sample_rate: f32,
    pub sample_clock: f32,
    pub nchannels: usize,

    pub note: Note,
    pub melody: Vec<Note>,
}

impl SampleRequestOptions {
    fn tone(&self) -> f32 {
        let note_number: usize =
            (self.sample_clock as i64 / 30000) as usize % self.melody.len() as usize;
        (self.sample_clock
            * self.melody[note_number].pitch_relative_to_a
            * A_IN_HZ
            * 2.0
            * std::f32::consts::PI
            / self.sample_rate)
            .sin()
    }
    fn tick(&mut self) {
        self.sample_clock = (self.sample_clock + 1.0) % self.sample_rate;
    }
}

pub fn stream_setup_for<F>(on_sample: F) -> Result<cpal::Stream, anyhow::Error>
where
    F: FnMut(&mut SampleRequestOptions) -> f32 + std::marker::Send + 'static + Copy,
{
    let (_host, device, config) = host_device_setup()?;

    match config.sample_format() {
        cpal::SampleFormat::F32 => stream_make::<f32, _>(&device, &config.into(), on_sample),
        cpal::SampleFormat::I16 => stream_make::<i16, _>(&device, &config.into(), on_sample),
        cpal::SampleFormat::U16 => stream_make::<u16, _>(&device, &config.into(), on_sample),
    }
}

pub fn host_device_setup(
) -> Result<(cpal::Host, cpal::Device, cpal::SupportedStreamConfig), anyhow::Error> {
    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::Error::msg("Default output device is not available"))?;
    println!("Output device : {}", device.name()?);

    let config = device.default_output_config()?;
    println!("Default output config : {:?}", config);

    Ok((host, device, config))
}

pub fn stream_make<T, F>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    on_sample: F,
) -> Result<cpal::Stream, anyhow::Error>
where
    T: cpal::Sample,
    F: FnMut(&mut SampleRequestOptions) -> f32 + std::marker::Send + 'static + Copy,
{
    let sample_rate = config.sample_rate.0 as f32;
    let sample_clock = 0f32;
    let nchannels = config.channels as usize;
    let myNote = Note {
        pitch_relative_to_a: 1.2,
        length: ToneLength::Full,
    };
    let myMelody = vec![
        Note {
            pitch_relative_to_a: 1.19,
            length: ToneLength::Full,
        },
        Note {
            pitch_relative_to_a: 1.26,
            length: ToneLength::Full,
        },
    ];
    let mut request = SampleRequestOptions {
        sample_rate,
        sample_clock,
        nchannels,

        note: myNote,
        melody: myMelody,
    };
    let err_fn = |err| eprintln!("Error building output sound stream: {}", err);

    let stream = device.build_output_stream(
        config,
        move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
            on_window(output, &mut request, on_sample)
        },
        err_fn,
    )?;

    Ok(stream)
}

fn on_window<T, F>(output: &mut [T], request: &mut SampleRequestOptions, mut on_sample: F)
where
    T: cpal::Sample,
    F: FnMut(&mut SampleRequestOptions) -> f32 + std::marker::Send + 'static,
{
    for frame in output.chunks_mut(request.nchannels) {
        let value: T = cpal::Sample::from::<f32>(&on_sample(request));
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}
