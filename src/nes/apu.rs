use std::sync::{Arc, Mutex};
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};

pub struct Apu {
    device: AudioDevice<PlaybackHandler>,
    playback_data: Arc<Mutex<PlaybackData>>,
}

struct PlaybackData {
    pulse1_enabled: bool,
    phase_inc: f32,
    phase: f32,
    volume: f32
}

struct PlaybackHandler {
    data: Arc<Mutex<PlaybackData>>,
}

impl AudioCallback for PlaybackHandler {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let mut data = self.data.lock().unwrap();
        let volume = if data.pulse1_enabled {
            data.volume
        } else {
            0.0
        };
        for x in out.iter_mut() {
            *x = if data.phase <= 0.5 {
                volume
            } else {
                -volume
            };
            data.phase = (data.phase + data.phase_inc) % 1.0;
        }
    }
}

impl Apu {
    pub fn new(sdl_context: &mut sdl2::Sdl) -> Apu {
        let audio_subsystem = sdl_context.audio().unwrap();
        let desired_spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1),  // mono
            samples: None       // default sample size
        };

        let playback_data = Arc::new(Mutex::new(PlaybackData{
            pulse1_enabled: false,
            phase_inc: 0.0,
            phase: 0.0,
            volume: 0.25
        }));
        
        let device = audio_subsystem.open_playback(None, &desired_spec, |spec| {
            let playback_data = Arc::clone(&playback_data);
            {
                let mut data = playback_data.lock().unwrap();
                data.phase_inc = 440.0 / spec.freq as f32;
            }
            PlaybackHandler { data: playback_data }
        }).unwrap();
        
        device.resume();

        Apu {
            device: device,
            playback_data: playback_data,
        }
    }

    pub fn write_mem(&mut self, address: u16, value: u8) {
        match address {
            0x4015 => {
                let mut data = self.playback_data.lock().unwrap();
                data.pulse1_enabled = value & 0x01 != 0;
            }
            _ => { }
        }
    }
}
