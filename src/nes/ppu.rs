extern crate sdl2;

use sdl2::render::Renderer;
use sdl2::pixels::Color;
use sdl2::rect::Point;

struct Registers {
    v: u16,
    t: u16,
    x: u8,
    w: bool,
}

pub struct Ppu<'a> {
    pub scan_line: i16,
    pub cycle_count: u16,
    vblank: bool,
    vram_addr_increment: u16,
    gen_nmi_at_vblank: bool,
    pub mem_read_mut_enabled: bool,
    background_enabled: bool,
    sprites_enabled: bool,
    pub memory: Vec<u8>,
    reg: Registers,
    bg_pattern_table_addr: u16,
    renderer: Renderer<'a>,
    colors: Vec<u8>,
}


fn copy_bits(dest: u16, src: u16, mask: u16) -> u16 {
    let tmp = dest & !mask;
    return tmp | (src & mask);
}


impl<'a> Ppu<'a> {
    pub fn new(sdl_context: &mut sdl2::Sdl) -> Ppu<'a> {
        let video_subsystem = sdl_context.video().unwrap();

        let window = video_subsystem.window("nesemu", 256, 240)
            .position_centered()
            .build()
            .unwrap();

        let renderer = window.renderer().build().unwrap();

        Ppu {
            scan_line: 0,
            cycle_count: 0,
            vblank: false,
            vram_addr_increment: 1,
            gen_nmi_at_vblank: false,
            mem_read_mut_enabled: true,
            background_enabled: true,
            sprites_enabled: true,
            memory: vec![0; 0x10000],
            reg: Registers { t: 0, v: 0, x: 0, w: false },
            bg_pattern_table_addr: 0x0000,
            renderer: renderer,
            colors: vec![
                84, 84, 84,     0, 30, 116,     8, 16, 144,     48, 0, 136,
                68, 0, 100,     92, 0, 48,      84, 4, 0,       60, 24, 0,
                32, 42, 0,      8, 58, 0,       0, 64, 0,       0, 60, 0,
                0, 50, 60,      0, 0, 0,        0, 0, 0,        0, 0, 0,
                152, 150, 152,  8, 76, 196,     48, 50, 236,    92, 30, 228,
                136, 20, 176,   160, 20, 100,   152, 34, 32,    120, 60, 0,
                84, 90, 0,      40, 114, 0,     8, 124, 0,      0, 118, 40,
                0, 102, 120,    0, 0, 0,        0, 0, 0,        0, 0, 0,
                236, 238, 236,  76, 154, 236,   120, 124, 236,  176, 98, 236,
                228, 84, 236,   236, 88, 180,   236, 106, 100,  212, 136, 32,
                160, 170, 0,    116, 196, 0,    76, 208, 32,    56, 204, 108,
                56, 180, 204,   60, 60, 60,     0, 0, 0,        0, 0, 0,
                236, 238, 236,  168, 204, 236,  188, 188, 236,  212, 178, 236,
                236, 174, 236,  236, 174, 212,  236, 180, 176,  228, 196, 144,
                204, 210, 120,  180, 222, 120,  168, 226, 144,  152, 226, 180,
                160, 214, 228,  160, 162, 160,  0, 0, 0,        0, 0, 0,
            ],
        }
    }

    pub fn clear(&mut self) {
        self.renderer.set_draw_color(Color::RGB(0, 0, 0));
        self.renderer.clear();
    }

    pub fn present(&mut self) {
        self.renderer.present();
    }

    pub fn load_chr_rom(&mut self, chr_rom: &[u8]) {
        self.memory[0x0000..0x2000].clone_from_slice(&chr_rom);
    }

    #[cfg(test)]
    pub fn set_scan_line(&mut self, scan_line: i16) {
        self.scan_line = scan_line;
    }

    pub fn draw_pixel(&mut self) {
        if !self.background_enabled {
            return;
        }

        let tile_address = 0x2000 | (self.reg.v & 0x0FFF);
        let tile = self.read_mem_ppu(tile_address) as u16;

        let attribute_address = 0x23C0 | (self.reg.v & 0x0C00)
            | ((self.reg.v >> 4) & 0x38) | ((self.reg.v >> 2) & 0x07);
        let attribute = self.read_mem_ppu(attribute_address);

        let fine_y = self.reg.v >> 12;
        let pattern_address_lower = self.bg_pattern_table_addr | (tile << 4) | fine_y;
        let pattern_address_upper = self.bg_pattern_table_addr | (tile << 4) | 0x0008 | fine_y;

        let bitmap_row_lower = self.read_mem_ppu(pattern_address_lower);
        let bitmap_row_upper = self.read_mem_ppu(pattern_address_upper);

        let xoffset = (self.cycle_count % 8) as u8;
        let pattern_bit_lower = bitmap_row_lower & (0x80 >> (self.reg.x + xoffset)) != 0;
        let pattern_bit_upper = bitmap_row_upper & (0x80 >> (self.reg.x + xoffset)) != 0;
        let pattern_bits = (if pattern_bit_upper {2} else {0}) +
            (if pattern_bit_lower {1} else {0});

        let attr_x = self.reg.v & 0x0002 != 0;
        let attr_y = self.reg.v & 0x0040 != 0;

        let palette_bits = if !attr_x && !attr_y {
            attribute & 0x3
        }
        else if attr_x && !attr_y {
            (attribute >> 2) & 0x3
        }
        else if !attr_x && attr_y {
            (attribute >> 4) & 0x3
        }
        else {
            (attribute >> 6) & 0x3
        };

        let palette_index = (palette_bits << 2) | pattern_bits;
        let palette_address = 0x3F00 + (palette_index as u16);
        let color_index = self.read_mem_ppu(palette_address) as usize;
        let red = self.colors[color_index * 3 + 0];
        let green = self.colors[color_index * 3 + 1];
        let blue = self.colors[color_index * 3 + 2];
        self.renderer.set_draw_color(Color::RGB(red, green, blue));
        let x = self.cycle_count as i32;
        let y = self.scan_line as i32;

        if y >= 0 && y < 240 && x < 256 {
            self.renderer.draw_point(Point::new(x, y)).unwrap();
        }
    }

    pub fn step_cycle(&mut self, count: u16) -> bool {
        for _ in 0..count*3 {
            self.cycle_count += 1;
            if self.background_enabled || self.sprites_enabled {
                if self.scan_line == -1 {
                    if self.cycle_count >= 280 && self.cycle_count <= 304 {
                        // copy vertical bits
                        self.reg.v = copy_bits(self.reg.v, self.reg.t, 0x7BE0);
                    }
                }
                else {
                    if self.cycle_count == 256 {
                        // increase y
                        if self.reg.v & 0x7000 != 0x7000 {
                            self.reg.v += 0x1000;
                        }
                        else {
                            self.reg.v &= !0x7000;
                            let mut y = (self.reg.v & 0x03E0) >> 5;
                            if y == 29 {
                                y = 0;
                                self.reg.v ^= 0x0800;
                            }
                            else if y == 31 {
                                y = 0;
                            }
                            else {
                                y += 1;
                            }
                            self.reg.v = (self.reg.v & !0x03E0) | (y << 5);
                        }
                    }
                    else if self.cycle_count == 257 {
                        // copy horizontal bits
                        self.reg.v = copy_bits(self.reg.v, self.reg.t, 0x041F);
                    }
                    else if self.cycle_count < 256 {
                        if self.cycle_count != 0 && self.cycle_count % 8 == 0 {
                            // increase x
                            if self.reg.v & 0x001F == 31 {
                                self.reg.v &= !0x001F;
                                self.reg.v ^= 0x0400;
                            }
                            else {
                                self.reg.v += 1;
                            }
                        }
                    }
                    self.draw_pixel();
                }
            }
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
                    self.reg.w = false;
                }
                value
            }
            0x2007 => {
                let addr = self.reg.v;
                let value = self.read_mem_ppu(addr);
                self.reg.v += self.vram_addr_increment;
                value
            }
            _ => panic!("Unimplemented read address: {:04X}", cpu_address)
        }
    }

    pub fn write_mem(&mut self, cpu_address: u16, value: u8) {
        match cpu_address {
            0x2000 => {
                self.vram_addr_increment = if (value & 0x04) == 0 { 1 } else { 32 };
                self.gen_nmi_at_vblank = (value & 0x80) != 0;
                self.reg.t = copy_bits(self.reg.t, value as u16, 0x0003);
                self.bg_pattern_table_addr = if value & 0x10 != 0 { 0x1000 } else { 0x0000 };
            }
            0x2001 => {
                self.background_enabled = value & 0x08 != 0;
                self.sprites_enabled = value & 0x10 != 0;
            }
            0x2005 => {
                if !self.reg.w {
                    self.reg.t = copy_bits(self.reg.t, (value as u16) >> 3, 0x001F);
                    self.reg.x = value & 0x7;
                }
                else {
                    self.reg.t = copy_bits(self.reg.t, (value as u16) << 12, 0x7000);
                    self.reg.t = copy_bits(self.reg.t, (value as u16) << 2, 0x03E0);
                }
                self.reg.w = !self.reg.w;
            }
            0x2006 => {
                if !self.reg.w {
                    self.reg.t = copy_bits(self.reg.t, (value as u16) << 8, 0x3F00);
                    self.reg.t &= 0x7FFF;
                }
                else {
                    self.reg.t = copy_bits(self.reg.t, value as u16, 0x00FF);
                    self.reg.v = self.reg.t;
                }
                self.reg.w = !self.reg.w;
            }
            0x2007 => {
                let addr = self.reg.v;
                self.write_mem_ppu(addr, value);
                self.reg.v += self.vram_addr_increment;
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
