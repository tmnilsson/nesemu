pub struct Ppu {
    pub scan_line: i16,
    pub cycle_count: u16,
    vblank: bool,
    gen_nmi_at_vblank: bool,
    pub mem_read_mut_enabled: bool,
    ppu_addr: u16,
    memory: Vec<u8>
}

impl Ppu {
    pub fn new() -> Ppu {
        Ppu {
            scan_line: 0,
            cycle_count: 0,
            vblank: false,
            gen_nmi_at_vblank: false,

            mem_read_mut_enabled: true,
            ppu_addr: 0,
            memory: vec![0; 0x10000],
        }
    }

    pub fn set_scan_line(&mut self, scan_line: i16) {
        self.scan_line = scan_line;
    }

    pub fn step_cycle(&mut self, count: u16) -> bool {
        self.cycle_count += count * 3;
        if self.cycle_count >= 341 {
            self.cycle_count -= 341;
            self.scan_line += 1;
            if self.scan_line == 241 {
                self.vblank = true;
            }
            if self.scan_line >= 261 {
                self.scan_line = -1;
                self.vblank = false;
            }
        }
        let nmi_line = !(self.vblank && self.gen_nmi_at_vblank);
        nmi_line
    }

    pub fn read_mem(&mut self, cpu_address: u16) -> u8 {
        match cpu_address {
            0x2000 | 0x2001 | 0x2005 | 0x2006 => { // Write-only registers, return 0
                0
            }
            0x2002 => {
                let value = if self.vblank {0x80} else {0x00};
                if self.mem_read_mut_enabled {
                    self.vblank = false;
                    self.ppu_addr = 0;
                }
                value
            }
            0x2007 => {
                let addr = self.ppu_addr;
                self.read_mem_ppu(addr)
            }
            _ => panic!("Unimplemented read address: {:04X}", cpu_address)
        }
    }

    pub fn write_mem(&mut self, cpu_address: u16, value: u8) {
        match cpu_address {
            0x2000 => {
                if value != 0 && value != 0x80 && value != 0x84 {
                    panic!("Unimplemented! {:02X}", value);
                }
                self.gen_nmi_at_vblank = (value & 0x80) != 0;
            }
            0x2001 | 0x2005 => {
            }
            0x2006 => {
                self.ppu_addr = (self.ppu_addr << 8) + value as u16;
            }
            0x2007 => {
                let addr = self.ppu_addr;
                self.write_mem_ppu(addr, value);
            }
            _ => panic!("Unimplemented write address: {:04X}", cpu_address)
        }
    }

    fn read_mem_ppu(&self, ppu_address: u16) -> u8 {
        self.memory[ppu_address as usize]
    }

    fn write_mem_ppu(&mut self, ppu_address: u16, value: u8) {
        self.memory[ppu_address as usize] = value;
    }
}
