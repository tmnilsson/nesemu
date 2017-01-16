use nes;

pub struct Cartridge {
    rom: nes::NesRom,
}

impl Cartridge {
    pub fn new(rom: nes::NesRom) -> Self {
        Cartridge {
            rom: rom,
        }
    }

    pub fn read_mem_cpu(&self, address: u16) -> u8 {
        match self.rom.mapper {
            nes::Mapper::NROM => {
                let mem_address = if self.rom.prg_rom.len() == 16384 {
                    (address - 0x8000) & 0x3FFF
                }
                else {
                    address
                };
                self.rom.prg_rom[mem_address as usize]
            }
            nes::Mapper::CNROM => {
                unimplemented!()
            }
        }
    }

    pub fn write_mem_cpu(&self, address: u16, value: u8) {
        println!("ignoring write {} to addr {:04X}", value, address);
    }

    pub fn read_mem_ppu(&self, address: u16, vram: &[u8]) -> u8 {
        match self.rom.mapper {
            nes::Mapper::NROM => {
                if address < 0x2000 {
                    self.rom.chr_rom[address as usize]
                }
                else if address < 0x3000 {
                    let vram_address = if self.rom.mirroring == nes::MirroringType::Vertical {
                        (address & 0xF7FF) - 0x2000
                    }
                    else {
                        ((address & 0xF3FF) | ((address >> 1) & 0x0400)) - 0x2000
                    };
                    vram[vram_address as usize]
                }
                else if address < 0x3F00 {
                    self.read_mem_ppu(address - 0x1000, vram)
                }
                else {
                    panic!("unexpected address: {:04X}", address);
                }
            }
            nes::Mapper::CNROM => {
                0
            }
        }
    }

    pub fn write_mem_ppu(&self, address: u16, value: u8, vram: &mut [u8]) {
        match self.rom.mapper {
            nes::Mapper::NROM => {
                if address < 0x2000 {
                    //panic!("unexpected address: {:04X}", address);
                }
                else if address < 0x3000 {
                    let vram_address = if self.rom.mirroring == nes::MirroringType::Vertical {
                        (address & 0xF7FF) - 0x2000
                    }
                    else {
                        ((address & 0xF3FF) | ((address >> 1) & 0x0400)) - 0x2000
                    };
                    vram[vram_address as usize] = value;
                }
                else if address < 0x3F00 {
                    self.write_mem_ppu(address - 0x1000, value, vram)
                }
                else {
                    panic!("unexpected address: {:04X}", address);
                }
            }
            nes::Mapper::CNROM => {

            }
        }
    }
}
