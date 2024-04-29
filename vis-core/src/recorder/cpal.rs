use crate::analyzer;
use std::thread;
use cpal::HostId;
use cpal::traits::*;

#[derive(Debug, Default)]
pub struct CPalBuilder {
    pub rate: Option<usize>,
    pub buffer_size: Option<usize>,
    pub read_size: Option<usize>,
}

impl CPalBuilder {
    pub fn new() -> CPalBuilder {
        Default::default()
    }

    pub fn rate(&mut self, rate: usize) -> &mut CPalBuilder {
        self.rate = Some(rate);
        self
    }

    pub fn buffer_size(&mut self, buffer_size: usize) -> &mut CPalBuilder {
        self.buffer_size = Some(buffer_size);
        self
    }

    pub fn read_size(&mut self, read_size: usize) -> &mut CPalBuilder {
        self.read_size = Some(read_size);
        self
    }

    pub fn create(&self) -> CPalRecorder {
        CPalRecorder::from_builder(self)
    }

    pub fn build(&self) -> Box<dyn super::Recorder> {
        Box::new(self.create())
    }
}

#[derive(Debug)]
pub struct CPalRecorder {
    #[allow(unused)]
    rate: usize,
    buffer: analyzer::SampleBuffer,
}

impl CPalRecorder {
    fn from_builder(build: &CPalBuilder) -> CPalRecorder {
        let rate = build
            .rate
            .unwrap_or_else(|| crate::CONFIG.get_or("audio.rate", 8000));
        let buffer_size = build
            .buffer_size
            .unwrap_or_else(|| crate::CONFIG.get_or("audio.buffer", 16000));
        let read_size = build
            .buffer_size
            .unwrap_or_else(|| crate::CONFIG.get_or("audio.read_size", 256));

        let buf = analyzer::SampleBuffer::new(buffer_size, rate);

        {
            let buf = buf.clone();
            let mut chunk_buffer = vec![[0.0; 2]; read_size];

            thread::Builder::new()
                .name("cpal-recorder".into())
                .spawn(move || {
                    let host = if let Some(hostname) = crate::CONFIG.get::<String>("capl.host"){
                        cpal::available_hosts()
                            .into_iter()
                            .filter(|host|host.name() == hostname)
                            .next()
                            .map(|id|cpal::host_from_id(id).ok())
                            .flatten()
                            .unwrap_or_else(||{
                                log::debug!("The specified Host could not be found. Using default.");
                                cpal::default_host()
                            })
                    } else {
                        let hosts = cpal::available_hosts()
                            .iter()
                            .map(HostId::name)
                            .fold(String::new(), |mut s1, s2| {s1.push_str(s2); s1.push('\r'); s1.push('\n'); s1});
                        log::debug!("Available hosts: {}", hosts);
                        cpal::default_host()
                    };

                    let device = {
                        if let Some(name) = crate::CONFIG.get::<String>("capl.device") {
                            if let Ok(input_devices) = host.input_devices() {
                                input_devices.filter(|item|if let Ok(device_name) = item.name() {device_name == name} else {false}).next()
                            }else{
                                log::warn!("Could not get input devices");
                                None
                            }
                        } else {
                            if let Ok(input_devices) = host.input_devices(){
                                let devices = input_devices.map(|device| device.name())
                                    .filter(Result::is_ok)
                                    .map(Result::unwrap_or_default)
                                    .filter(|s|!s.is_empty())
                                    .fold(String::new(), |mut s1, s2| {s1.push_str(s2.as_str()); s1.push('\r'); s1.push('\n'); s1});
                                log::debug!("Available input devices: {}", devices);
                            }
                            None
                        }
                        .unwrap_or_else(||{
                            log::warn!("Could not get default input device");
                            host.default_input_device().expect("No Input device found")
                        })
                    };

                    let config = cpal::StreamConfig {
                        channels: 2,
                        sample_rate: cpal::SampleRate(rate as u32),
                        buffer_size: cpal::BufferSize::Fixed(read_size as u32),
                    };

                    let stream = device.build_input_stream_raw(
                        &config,
                        cpal::SampleFormat::F32,
                        move |data, _info| {
                            let slice = data.as_slice::<f32>().expect("Wrong sample buffer data type!");
                            for chunk in slice.chunks(chunk_buffer.len() * 2) {
                                let len = chunk.len() / 2;
                                for p in chunk_buffer.iter_mut().zip(chunk.chunks_exact(2)) {
                                    match p {
                                        (b, [l, r]) => *b = [*l, *r],
                                        _ => unreachable!(),
                                    }
                                }
                                buf.push(&chunk_buffer[..len]);
                            }
                        },
                        |err| {
                            panic!("Stream Error: {err:?}");
                        },
                        None,
                    ).expect("Failed to build input stream");

                    log::debug!("CPal:");
                    log::debug!("    Sample Rate = {:6}", rate);
                    log::debug!("    Read Size   = {:6}", read_size);
                    log::debug!("    Buffer Size = {:6}", buffer_size);
                    log::debug!("    Device      = \"{}\"", device.name().as_deref().unwrap_or("unknown"));

                    stream.play().unwrap();

                    loop {
                        std::thread::park();
                    }
                })
                .unwrap();
        }

        CPalRecorder { rate, buffer: buf }
    }
}

impl super::Recorder for CPalRecorder {
    fn sample_buffer<'a>(&'a self) -> &'a analyzer::SampleBuffer {
        &self.buffer
    }
}
