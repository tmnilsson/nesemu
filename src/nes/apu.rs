use sdl2::audio::{AudioQueue, AudioSpecDesired};

const CYCLE_FREQ: f64 = 1.789773 * 1000000.0 / 2.0;

enum FrameCounterSequence {
    FourStep,
    FiveStep,
}

pub struct Apu {
    output_sample_generator: OutputSampleGenerator,
    frame_counter_sequence: FrameCounterSequence,
    interrupt_inhibit_flag: bool,
    cycle_count: u64,
    audio_level: f32,
    pulse1: PulseChannel,
    pulse2: PulseChannel,
    triangle: TriangleChannel,
}

impl Apu {
    pub fn new(sdl_context: &mut sdl2::Sdl) -> Apu {
        Apu {
            output_sample_generator: OutputSampleGenerator::new(sdl_context),
            frame_counter_sequence: FrameCounterSequence::FourStep,
            interrupt_inhibit_flag: false,
            cycle_count: 0,
            audio_level: 0.0,
            pulse1: PulseChannel::new(true),
            pulse2: PulseChannel::new(false),
            triangle: TriangleChannel::new(),
        }
    }

    pub fn step_cycle(&mut self, count: u16) -> bool {
        let mut irq_triggered = false;
        for _ in 0..count {
            self.triangle.update_level();
            if self.cycle_count % 2 == 0 {
                self.pulse1.update_level();
                self.pulse2.update_level();
                self.update_audio_level();
                self.output_sample_generator.maybe_generate(self.audio_level);
            }
            self.cycle_count += 1;
            let cycle_wrap_around = match self.frame_counter_sequence {
                FrameCounterSequence::FourStep => 14915 * 2,
                FrameCounterSequence::FiveStep => 18641 * 2,
            };
            if self.cycle_count >= cycle_wrap_around {
                self.cycle_count = 0;
            }
            match self.frame_counter_sequence {
                FrameCounterSequence::FourStep => {
                    if self.cycle_count == 7456*2+1 || self.cycle_count == 14914*2+1 {
                        self.step_quarter_frame_clock();
                        self.step_half_frame_clock();
                    }
                    else if self.cycle_count == 3728*2+1 || self.cycle_count == 11185*2+1 {
                        self.step_quarter_frame_clock();
                    }
                    if self.cycle_count == 0 || self.cycle_count >= 14914*2 {
                        if !self.interrupt_inhibit_flag {
                            irq_triggered = true;
                        }
                    }
                }
                FrameCounterSequence::FiveStep => {
                    if self.cycle_count == 7456*2+1 || self.cycle_count == 18640*2+1 {
                        self.step_quarter_frame_clock();
                        self.step_half_frame_clock();
                    }
                    else if self.cycle_count == 3728*2+1 || self.cycle_count == 11185*2+1 {
                        self.step_quarter_frame_clock();
                    }
                }
            }
        }
        irq_triggered
    }

    fn step_quarter_frame_clock(&mut self) {
        self.pulse1.step_envelope_clock();
        self.pulse2.step_envelope_clock();
        self.triangle.step_linear_counter_clock();
    }

    fn step_half_frame_clock(&mut self) {
        self.pulse1.step_length_counter_clock();
        self.pulse1.step_sweep_clock();
        self.pulse2.step_length_counter_clock();
        self.pulse2.step_sweep_clock();
        self.triangle.step_length_counter_clock();
    }

    fn update_audio_level(&mut self) {
        let pulse_out = 95.88 / ((8128.0 / (self.pulse1.output_level as f32 + self.pulse2.output_level as f32)) + 100.0);
        let tnd_out = 159.79 / (1.0 / (self.triangle.output_level as f32 / 8227.0) + 100.0);
        self.audio_level = pulse_out + tnd_out;
    }

    pub fn get_queue_size_ms(&self) -> usize {
        self.output_sample_generator.get_queue_size_ms()
    }

    pub fn write_mem(&mut self, address: u16, value: u8) {
        match address {
            0x4000 => {
                self.pulse1.set_control1(value);
            }
            0x4001 => {
                self.pulse1.setup_sweep(value);
            }
            0x4002 => {
                self.pulse1.set_timer_max_low(value);
            }
            0x4003 => {
                self.pulse1.set_timer_max_high(value);
            }
            0x4004 => {
                self.pulse2.set_control1(value);
            }
            0x4005 => {
                self.pulse2.setup_sweep(value);
            }
            0x4006 => {
                self.pulse2.set_timer_max_low(value);
            }
            0x4007 => {
                self.pulse2.set_timer_max_high(value);
            }
            0x4008 => {
                self.triangle.set_halt_and_linear_counter_load(value);
            }
            0x400A => {
                self.triangle.set_timer_max_low(value);
            }
            0x400B => {
                self.triangle.set_length_counter_load_and_timer_max_high(value);
            }
            0x4015 => {
                self.pulse1.set_enabled(value & 0x01 != 0);
                self.pulse2.set_enabled(value & 0x02 != 0);
                self.triangle.set_enabled(value & 0x04 != 0);
            }
            0x4017 => {
                self.frame_counter_sequence = if value & 0x80 == 0 {
                    FrameCounterSequence::FourStep
                } else {
                    FrameCounterSequence::FiveStep
                };
                self.interrupt_inhibit_flag = value & 0b0100_0000 != 0;
            }
            _ => { }
        }
    }
}

struct Envelope {
    volume: u8,  // also used as envelope period (like in the hardware)
    loop_flag: bool,
    constant_volume_flag: bool,
    start_flag: bool,
    decay_level: u8,
    divider: u8,
}

impl Envelope {
    fn new() -> Envelope {
        Envelope {
            volume: 0,
            constant_volume_flag: false,
            loop_flag: false,
            start_flag: false,
            decay_level: 0,
            divider: 0,
        }
    }

    fn set_volume(&mut self, volume: u8) {
        self.volume = volume;
    }

    fn set_loop_flag(&mut self, loop_flag: bool) {
        self.loop_flag = loop_flag;
    }

    fn set_constant_volume_flag(&mut self, constant_volume_flag: bool) {
        self.constant_volume_flag = constant_volume_flag;
    }

    fn set_start_flag(&mut self) {
        self.start_flag = true;
    }

    fn step_clock(&mut self) {
        if self.start_flag {
            self.start_flag = false;
            self.decay_level = 15;
            self.divider = self.volume;
        }
        else {
            if self.divider > 0 {
                self.divider -= 1;
            }
            else {
                self.divider = self.volume;
                if self.decay_level > 0 {
                    self.decay_level -= 1;
                }
                else {
                    if self.loop_flag {
                        self.decay_level = 15;
                    }
                }
            }
        }
    }

    fn get_output_level(&self) -> u8 {
        if self.constant_volume_flag {
            self.volume
        }
        else {
            self.decay_level
        }
    }
}

struct LengthCounter {
    counter: u8,
    enabled: bool,
    halt: bool,
}

impl LengthCounter {
    const LENGTH_TABLE: [u8; 32] = [
        10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14,
        12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
    ];

    fn new() -> LengthCounter {
        LengthCounter {
            counter: 0,
            enabled: false,
            halt: false,
        }
    }

    fn step_clock(&mut self) {
        if self.counter > 0 && !self.halt {
            self.counter -= 1;
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !self.enabled {
            self.counter = 0;
        }
    }

    fn set_halt(&mut self, halt: bool) {
        self.halt = halt;
    }

    fn load(&mut self, value: u8) {
        if self.enabled {
            self.counter = LengthCounter::LENGTH_TABLE[value as usize];
        }
    }

    fn is_zero(&self) -> bool {
        return self.counter == 0;
    }
}

struct Sweep {
    enabled: bool,
    timer_max: u8,
    timer: u8,
    negate: bool,
    shift_count: u8,
    reload_flag: bool,
    muted: bool,
    extra_minus_one: bool,
}

impl Sweep {
    fn new(extra_minus_one: bool) -> Sweep {
        Sweep {
            enabled: false,
            timer_max: 0,
            timer: 0,
            negate: false,
            shift_count: 0,
            reload_flag: false,
            muted: false,
            extra_minus_one: extra_minus_one,
        }
    }

    fn step_clock(&mut self, period: &mut u16) {
        let target_period = if self.shift_count == 0 {
            *period
        }
        else {
            let mut change_amount: i16 = (*period as i16) >> self.shift_count as i16;
            if self.negate {
                change_amount = -change_amount;
                if self.extra_minus_one {
                    change_amount -= 1;
                }
            }
            (*period as i16 + change_amount) as u16
        };
        self.muted = target_period > 0x7FF || *period < 8;

        if self.timer == 0 && self.enabled && !self.muted{
            *period = target_period;
        }

        if self.timer == 0 || self.reload_flag {
            self.timer = self.timer_max;
            self.reload_flag = false;
        }
        else {
            self.timer -= 1;
        }
    }

    fn is_muted(&self) -> bool {
        return self.muted;
    }

    fn setup(&mut self, value: u8) {
        self.enabled = value & 0b1000_0000 != 0;
        self.timer_max = (value & 0b0111_0000) >> 4;
        self.negate = value & 0b0000_1000 != 0;
        self.shift_count = value & 0b0000_0111;
        self.reload_flag = true;
    }
}

struct PulseChannel {
    duty_cycle: usize,
    timer_max: u16,
    timer: u16,
    sequence_index: usize,
    envelope: Envelope,
    length_counter: LengthCounter,
    sweep: Sweep,
    pub output_level: u8,
}

impl PulseChannel {
    const WAVEFORMS: [[u8; 8]; 4] = [
        [0, 1, 0, 0, 0, 0, 0, 0],
        [0, 1, 1, 0, 0, 0, 0, 0],
        [0, 1, 1, 1, 1, 0, 0, 0],
        [1, 0, 0, 1, 1, 1, 1, 1],
    ];

    fn new(extra_minus_one: bool) -> PulseChannel {
        PulseChannel {
            duty_cycle: 0,
            timer_max: 0,
            timer: 0,
            sequence_index: 0,
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
            sweep: Sweep::new(extra_minus_one),
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
            self.output_level = &PulseChannel::WAVEFORMS[self.duty_cycle][self.sequence_index] * self.envelope.get_output_level();
            if self.timer_max < 8 {
                self.output_level = 0;
            }
        } else {
            self.timer -= 1
        }
        if self.length_counter.is_zero() || self.sweep.is_muted() {
            self.output_level = 0;
        }
    }

    fn set_control1(&mut self, value: u8) {
        self.duty_cycle = (value >> 6).into();
        let loop_and_halt_flag = value & 0x20 != 0;
        self.envelope.set_loop_flag(loop_and_halt_flag);
        self.length_counter.set_halt(loop_and_halt_flag);
        self.envelope.set_constant_volume_flag(value & 0x10 != 0);
        self.envelope.set_volume(value & 0x0F);
    }

    fn set_timer_max_low(&mut self, value: u8) {
        self.timer_max = (self.timer_max & 0xFF00) | value as u16;
    }

    fn set_timer_max_high(&mut self, value: u8) {
        self.timer_max = (self.timer_max & 0x00FF) | ((value as u16 & 0x07) << 8);
        self.length_counter.load(value >> 3);
        self.envelope.set_start_flag();
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.length_counter.set_enabled(enabled);
    }

    fn setup_sweep(&mut self, value: u8) {
        self.sweep.setup(value);
    }

    fn step_envelope_clock(&mut self) {
        self.envelope.step_clock();
    }

    fn step_length_counter_clock(&mut self) {
        self.length_counter.step_clock();
    }

    fn step_sweep_clock(&mut self) {
        self.sweep.step_clock(&mut self.timer_max);
    }
}

struct LinearCounter {
    counter: u8,
    reload_value: u8,
    reload_flag: bool,
    control_flag: bool,
}

impl LinearCounter {
    fn new() -> LinearCounter {
        LinearCounter {
            counter: 0,
            reload_value: 0,
            reload_flag: false,
            control_flag: false,
        }
    }

    fn step_clock(&mut self) {
        if self.reload_flag {
            self.counter = self.reload_value;
        }
        else if self.counter > 0 {
            self.counter -= 1;
        }
        if !self.control_flag {
            self.reload_flag = false;
        }
    }

    fn setup(&mut self, value: u8) {
        self.control_flag = value & 0x80 != 0;
        self.reload_value = value & 0x7F;
    }

    fn set_reload_flag(&mut self) {
        self.reload_flag = true;
    }

    fn is_zero(&self) -> bool {
        return self.counter == 0;
    }
}

struct TriangleChannel {
    pub timer_max: u16,
    timer: u16,
    sequence_index: usize,
    length_counter: LengthCounter,
    linear_counter: LinearCounter,
    pub output_level: u8,
}

impl TriangleChannel {
    const WAVEFORM: [u8; 32] = [
        15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ];

    fn new() -> TriangleChannel {
        TriangleChannel {
            timer_max: 0,
            timer: 0,
            sequence_index: 0,
            length_counter: LengthCounter::new(),
            linear_counter: LinearCounter::new(),
            output_level: 0,
        }
    }

    fn update_level(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_max;
            self.sequence_index += 1;
            if self.sequence_index > 31 {
                self.sequence_index = 0;
            }
            self.output_level = TriangleChannel::WAVEFORM[self.sequence_index];
        } else {
            self.timer -= 1
        }
        if self.length_counter.is_zero() || self.linear_counter.is_zero() {
            self.output_level = 0;
        }
    }

    fn set_halt_and_linear_counter_load(&mut self, value: u8) {
        let control_and_halt_flag = value & 0x80 != 0;
        self.length_counter.set_halt(control_and_halt_flag);
        self.linear_counter.setup(value);
    }

    fn set_timer_max_low(&mut self, value: u8) {
        self.timer_max = (self.timer_max & 0xFF00) | value as u16;
    }

    fn set_length_counter_load_and_timer_max_high(&mut self, value: u8) {
        self.timer_max = (self.timer_max & 0x00FF) | ((value as u16 & 0x7) << 8);
        self.length_counter.load(value >> 3);
        self.linear_counter.set_reload_flag();
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.length_counter.set_enabled(enabled);
    }

    fn step_length_counter_clock(&mut self) {
        self.length_counter.step_clock();
    }

    fn step_linear_counter_clock(&mut self) {
        self.linear_counter.step_clock();
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
