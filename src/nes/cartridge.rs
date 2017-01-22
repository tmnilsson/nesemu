use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};


#[derive(Debug,PartialEq,Clone,Copy)]
enum MirroringType {
    Horizontal,
    Vertical,
}

#[derive(Debug,Clone)]
enum Mapper {
    NROM,
    MMC1 {
        shift: u8,
        shift_count: u8,
        mirroring: MirroringType,
        prg_swap_range_bit: bool,
        prg_size_bit: bool,
        chr_size_bit: bool,
        chr_bank_0: u8,
        chr_bank_1: u8,
        prg_bank: u8,
        prg_ram: Vec<u8>,
        chr_ram: Option<Vec<u8>>,
    },
    CNROM {
        bank: u8
    },
}

#[derive(Debug)]
struct NesRomFile {
    header: [u8; 16],
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    mirroring: MirroringType,
    has_persistent_ram: bool,
    has_chr_ram: bool,
    mapper_id: u8,
}

pub struct Cartridge {
    nes_path: PathBuf,
    rom: NesRomFile,
    mapper: Mapper,
}

impl NesRomFile {
    fn load(path: &Path) -> Self {
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
        let has_persistent_ram = data[6] & 0x2 != 0;
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
                     has_persistent_ram: has_persistent_ram,
                     has_chr_ram: chr_size == 0,
                     mapper_id: mapper_id}
    }
}

impl Cartridge {
    pub fn load(path: &Path) -> Self {
        if path.extension().unwrap().to_str().unwrap() == "nes" {
            let rom = NesRomFile::load(path);
            let save_path = path.with_extension("sav");
            let mut save_data = vec![0; 8192];
            if rom.has_persistent_ram {
                match File::open(&save_path) {
                    Ok(mut f) => {
                        save_data.clear();
                        f.read_to_end(&mut save_data).expect("Unable to read save data");
                    }
                    Err(_) => {
                    }
                }
            }

            let mapper = match rom.mapper_id {
                0 => Mapper::NROM,
                1 => Mapper::MMC1 {
                    shift: 0,
                    shift_count: 0,
                    mirroring: MirroringType::Vertical,
                    prg_swap_range_bit: true,
                    prg_size_bit: true,
                    chr_size_bit: false,
                    chr_bank_0: 0,
                    chr_bank_1: 0,
                    prg_bank: 0,
                    prg_ram: save_data,
                    chr_ram: if rom.has_chr_ram { Some(vec![0; 8192]) } else { None },
                },
                3 => Mapper::CNROM {
                    bank: 0
                },
                _ => { unimplemented!(); },
            };

            Cartridge {
                nes_path: path.to_path_buf(),
                rom: rom,
                mapper: mapper,
            }
        }
        else {
            unimplemented!();
        }
    }

    pub fn save(&self) {
        if self.rom.has_persistent_ram {
            let save_path = self.nes_path.with_extension("sav");
            match self.mapper {
                Mapper::MMC1 { ref prg_ram, .. } => {
                    let mut f = File::create(&save_path).unwrap();
                    f.write_all(prg_ram).expect("Unable to write save data");
                }
                _ => { panic!("persistent ram not supported"); }
            }
        }
    }

    pub fn read_mem_cpu(&self, address: u16) -> u8 {
        match self.mapper {
            Mapper::NROM | Mapper::CNROM {bank: _} => {
                if address < 0x8000 {
                    0xFF
                }
                else {
                    let mem_address = if self.rom.prg_rom.len() == 16384 {
                        (address - 0x8000) & 0x3FFF
                    }
                    else {
                        address - 0x8000
                    };
                    self.rom.prg_rom[mem_address as usize]
                }
            }
            Mapper::MMC1 {prg_bank, prg_size_bit, prg_swap_range_bit,
                          ref prg_ram, ..} => {
                if address < 0x6000 {
                    0xFF
                }
                else if address < 0x8000 {
                    if prg_bank & 0x10 == 0 {
                        prg_ram[address as usize - 0x6000]
                    }
                    else {
                        0xFF
                    }
                }
                else {
                    let mem_address = if prg_size_bit { // 16KB switching
                        let bank = (prg_bank & 0xF) as u16;
                        let num_banks = (self.rom.prg_rom.len() / 16384) as u16;
                        let (on_lower_bank, bank_offset) = if address >= 0xC000 {
                            (false, address - 0xC000)
                        }
                        else {
                            (true, address - 0x8000)
                        };
                        let effective_bank = if on_lower_bank == prg_swap_range_bit {
                            bank
                        }
                        else if on_lower_bank {
                            0
                        }
                        else {
                            num_banks - 1
                        };
                        effective_bank as usize * 16384 + bank_offset as usize
                    }
                    else { // 32KB switching
                        let bank = ((prg_bank & 0xF) >> 1) as u16;
                        (bank * 32768 + address - 0x8000) as usize
                    };
                    self.rom.prg_rom[mem_address]
                }
            }
        }
    }

    pub fn write_mem_cpu(&mut self, address: u16, value: u8) {
        match self.mapper {
            Mapper::NROM => {
            }
            Mapper::MMC1 {ref mut prg_ram, ref mut shift,
                          ref mut shift_count, ref mut mirroring, ref mut prg_swap_range_bit,
                          ref mut prg_size_bit, ref mut chr_size_bit, ref mut chr_bank_0,
                          ref mut chr_bank_1, ref mut prg_bank, ..} => {
                if address < 0x6000 {
                }
                else if address < 0x8000 {
                    if *prg_bank & 0x10 == 0 {
                        prg_ram[address as usize - 0x6000] = value;
                    }
                }
                else {
                    if value & 0x80 != 0 {
                        *shift = 0;
                        *shift_count = 0;
                    }
                    else {
                        *shift = (*shift >> 1) | (if value & 0x1 != 0 {0x10} else {0});
                        *shift_count += 1;
                        if *shift_count == 5 {
                            let effective_address = 0x8000 | (address & 0x6000);
                            let effective_value = *shift;
                            *shift = 0;
                            *shift_count = 0;
                            if effective_address < 0xA000 {
                                *mirroring = match effective_value & 0x3 {
                                    2 => MirroringType::Vertical,
                                    3 => MirroringType::Horizontal,
                                    _ => unimplemented!(),
                                };
                                *prg_swap_range_bit = effective_value & 0x4 != 0;
                                *prg_size_bit = effective_value & 0x8 != 0;
                                *chr_size_bit = effective_value & 0x10 != 0;
                            }
                            else if effective_address < 0xC000 {
                                *chr_bank_0 = effective_value;
                            }
                            else if effective_address < 0xE000 {
                                *chr_bank_1 = effective_value;
                            }
                            else {
                                *prg_bank = effective_value & 0xF;
                            }
                        }
                    }
                }
            }
            Mapper::CNROM {bank:_} => {
                if address >= 0x8000 {
                    self.mapper = Mapper::CNROM {bank: value};
                }
            }
        }
    }

    fn get_chr_mem_index(address: u16, chr_size_bit: bool,
                         chr_bank_0: u8, chr_bank_1: u8) -> usize {
        if chr_size_bit {
            if address < 0x1000 {
                chr_bank_0 as usize * 0x1000 + address as usize
            }
            else {
                chr_bank_1 as usize * 0x1000 + address as usize - 0x1000
            }
        }
        else {
            (chr_bank_0 >> 1) as usize * 0x2000 + address as usize
        }
    }

    pub fn read_mem_ppu(&self, address: u16, vram: &[u8]) -> u8 {
        if address < 0x2000 {
            match self.mapper {
                Mapper::NROM => {
                    self.rom.chr_rom[address as usize]
                }
                Mapper::MMC1 {chr_size_bit, chr_bank_0, chr_bank_1, ref chr_ram, ..} => {
                    let chr_mem = match *chr_ram {
                        Some(ref ram) => ram,
                        None => &self.rom.chr_rom,
                    };
                    let index = Cartridge::get_chr_mem_index(address, chr_size_bit,
                                                             chr_bank_0, chr_bank_1);
                    chr_mem[index]
                }
                Mapper::CNROM {bank} => {
                    self.rom.chr_rom[bank as usize * 0x2000 + address as usize]
                }
            }
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

    pub fn write_mem_ppu(&mut self, address: u16, value: u8, vram: &mut [u8]) {
        if address < 0x2000 {
            match self.mapper {
                Mapper::NROM | Mapper::CNROM { .. } => {
                    //panic!("unexpected address: {:04X}", address);
                },
                Mapper::MMC1 {ref mut chr_ram, chr_size_bit, chr_bank_0, chr_bank_1, ..} => {
                    match chr_ram.as_mut() {
                        Some(ref mut ram) => {
                            let index = Cartridge::get_chr_mem_index(address, chr_size_bit,
                                                                     chr_bank_0, chr_bank_1);
                            ram[index] = value;
                        }
                        None => {}
                    }
                }
            }
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
