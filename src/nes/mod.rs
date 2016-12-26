extern crate sdl2;

mod ppu;
pub mod cpu;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use std::fs::File;
use std::io::Read;


pub struct Machine<'a> {
    pub ppu: ppu::Ppu<'a>,
    memory: Vec<u8>,
    nmi_line: bool,
    sdl_context: sdl2::Sdl,
}


#[allow(dead_code)]
pub fn get_state_string(cpu: &cpu::Cpu, machine: &mut Machine) -> String {
    format!("{} {}", cpu.get_state_string(machine), machine.get_state_string())
}


impl<'a> Machine<'a> {
    pub fn new() -> Self {
        let mut sdl_context = sdl2::init().unwrap();

        let mut memory = vec![0; 0x10000];
        // Set I/O registers to FF
        for i in 0x4000..0x4020 {
            memory[i] = 0xFF;
        }
        Machine {
            ppu: ppu::Ppu::new(&mut sdl_context),
            memory: memory,
            nmi_line: true,
            sdl_context: sdl_context,
        }
    }

    pub fn clear(&mut self) {
        self.ppu.clear();
    }

    pub fn present(&mut self) {
        self.ppu.present();
    }

    pub fn load_rom(&mut self, rom: NesRom) {
        if rom.prg_rom.len() == 16384 {
            self.memory[0x8000..0xc000].clone_from_slice(&rom.prg_rom);
            self.memory[0xc000..0x10000].clone_from_slice(&rom.prg_rom);
        }
        self.ppu.load_chr_rom(&rom.chr_rom);
    }

    pub fn handle_events(&mut self) -> bool {
        let mut event_pump = self.sdl_context.event_pump().unwrap();
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    return true;
                },
                Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                    println!("Space");
                }
                _ => {}
            }
        }
        false
    }

    #[cfg(test)]
    pub fn set_scan_line(&mut self, scan_line: i16) {
        self.ppu.set_scan_line(scan_line);
    }

    #[allow(dead_code)]
    pub fn get_state_string(&self) -> String {
        format!("CYC:{:3} SL:{}",
                self.ppu.cycle_count, self.ppu.scan_line)
    }
    
    fn step_cycle(&mut self, count: u16) -> bool {
        let old_nmi_line = self.nmi_line;
        self.nmi_line = self.ppu.step_cycle(count);
        let nmi_triggered = old_nmi_line && !self.nmi_line;
        nmi_triggered
    }

    fn read_mem(&mut self, address: u16) -> u8 {
        if address >= 0x2000 && address < 0x2008 {
            self.ppu.read_mem(address)
        }
        else {
            self.memory[address as usize]
        }
    }

    fn write_mem(&mut self, address: u16, value: u8) {
        if address >= 0x2000 && address < 0x2008 {
            self.ppu.write_mem(address, value);
        }
        else {
            self.memory[address as usize] = value;
        }
    }
}

#[derive(Debug)]
pub struct NesRom {
    header: [u8; 16],
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
}

pub fn read_nes_file(path: &str) -> NesRom {
    let mut data = Vec::new();
    let mut f = File::open(path).expect("Unable to open file");
    f.read_to_end(&mut data).expect("Unable to read data");

    let mut header = [0; 16];
    header.clone_from_slice(&data[0..16]);
    let magic = "NES\x1a".as_bytes();
    if &data[0..4] != magic {
        panic!("Not a NES file");
    }
    let prg_rom_size_16kb_units = data[4];
    let chr_rom_size_8kb_units = data[5];
    let _flags6 = data[6];
    let _has_trainer = data[6] & (1 << 2) == (1 << 2);
    let _has_play_choice_rom = data[7] & (1 << 2) == (1 << 2);
    let _prg_ram_size_8kb_units = data[8];

    let prg_size = prg_rom_size_16kb_units as usize * 16384;
    let chr_size = chr_rom_size_8kb_units as usize * 8192;
    let mut prg_rom = vec![0; prg_size];
    prg_rom.clone_from_slice(&data[16 .. 16 + prg_size]);
    let mut chr_rom = vec![0; chr_size];
    chr_rom.clone_from_slice(&data[16 + prg_size .. 16 + prg_size + chr_size]);

    NesRom { header: header,
             prg_rom: prg_rom,
             chr_rom: chr_rom }
}
