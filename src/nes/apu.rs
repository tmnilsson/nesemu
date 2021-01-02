use sdl2::audio::{AudioQueue, AudioSpecDesired};

const CYCLE_FREQ: f64 = 1.789773 * 1000000.0 / 2.0;

pub struct Apu {
    output_sample_generator: OutputSampleGenerator,
    cycle_count: u64,
    audio_level: f32,
    pulse1_enabled: bool,
    phase_inc: f32,
    phase: f32,
    volume: f32,
}

impl Apu {
    pub fn new(sdl_context: &mut sdl2::Sdl) -> Apu {
        Apu {
            output_sample_generator: OutputSampleGenerator::new(sdl_context),
            cycle_count: 0,
            audio_level: 0.0,
            pulse1_enabled: false,
            phase_inc: 440.0 / CYCLE_FREQ as f32,
            phase: 0.0,
            volume: 0.25
        }
    }

    pub fn step_cycle(&mut self, count: u16) {
        for _ in 0..count {
            if self.cycle_count % 2 == 0 {
                self.update_audio_level();
                self.output_sample_generator.maybe_generate(self.audio_level);
            }
            self.cycle_count += 1;
        }
    }

    fn update_audio_level(&mut self) {
        let volume = if self.pulse1_enabled {
            self.volume
        } else {
            0.0
        };
        self.audio_level = if self.phase <= 0.5 {
            volume
        } else {
            -volume
        };
        self.phase = (self.phase + self.phase_inc) % 1.0;
    }

    pub fn get_queue_size_ms(&self) -> usize {
        self.output_sample_generator.get_queue_size_ms()
    }

    pub fn write_mem(&mut self, address: u16, value: u8) {
        match address {
            0x4015 => {
                self.pulse1_enabled = value & 0x01 != 0;
            }
            _ => { }
        }
    }
}

struct OutputSampleGenerator {
    device: AudioQueue<f32>,
    cycle_time: f64,
    time_to_next_output_sample: f64,
    output_sample_period: f64,
    output_sample_buffer: Vec<f32>,
    output_sample_index: usize,
}

impl OutputSampleGenerator {
    pub fn new(sdl_context: &mut sdl2::Sdl) -> OutputSampleGenerator {
        let audio_subsystem = sdl_context.audio().unwrap();
        let desired_spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1),  // mono
            samples: None       // default sample size
        };

        let device = audio_subsystem.open_queue(None, &desired_spec).unwrap();
        
        device.resume();

        let spec = device.spec().clone();

        OutputSampleGenerator {
            device: device,
            cycle_time: 1.0 / CYCLE_FREQ as f64,
            time_to_next_output_sample: 0.0,
            output_sample_period: 1.0 / spec.freq as f64,
            output_sample_buffer: vec![0.0; spec.samples as usize],
            output_sample_index: 0,
        }
    }

    fn maybe_generate(&mut self, audio_level: f32) {
        self.time_to_next_output_sample -= self.cycle_time;
        if self.time_to_next_output_sample <= 0.0 {
            self.time_to_next_output_sample += self.output_sample_period;
            self.output_sample_buffer[self.output_sample_index] = audio_level;
            self.output_sample_index += 1;
            if self.output_sample_index >= self.output_sample_buffer.len() {
                self.device.queue(&self.output_sample_buffer);
                self.output_sample_index = 0;
            }
        }
    }

    pub fn get_queue_size_ms(&self) -> usize {
        let queue_size_bytes = self.device.size();
        let bytes_per_sample = 4;  // f32
        let queue_size_samples = queue_size_bytes / bytes_per_sample;
        let queue_size_ms = ((queue_size_samples as f64 * self.output_sample_period) * 1000.0) as usize;
        queue_size_ms
    }
}