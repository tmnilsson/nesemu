extern crate sdl2;

pub mod cpu;
pub mod cartridge;
mod ppu;
mod apu;
mod controller;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;


pub struct Machine {
    pub ppu: ppu::Ppu,
    pub apu: apu::Apu,
    pub controller: controller::Controller,
    ram: Vec<u8>,
    nmi_line: bool,
    sdl_context: sdl2::Sdl,
    cartridge: Option<cartridge::Cartridge>,
}

#[derive(PartialEq)]
pub enum SystemEvent {
    Quit,
    Reset,
}

#[allow(dead_code)]
pub fn get_state_string(cpu: &cpu::Cpu, machine: &mut Machine) -> String {
    format!("{} {}", cpu.get_state_string(machine), machine.get_state_string())
}


impl Machine {
    pub fn new(show_name_table: bool) -> Self {
        let mut sdl_context = sdl2::init().unwrap();

        let ram = vec![0; 0x800];
        Machine {
            ppu: ppu::Ppu::new(&mut sdl_context, show_name_table),
            apu: apu::Apu::new(&mut sdl_context),
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

    pub fn load_cartridge(&mut self, cartridge: cartridge::Cartridge) {
        self.cartridge = Some(cartridge);
    }

    pub fn save(&self) {
        match self.cartridge.as_ref() {
            Some(c) => c.save(),
            None => {}
        }
    }

    pub fn handle_events(&mut self) -> Option<SystemEvent> {
        let mut event_pump = self.sdl_context.event_pump().unwrap();
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    return Some(SystemEvent::Quit);
                },
                Event::KeyDown { keycode: Some(c), .. } => {
                    if c == Keycode::R {
                        return Some(SystemEvent::Reset);
                    }
                    else {
                        self.controller.handle_key_down(c);
                    }
                }
                Event::KeyUp { keycode: Some(c), .. } => {
                    self.controller.handle_key_up(c);
                }
                _ => {}
            }
        }
        None
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
    
    fn step_cycle(&mut self, count: u16) -> (bool, bool) {
        let irq_triggered = self.apu.step_cycle(count);
        let old_nmi_line = self.nmi_line;
        let cart = self.cartridge.as_mut().unwrap();
        self.nmi_line = self.ppu.step_cycle(count, cart);
        let nmi_triggered = old_nmi_line && !self.nmi_line;
        (nmi_triggered, irq_triggered)
    }

    pub fn get_audio_queue_size_ms(&self) -> usize {
        self.apu.get_queue_size_ms()
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
            0xFF // TODO: implement APU
        }
        else if address < 0x4018 {
            self.controller.read_mem(address)
        }
        else if address < 0x4020 {
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
            self.apu.write_mem(address, value);
        }
        else if address == 0x4014 {
            let ref ram = self.ram;
            let cartridge = self.cartridge.as_mut().unwrap();
            self.ppu.perform_dma(cartridge, &ram, value as u16 * 0x100);
        }
        else if address == 0x4015 {
            self.apu.write_mem(address, value);
        }
        else if address == 0x4016 {
            self.controller.write_mem(address, value);
        }
        else if address == 0x4017 {
            self.apu.write_mem(address, value);
        }
        else if address < 0x4020 {
        }
        else {
            self.cartridge.as_mut().unwrap().write_mem_cpu(address, value);
        }
    }
}
