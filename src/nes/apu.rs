use sdl2::audio::{AudioQueue, AudioSpecDesired};

const CYCLE_FREQ: f64 = 1.789773 * 1000000.0 / 2.0;
const WAVEFORMS: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];

pub struct Apu {
    output_sample_generator: OutputSampleGenerator,
    cycle_count: u64,
    audio_level: f32,
    pulse1: PulseChannel,
}

impl Apu {
    pub fn new(sdl_context: &mut sdl2::Sdl) -> Apu {
        Apu {
            output_sample_generator: OutputSampleGenerator::new(sdl_context),
            cycle_count: 0,
            audio_level: 0.0,
            pulse1: PulseChannel::new(),
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
        self.pulse1.update_level();
        let pulse_out = 95.88 / ((8128.0 / (self.pulse1.output_level as f32)) + 100.0);
        self.audio_level = pulse_out;
    }

    pub fn get_queue_size_ms(&self) -> usize {
        self.output_sample_generator.get_queue_size_ms()
    }

    pub fn write_mem(&mut self, address: u16, value: u8) {
        match address {
            0x4000 => {
                self.pulse1.set_control1(value);
            }
            0x4002 => {
                self.pulse1.set_timer_max_low(value);
            }
            0x4003 => {
                self.pulse1.set_timer_max_high(value);
            }
            0x4015 => {
                self.pulse1.set_enabled(value & 0x01 != 0);
            }
            _ => { }
        }
    }
}

struct PulseChannel {
    pub enabled: bool,
    pub duty_cycle: usize,
    pub timer_max: u16,
    timer: u16,
    sequence_index: usize,
    pub volume: u8,
    pub output_level: u8,
}

impl PulseChannel {
    fn new() -> PulseChannel {
        PulseChannel {
            enabled: false,
            duty_cycle: 0,
            timer_max: 0,
            timer: 0,
            sequence_index: 0,
            volume: 0,
            output_level: 0,
        }
    }

    fn update_level(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_max;
            self.sequence_index += 1;
            if self.sequence_index > 7 {
                self.sequence_index = 0;
            }
            self.output_level = &WAVEFORMS[self.duty_cycle][self.sequence_index] * self.volume;
            if self.timer_max < 8 {
                self.output_level = 0;
            }
        } else {
            self.timer -= 1
        }
        if !self.enabled {
            self.output_level = 0;
        }
    }

    fn set_control1(&mut self, value: u8) {
        self.duty_cycle = (value >> 6).into();
        self.volume = value & 0x0F;
    }

    fn set_timer_max_low(&mut self, value: u8) {
        self.timer_max = (self.timer_max & 0xFF00) | value as u16;
    }

    fn set_timer_max_high(&mut self, value: u8) {
        self.timer_max = (self.timer_max & 0x00FF) | ((value as u16) << 8);
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
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
