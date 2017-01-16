extern crate sdl2;

mod ppu;
pub mod cpu;
mod controller;
mod cartridge;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use std::fs::File;
use std::io::Read;


pub struct Machine<'a> {
    pub ppu: ppu::Ppu<'a>,
    controller: controller::Controller,
    ram: Vec<u8>,
    nmi_line: bool,
    sdl_context: sdl2::Sdl,
    cartridge: Option<cartridge::Cartridge>,
}

#[derive(Debug,Clone,Copy)]
enum Mapper {
    NROM,
    CNROM,
}

#[allow(dead_code)]
pub fn get_state_string(cpu: &cpu::Cpu, machine: &mut Machine) -> String {
    format!("{} {}", cpu.get_state_string(machine), machine.get_state_string())
}


impl<'a> Machine<'a> {
    pub fn new() -> Self {
        let mut sdl_context = sdl2::init().unwrap();

        let ram = vec![0; 0x800];
        Machine {
            ppu: ppu::Ppu::new(&mut sdl_context),
            controller: controller::Controller::new(),
            ram: ram,
            nmi_line: true,
            sdl_context: sdl_context,
            cartridge: None,
        }
    }

    pub fn present(&mut self) {
        let cartridge = self.cartridge.as_ref().unwrap();
        self.ppu.present(cartridge);
    }

    pub fn load_rom(&mut self, rom: NesRom) {
        self.cartridge = Some(cartridge::Cartridge::new(rom));
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
                Event::KeyDown { keycode: Some(c), .. } => {
                    self.controller.handle_key_down(c);
                }
                Event::KeyUp { keycode: Some(c), .. } => {
                    self.controller.handle_key_up(c);
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
        let cart = self.cartridge.as_mut().unwrap();
        self.nmi_line = self.ppu.step_cycle(count, cart);
        let nmi_triggered = old_nmi_line && !self.nmi_line;
        nmi_triggered
    }

    fn read_mem(&mut self, address: u16) -> u8 {
        if address < 0x2000 {
            let ram_address = address & 0x7FF;
            self.ram[ram_address as usize]
        }
        else if address < 0x4000 {
            let reg_address = 0x2000 + ((address - 0x2000) & 0x7);
            let cartridge = self.cartridge.as_mut().unwrap();
            self.ppu.read_mem(cartridge, reg_address)
        }
        else if address < 0x4016 {
            //panic!("apu address {:04X} not implemented", address);
            0xFF
        }
        else if address < 0x4018 {
            self.controller.read_mem(address)
        }
        else if address < 0x8000 {
            0xFF
        }
        else {
            self.cartridge.as_ref().unwrap().read_mem_cpu(address)
        }
    }

    fn write_mem(&mut self, address: u16, value: u8) {
        if address < 0x2000 {
            let ram_address = address & 0x7FF;
            self.ram[ram_address as usize] = value;
        }
        else if address < 0x4000 {
            let reg_address = 0x2000 + ((address - 0x2000) & 0x7);
            let cartridge = self.cartridge.as_mut().unwrap();
            self.ppu.write_mem(reg_address, value, cartridge);
        }
        else if address < 0x4014 {
        }
        else if address == 0x4014 {
            let ref ram = self.ram;
            let cartridge = self.cartridge.as_mut().unwrap();
            self.ppu.perform_dma(cartridge, &ram, value as u16 * 0x100);
        }
        else if address == 0x4015 {
        }
        else if address == 0x4016 {
            self.controller.write_mem(address, value);
        }
        else if address < 0x8000 {
        }
        else {
            self.cartridge.as_ref().unwrap().write_mem_cpu(address, value);
        }
    }
}

#[derive(Debug,PartialEq)]
enum MirroringType {
    Horizontal,
    Vertical,
}

#[derive(Debug)]
pub struct NesRom {
    header: [u8; 16],
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    mirroring: MirroringType,
    mapper: Mapper,
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
    let mirroring = if data[6] & 0x01 != 0 {
        MirroringType::Vertical
    }
    else {
        MirroringType::Horizontal
    };
    let _has_trainer = data[6] & (1 << 2) == (1 << 2);
    let _has_play_choice_rom = data[7] & (1 << 2) == (1 << 2);
    let _prg_ram_size_8kb_units = data[8];
    let mapper_u8 = data[7] & 0xF0 | ((_flags6 & 0xF0) >> 4);
    let mapper = match mapper_u8 {
        0 => Mapper::NROM,
        3 => Mapper::CNROM,
        _ => { panic!("unsupported mapper: {}", mapper_u8); }
    };

    let prg_size = prg_rom_size_16kb_units as usize * 16384;
    let chr_size = chr_rom_size_8kb_units as usize * 8192;
    let mut prg_rom = vec![0; prg_size];
    prg_rom.clone_from_slice(&data[16 .. 16 + prg_size]);
    let mut chr_rom = vec![0; chr_size];
    chr_rom.clone_from_slice(&data[16 + prg_size .. 16 + prg_size + chr_size]);

    NesRom { header: header,
             prg_rom: prg_rom,
             chr_rom: chr_rom,
             mirroring: mirroring,
             mapper: mapper}
}
