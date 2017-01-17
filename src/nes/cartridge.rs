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
    CNROM { bank: u8 },
}

#[derive(Debug)]
pub struct NesRomFile {
    header: [u8; 16],
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    mirroring: MirroringType,
    mapper_id: u8,
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
    let mapper_id = data[7] & 0xF0 | ((_flags6 & 0xF0) >> 4);

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
                 mapper_id: mapper_id}
}


pub struct Cartridge {
    rom: NesRomFile,
    mapper: Mapper,
}

impl Cartridge {
    pub fn new(rom: NesRomFile) -> Self {
        let mapper = match rom.mapper_id {
            0 => Mapper::NROM,
            3 => Mapper::CNROM{bank: 0},
            _ => { unimplemented!(); },
        };
        Cartridge {
            rom: rom,
            mapper: mapper,
        }
    }

    pub fn read_mem_cpu(&self, address: u16) -> u8 {
        match self.mapper {
            Mapper::NROM | Mapper::CNROM {bank: _} => {
                let mem_address = if self.rom.prg_rom.len() == 16384 {
                    (address - 0x8000) & 0x3FFF
                }
                else {
                    address
                };
                self.rom.prg_rom[mem_address as usize]
            }
        }
    }

    pub fn write_mem_cpu(&mut self, address: u16, value: u8) {
        match self.mapper {
            Mapper::NROM => {
                println!("ignoring write {} to addr {:04X}", value, address);
            }
            Mapper::CNROM {bank:_} => {
                self.mapper = Mapper::CNROM {bank: value};
            }
        }
    }

    pub fn read_mem_ppu(&self, address: u16, vram: &[u8]) -> u8 {
        let chr_rom = match self.mapper {
            Mapper::NROM => { &self.rom.chr_rom }
            Mapper::CNROM{bank} => { &self.rom.chr_rom[bank as usize * 0x2000 ..] }
        };
        if address < 0x2000 {
            chr_rom[address as usize]
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

    pub fn write_mem_ppu(&self, address: u16, value: u8, vram: &mut [u8]) {
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
}
