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

#[derive(Debug, Clone, Copy)]
pub enum ToneLength {
    Four,
    FourDot,
    Two,
    TwoDot,
    Full,
    FullDot,
    Half,
    HalfDot,
    Quarter,
    QuarterDot,
    Octet,
}

#[derive(Debug, Clone, Copy)]
pub struct Note {
    pub pitch_relative_to_a: RelativeFrequency,
    pub length: ToneLength,
}

impl Note {
    pub fn beats(self) -> f32 {
        match self.length {
            ToneLength::Four => 4.0,
            ToneLength::FourDot => 6.0,
            ToneLength::Two => 2.0,
            ToneLength::TwoDot => 3.0,
            ToneLength::Full => 1.0,
            ToneLength::FullDot => 1.5,
            ToneLength::Half => 0.5,
            ToneLength::HalfDot => 0.75,
            ToneLength::Quarter => 0.25,
            ToneLength::QuarterDot => 0.25 + 0.0125,
            ToneLength::Octet => 0.0125,
        }
    }
}

fn sample_next(o: &mut SampleRequestOptions) -> f32 {
    o.tick();

    o.tone()
}

#[derive(Debug, Clone)]
pub struct Melody {
    pub melody: Vec<Note>,
}

type BEATS_PER_MINUTE = u8;
type SECONDS = u64;

fn current_beat_number(time: SECONDS, bpm: BEATS_PER_MINUTE) -> f32 {
    (time * bpm as u64 / 60) as f32
}

impl Melody {
    pub fn pitch_at(&self, time: SECONDS, bpm: BEATS_PER_MINUTE) -> f32 {
        self.melody[self.beat_to_note(current_beat_number(time, bpm))].pitch_relative_to_a
    }

    fn beat_to_note(&self, time_in_beat: f32) -> usize {
        let beats: Vec<f32> = self
            .melody
            .iter()
            .scan(0.0, |last_beat, &x| {
                *last_beat += x.beats();
                Some(*last_beat)
            })
            .collect();

        let too_early =
            beats
                .iter()
                .map(|&x| x <= time_in_beat)
                .fold(0, |acc, x| if x { acc + 1 } else { acc });
        too_early as usize
    }
}

pub struct SampleRequestOptions {
    pub sample_rate: f32,
    pub sample_clock: f32,
    pub nchannels: usize,

    pub note: Note,
    pub melody: Melody,
}

impl SampleRequestOptions {
    fn tone(&self) -> f32 {
        let time_in_seconds: SECONDS = (self.sample_clock as i64 / 1000) as u64;
        let bpm = 2;

        (self.sample_clock
            * self.melody.pitch_at(time_in_seconds, bpm)
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
    let my_note = Note {
        pitch_relative_to_a: 1.2,
        length: ToneLength::Full,
    };
    let my_melody = Melody {
        melody: vec![
            Note {
                pitch_relative_to_a: 1.0,
                length: ToneLength::Full,
            },
            Note {
                pitch_relative_to_a: 2.0,
                length: ToneLength::Full,
            },
        ],
    };
    let mut request = SampleRequestOptions {
        sample_rate,
        sample_clock,
        nchannels,

        note: my_note,
        melody: my_melody,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_tone_first_tone_of_melody() {
        let my_melody = Melody {
            melody: vec![Note {
                pitch_relative_to_a: 1.0,
                length: ToneLength::Full,
            }],
        };
        assert_eq!(my_melody.pitch_at(0, 1), 1.0);
    }

    #[test]
    fn get_tone_second_tone_of_melody() {
        let my_melody = Melody {
            melody: vec![
                Note {
                    pitch_relative_to_a: 1.0,
                    length: ToneLength::Full,
                },
                Note {
                    pitch_relative_to_a: 2.0,
                    length: ToneLength::Full,
                },
            ],
        };
        assert_eq!(my_melody.pitch_at(61, 1), 2.0);
    }
    #[test]
    fn get_tone_first_tone_of_daa_da_melody() {
        let my_melody = Melody {
            melody: vec![
                Note {
                    pitch_relative_to_a: 1.0,
                    length: ToneLength::Two,
                },
                Note {
                    pitch_relative_to_a: 2.0,
                    length: ToneLength::Full,
                },
            ],
        };
        assert_eq!(my_melody.pitch_at(61, 1), 1.0);
    }
}
