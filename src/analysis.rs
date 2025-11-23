use cpal::{
    InputCallbackInfo,
    traits::{DeviceTrait, HostTrait},
};
use microfft::Complex32;
use std::{cell::OnceCell, io::Read};
use thingbuf::mpsc::blocking::Sender;

const BASE_BAND_SIZE: usize = 16;
const FFT_SIZE: usize = 2048;
const NUM_BANDS: usize = count_bands();
const ACCUMULATOR_SIZE: usize = FFT_SIZE * 2;

pub type Block = [f32; FFT_SIZE];
pub type SpectralFrame = [f32; NUM_BANDS];

thread_local! {
    static HANNING: OnceCell<[f32; FFT_SIZE]> = const {OnceCell::new()};
}

struct Analyzer {
    sample_rate: usize,
    rms: f32,
    accumlator: [f32; ACCUMULATOR_SIZE],
    accumulator_top: usize,
    sender: Sender<Frame>,
}

impl Analyzer {
    fn new(sample_rate: usize, sender: Sender<Frame>) -> Self {
        Self {
            sample_rate,
            rms: 0.,
            accumlator: [0.; ACCUMULATOR_SIZE],
            accumulator_top: 0,
            sender,
        }
    }

    fn make_frame_fn(mut self) -> impl FnMut(&[f32], &InputCallbackInfo) {
        move |input, _| {
            if self.accumulator_top >= FFT_SIZE {
                let spectral_data = fft(std::array::from_fn(|i| self.accumlator[i]));
                let rms = self.rms;
                self.sender
                    .send(Frame { spectral_data, rms })
                    .unwrap_or_else(|e| panic!("Channel unexpectedly closed!\n{e}"));
                self.accumlator.copy_within(FFT_SIZE.., 0);
                self.accumulator_top -= FFT_SIZE;
            }

            self.accumlator[self.accumulator_top..self.accumulator_top + input.len()]
                .copy_from_slice(input);
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Frame {
    spectral_data: SpectralFrame,
    rms: f32,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("IO failed: {0}")]
    IO(String),
}

const fn count_bands() -> usize {
    let mut count = 0;
    let mut total = 1;

    while total < FFT_SIZE / 2 {
        total += 1 << count;
        count += 1;
    }

    count
}

fn group_bands(spectrum: [Complex32; FFT_SIZE / 2]) -> [f32; NUM_BANDS] {
    let mut bands = [0.; NUM_BANDS];

    let mut start = 1;

    for (i, band) in bands.iter_mut().enumerate() {
        let band_size = 1 << i;
        let end = (start + band_size).min(FFT_SIZE / 2);

        if start >= FFT_SIZE / 2 {
            break;
        }

        let sum: f32 = spectrum[start..end].iter().map(|c| c.l1_norm()).sum();
        let band_amp = (sum / (start + end) as f32) / 50f32;
        *band = band_amp.min(1.);

        start = end;
    }

    bands
}

fn fft(mut samples: [f32; FFT_SIZE]) -> SpectralFrame {
    use microfft::real;

    let hanning = HANNING.with(|cell| *cell.get_or_init(|| std::array::from_fn(cosine_at)));
    samples
        .iter_mut()
        .zip(hanning)
        .for_each(|(sample, window)| *sample *= window);

    let frame = real::rfft_2048(&mut samples);
    group_bands(*frame)
}

fn cosine_at(idx: usize) -> f32 {
    const A: f32 = 0.5;
    const B: f32 = 0.5;
    const C: f32 = 0.;
    const D: f32 = 0.;

    let x = (std::f32::consts::PI * idx as f32) / (FFT_SIZE - 1) as f32;
    let b = B * (2. * x).cos();
    let c = C * (4. * x).cos();
    let d = D * (6. * x).cos();

    (A - b) + (c - d)
}

pub fn make_analysis_stream(sender: Sender<Frame>) -> Result<cpal::Stream, Error> {
    let hosts = cpal::available_hosts();

    let hosts_str = hosts
        .iter()
        .enumerate()
        .fold(String::new(), |mut acc, (idx, host)| {
            acc.push_str(&format!("{idx} - {}", host.name()));

            acc
        });

    let mut stdin = std::io::stdin().lock();

    println!("Please select a host:\n{hosts_str}");
    let mut host_buf = [0; 16];
    stdin
        .read(&mut host_buf)
        .map_err(|e| Error::IO(e.to_string()))?;

    let host = if let Some(id) = String::from_utf8_lossy(host_buf.as_slice())
        .parse()
        .ok()
        .and_then(|idx: usize| hosts.get(idx))
    {
        cpal::host_from_id(*id).unwrap()
    } else {
        cpal::default_host()
    };

    let device = host.default_input_device().unwrap();
    let config = device.default_input_config().unwrap();

    let sample_rate = config.sample_rate().0 as usize;

    let analyzer = Analyzer::new(sample_rate, sender);
    let err_fn = |e| log::error!("AUDIO CALLBACK: {e}");

    let stream = device
        .build_input_stream(&config.into(), analyzer.make_frame_fn(), err_fn, None)
        .unwrap();

    Ok(stream)
}
