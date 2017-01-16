use std::fs::File;
use std::io::Read;


#[derive(Debug,PartialEq)]
enum MirroringType {
    Horizontal,
    Vertical,
}

#[derive(Debug,Clone,Copy)]
enum Mapper {
    NROM,
    CNROM,
}

#[derive(Debug)]
pub struct NesRomFile {
    header: [u8; 16],
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    mirroring: MirroringType,
    mapper: Mapper,
}

pub fn read_nes_file(path: &str) -> NesRomFile {
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

    NesRomFile { header: header,
                 prg_rom: prg_rom,
                 chr_rom: chr_rom,
                 mirroring: mirroring,
                 mapper: mapper}
}


pub struct Cartridge {
    rom: NesRomFile,
}

impl Cartridge {
    pub fn new(rom: NesRomFile) -> Self {
        Cartridge {
            rom: rom,
        }
    }

    pub fn read_mem_cpu(&self, address: u16) -> u8 {
        match self.rom.mapper {
            Mapper::NROM => {
                let mem_address = if self.rom.prg_rom.len() == 16384 {
                    (address - 0x8000) & 0x3FFF
                }
                else {
                    address
                };
                self.rom.prg_rom[mem_address as usize]
            }
            Mapper::CNROM => {
                unimplemented!()
            }
        }
    }

    pub fn write_mem_cpu(&self, address: u16, value: u8) {
        println!("ignoring write {} to addr {:04X}", value, address);
    }

    pub fn read_mem_ppu(&self, address: u16, vram: &[u8]) -> u8 {
        match self.rom.mapper {
            Mapper::NROM => {
                if address < 0x2000 {
                    self.rom.chr_rom[address as usize]
                }
                else if address < 0x3000 {
                    let vram_address = if self.rom.mirroring == MirroringType::Vertical {
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
            Mapper::CNROM => {
                0
            }
        }
    }

    pub fn write_mem_ppu(&self, address: u16, value: u8, vram: &mut [u8]) {
        match self.rom.mapper {
            Mapper::NROM => {
                if address < 0x2000 {
                    //panic!("unexpected address: {:04X}", address);
                }
                else if address < 0x3000 {
                    let vram_address = if self.rom.mirroring == MirroringType::Vertical {
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
            Mapper::CNROM => {

            }
        }
    }
}
