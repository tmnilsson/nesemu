use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::cell::RefCell;

#[derive(Debug)]
struct Registers {
    pc : u16,
    sp : u8,
    a : u8,
    x : u8,
    y : u8,
    status : u8,
}

enum StatusFlag {
    Carry = 0,
    Zero = 1,
    InterruptDisable = 2,
    DecimalMode = 3,
//    BreakCommand = 4,
    Overflow = 6,
    Negative = 7,
}

#[derive(Debug,PartialEq,Copy,Clone)]
enum AddressingMode {
    Accumulator,
    Immediate,
    Relative,
    Absolute,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Implied,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
}

struct Ppu {
    scan_line: i16,
    cycle_count: u16,
    vblank : bool,
    gen_nmi_at_vblank: bool,
    mem_read_mut_enabled: bool,
    ppu_addr: u16,
    memory: Vec<u8>
}

impl Ppu {
    fn new() -> Ppu {
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

    fn step_cycle(&mut self, count: u16) -> bool {
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

    fn read_mem(&mut self, cpu_address: u16) -> u8 {
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

    fn write_mem(&mut self, cpu_address: u16, value: u8) {
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

pub struct Machine {
    reg: Registers,
    memory: Vec<u8>,
    ppu: RefCell<Ppu>,
    nmi_line: bool,
    nmi_triggered: bool,
    instructions: HashMap<u8, Instruction>,
}

#[derive(Debug)]
struct Instruction {
    op_code: u8,
    mnemonic: String,
    addressing_mode: AddressingMode,
}

impl Instruction {
    fn new(op_code: u8, mnemonic: &str,
           addressing_mode: AddressingMode) -> Instruction {
        Instruction { op_code: op_code,
                      mnemonic: mnemonic.to_string(),
                      addressing_mode: addressing_mode }
    }
}

fn set_flag(status: &mut u8, flag: StatusFlag, enabled: bool) {
    if enabled {
        *status |= 1 << flag as u8;
    }
    else {
        *status &= !(1 << flag as u8);
    }
}

impl Machine {
    pub fn new() -> Machine {
        let mut memory = vec![0; 0x10000];
        // Set I/O registers to FF
        for i in 0x4000..0x4020 {
            memory[i] = 0xFF;
        }
        Machine {
            reg: Registers { pc:0, sp:0xfd, a:0, x:0, y:0, status:0x24 },
            memory: memory,
            nmi_line: true,
            nmi_triggered: false,
            ppu: RefCell::new(Ppu::new()),
            instructions: Machine::add_instructions(),
        }
    }

    pub fn load_rom(&mut self, rom: NesRom) {
        if rom.prg_rom.len() == 16384 {
            self.memory[0x8000..0xc000].clone_from_slice(&rom.prg_rom);
            self.memory[0xc000..0x10000].clone_from_slice(&rom.prg_rom);
        }
        self.reset();
    }

    pub fn reset(&mut self) {
        self.perform_interrupt(0xffc, 0xffd, false);
        self.reg.pc = ((self.read_mem(0xfffd) as u16) << 8) +
            self.read_mem(0xfffc) as u16;
    }

    pub fn set_program_counter(&mut self, address: u16) {
        self.reg.pc = address;
    }

    pub fn set_scan_line(&mut self, scan_line: i16) {
        self.ppu.borrow_mut().scan_line = scan_line;
    }

    fn get_status_flag(&mut self, flag: StatusFlag) -> bool {
        self.reg.status & (1 << flag as u8) != 0
    }

    pub fn get_state_string(&self) -> String {
        let reg_str = format!("A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
                              self.reg.a, self.reg.x, self.reg.y,
                              self.reg.status, self.reg.sp);
        let instr_str = self.decode_instruction();
        format!("{:04X}  {}{} CYC:{:3} SL:{}",
                self.reg.pc, instr_str, reg_str,
                self.ppu.borrow().cycle_count, self.ppu.borrow().scan_line)
    }

    fn get_op(&self, op_index: u8) -> u8 {
        self.read_mem(self.reg.pc + op_index as u16)
    }

    fn get_op_u16(&self) -> u16 {
        ((self.get_op(2) as u16) << 8) + self.get_op(1) as u16
    }

    fn decode_instruction(&self) -> String {
        self.ppu.borrow_mut().mem_read_mut_enabled = false;
        let op_code = self.read_mem(self.reg.pc);
        let instr = match self.instructions.get(&op_code) {
            Some(instr) => instr,
            None => { return format!("{:02X}        {:32}", op_code, "<unknown>")},
        };
        let mut code_str = format!("{:02X}", instr.op_code);
        if instr.addressing_mode != AddressingMode::Implied &&
            instr.addressing_mode != AddressingMode::Accumulator {
            code_str += &format!(" {:02X}", self.get_op(1));
        }
        if instr.addressing_mode == AddressingMode::Absolute ||
            instr.addressing_mode == AddressingMode::Indirect ||
            instr.addressing_mode == AddressingMode::AbsoluteX ||
            instr.addressing_mode == AddressingMode::AbsoluteY {
            code_str += &format!(" {:02X}", self.get_op(2));
        }

        let mut disass_str = String::new();
        if !instr.mnemonic.starts_with('*') {
            disass_str += &format!(" ");
        }
        disass_str += &format!("{}", instr.mnemonic);
        match instr.addressing_mode {
            AddressingMode::Accumulator => {
                disass_str += " A";
            }
            AddressingMode::Immediate => {
                disass_str += &format!(" #${:02X}", self.get_op(1));
            },
            AddressingMode::Relative => {
                disass_str += &format!(" ${:04X}", (self.reg.pc as i16 + 2 +
                                                   (self.get_op(1) as i8) as i16) as u16);
            }
            AddressingMode::Absolute => {
                let address = self.get_op_u16();
                disass_str += &format!(" ${:04X}", address);
                if instr.mnemonic != "JMP" && instr.mnemonic != "JSR" {
                    disass_str += &format!(" = {:02X}", self.read_mem(address));
                }
            },
            AddressingMode::ZeroPage => {
                let mem_value = self.read_mem(self.get_op(1) as u16);
                disass_str += &format!(" ${:02X} = {:02X}", self.get_op(1), mem_value);
            },
            AddressingMode::ZeroPageX => {
                let mem_value = self.get_op(1).wrapping_add(self.reg.x) as u16;
                let value = self.read_mem(mem_value);
                disass_str += &format!(" ${:02X},X @ {:02X} = {:02X}",
                                       self.get_op(1), mem_value, value);
            }
            AddressingMode::ZeroPageY => {
                let mem_value = self.get_op(1).wrapping_add(self.reg.y) as u16;
                let value = self.read_mem(mem_value);
                disass_str += &format!(" ${:02X},Y @ {:02X} = {:02X}",
                                       self.get_op(1), mem_value, value);
            }
            AddressingMode::Implied => {
            }
            AddressingMode::AbsoluteX => {
                let address = self.get_op_u16();
                let indirect_address = address.wrapping_add(self.reg.x as u16);
                let value = self.read_mem(indirect_address);
                disass_str += &format!(" ${:04X},X @ {:04X} = {:02X}",
                                       address, indirect_address, value);
            }
            AddressingMode::AbsoluteY => {
                let address = self.get_op_u16();
                let indirect_address = address.wrapping_add(self.reg.y as u16);
                let value = self.read_mem(indirect_address);
                disass_str += &format!(" ${:04X},Y @ {:04X} = {:02X}",
                                       address, indirect_address, value);
            }
            AddressingMode::Indirect => {
                let address = self.get_op_u16();
                let indirect_address_low = self.read_mem(address) as u16;
                let indirect_address_high = self.read_mem(address + 1) as u16;
                let indirect_address = (indirect_address_high << 8) + indirect_address_low;
                disass_str += &format!(" (${:04X}) = {:04X}", address, indirect_address);
            }
            AddressingMode::IndirectX => {
                let address = self.get_op(1) as u16;
                let x = self.reg.x as u16;
                let indirect_address_low = self.read_mem((address + x) & 0xff) as u16;
                let indirect_address_high = self.read_mem((address + x + 1) & 0xff) as u16;
                let indirect_address = (indirect_address_high << 8) + indirect_address_low;
                let value = self.read_mem(indirect_address);
                disass_str += &format!(" (${:02X},X) @ {:02X} = {:04X} = {:02X}",
                                       address, (address + x) & 0xff, indirect_address, value);
            }
            AddressingMode::IndirectY => {
                let address = self.get_op(1) as u16;
                let indirect_address_low = self.read_mem(address) as u16;
                let indirect_address_high = self.read_mem((address + 1) & 0xff) as u16;
                let indirect_address = (indirect_address_high << 8) + indirect_address_low;
                let final_address = indirect_address.wrapping_add(self.reg.y as u16);
                let value = self.read_mem(final_address);
                disass_str += &format!(" (${:02X}),Y = {:04X} @ {:04X} = {:02X}",
                                       address, indirect_address, final_address, value);
            }
        }
        self.ppu.borrow_mut().mem_read_mut_enabled = false;
        let result = format!("{:8} {:33}", code_str, disass_str);
        result
    }

    fn read_mem(&self, address: u16) -> u8 {
        if address >= 0x2000 && address < 0x2008 {
            self.ppu.borrow_mut().read_mem(address)
        }
        else {
            self.memory[address as usize]
        }
    }

    fn write_mem(&mut self, address: u16, value: u8) {
        if address >= 0x2000 && address < 0x2008 {
            self.ppu.borrow_mut().write_mem(address, value);
        }
        else {
            self.memory[address as usize] = value;
        }
    }

    fn step_cycle(&mut self, count: u16) {
        let old_nmi_line = self.nmi_line;
        self.nmi_line = self.ppu.borrow_mut().step_cycle(count);
        if old_nmi_line && !self.nmi_line {
            self.nmi_triggered = true;
        }
    }

    fn step_pc_and_cycle(&mut self, counts: (u16, u16)) {
        let (pc_count, cycle_count) = counts;
        self.reg.pc += pc_count;
        self.step_cycle(cycle_count);
    }

    fn push(&mut self, value: u8) {
        let address = 0x100 + self.reg.sp as u16;
        self.write_mem(address, value);
        self.reg.sp -= 1;
    }

    fn pop(&mut self) -> u8 {
        self.reg.sp += 1;
        self.read_mem(0x100 + self.reg.sp as u16)
    }

    fn branch_immediate(&mut self) {
        let offset = self.get_op(1) as i8;
        self.reg.pc += 2;
        let old_pc = self.reg.pc;
        self.reg.pc = (self.reg.pc as i16 + offset as i16) as u16;
        self.step_cycle(1);
        if (old_pc & 0xFF00) != (self.reg.pc & 0xFF00) {
            self.step_cycle(1);
        }
    }

    fn update_zero_negative(status: &mut u8, value: u8) {
        set_flag(status, StatusFlag::Zero, value == 0);
        set_flag(status, StatusFlag::Negative, value & 0x80 != 0);
    }
    
    fn add_instructions() -> HashMap<u8, Instruction>
    {
        let mut instructions = HashMap::new();

        {
            let mut add = |op_code: u8, mnemonic: &str, addressing_mode: AddressingMode| {
                instructions.insert(op_code, Instruction::new(op_code, mnemonic,
                                                              addressing_mode));
            };

            add(0x01, "ORA", AddressingMode::IndirectX);
            add(0x03, "*SLO", AddressingMode::IndirectX);
            add(0x04, "*NOP", AddressingMode::ZeroPage);
            add(0x05, "ORA", AddressingMode::ZeroPage);
            add(0x06, "ASL", AddressingMode::ZeroPage);
            add(0x07, "*SLO", AddressingMode::ZeroPage);
            add(0x08, "PHP", AddressingMode::Implied);
            add(0x09, "ORA", AddressingMode::Immediate);
            add(0x0A, "ASL", AddressingMode::Accumulator);
            add(0x0C, "*NOP", AddressingMode::Absolute);
            add(0x0D, "ORA", AddressingMode::Absolute);
            add(0x0E, "ASL", AddressingMode::Absolute);
            add(0x0F, "*SLO", AddressingMode::Absolute);
            add(0x10, "BPL", AddressingMode::Relative);
            add(0x11, "ORA", AddressingMode::IndirectY);
            add(0x13, "*SLO", AddressingMode::IndirectY);
            add(0x14, "*NOP", AddressingMode::ZeroPageX);
            add(0x15, "ORA", AddressingMode::ZeroPageX);
            add(0x16, "ASL", AddressingMode::ZeroPageX);
            add(0x17, "*SLO", AddressingMode::ZeroPageX);
            add(0x18, "CLC", AddressingMode::Implied);
            add(0x19, "ORA", AddressingMode::AbsoluteY);
            add(0x1A, "*NOP", AddressingMode::Implied);
            add(0x1B, "*SLO", AddressingMode::AbsoluteY);
            add(0x1C, "*NOP", AddressingMode::AbsoluteX);
            add(0x1D, "ORA", AddressingMode::AbsoluteX);
            add(0x1E, "ASL", AddressingMode::AbsoluteX);
            add(0x1F, "*SLO", AddressingMode::AbsoluteX);
            add(0x20, "JSR", AddressingMode::Absolute);
            add(0x21, "AND", AddressingMode::IndirectX);
            add(0x23, "*RLA", AddressingMode::IndirectX);
            add(0x25, "AND", AddressingMode::ZeroPage);
            add(0x27, "*RLA", AddressingMode::ZeroPage);
            add(0x28, "PLP", AddressingMode::Implied);
            add(0x24, "BIT", AddressingMode::ZeroPage);
            add(0x26, "ROL", AddressingMode::ZeroPage);
            add(0x29, "AND", AddressingMode::Immediate);
            add(0x2A, "ROL", AddressingMode::Accumulator);
            add(0x2C, "BIT", AddressingMode::Absolute);
            add(0x2D, "AND", AddressingMode::Absolute);
            add(0x2E, "ROL", AddressingMode::Absolute);
            add(0x2F, "*RLA", AddressingMode::Absolute);
            add(0x30, "BMI", AddressingMode::Relative);
            add(0x31, "AND", AddressingMode::IndirectY);
            add(0x33, "*RLA", AddressingMode::IndirectY);
            add(0x34, "*NOP", AddressingMode::ZeroPageX);
            add(0x35, "AND", AddressingMode::ZeroPageX);
            add(0x36, "ROL", AddressingMode::ZeroPageX);
            add(0x37, "*RLA", AddressingMode::ZeroPageX);
            add(0x38, "SEC", AddressingMode::Implied);
            add(0x39, "AND", AddressingMode::AbsoluteY);
            add(0x3A, "*NOP", AddressingMode::Implied);
            add(0x3B, "*RLA", AddressingMode::AbsoluteY);
            add(0x3C, "*NOP", AddressingMode::AbsoluteX);
            add(0x3D, "AND", AddressingMode::AbsoluteX);
            add(0x3E, "ROL", AddressingMode::AbsoluteX);
            add(0x3F, "*RLA", AddressingMode::AbsoluteX);
            add(0x40, "RTI", AddressingMode::Implied);
            add(0x41, "EOR", AddressingMode::IndirectX);
            add(0x43, "*SRE", AddressingMode::IndirectX);
            add(0x44, "*NOP", AddressingMode::ZeroPage);
            add(0x45, "EOR", AddressingMode::ZeroPage);
            add(0x46, "LSR", AddressingMode::ZeroPage);
            add(0x47, "*SRE", AddressingMode::ZeroPage);
            add(0x48, "PHA", AddressingMode::Implied);
            add(0x49, "EOR", AddressingMode::Immediate);
            add(0x4A, "LSR", AddressingMode::Accumulator);
            add(0x4C, "JMP", AddressingMode::Absolute);
            add(0x4D, "EOR", AddressingMode::Absolute);
            add(0x4E, "LSR", AddressingMode::Absolute);
            add(0x4F, "*SRE", AddressingMode::Absolute);
            add(0x50, "BVC", AddressingMode::Relative);
            add(0x51, "EOR", AddressingMode::IndirectY);
            add(0x53, "*SRE", AddressingMode::IndirectY);
            add(0x54, "*NOP", AddressingMode::ZeroPageX);
            add(0x55, "EOR", AddressingMode::ZeroPageX);
            add(0x56, "LSR", AddressingMode::ZeroPageX);
            add(0x57, "*SRE", AddressingMode::ZeroPageX);
            add(0x59, "EOR", AddressingMode::AbsoluteY);
            add(0x5A, "*NOP", AddressingMode::Implied);
            add(0x5B, "*SRE", AddressingMode::AbsoluteY);
            add(0x5C, "*NOP", AddressingMode::AbsoluteX);
            add(0x5D, "EOR", AddressingMode::AbsoluteX);
            add(0x5E, "LSR", AddressingMode::AbsoluteX);
            add(0x5F, "*SRE", AddressingMode::AbsoluteX);
            add(0x60, "RTS", AddressingMode::Implied);
            add(0x61, "ADC", AddressingMode::IndirectX);
            add(0x63, "*RRA", AddressingMode::IndirectX);
            add(0x64, "*NOP", AddressingMode::ZeroPage);
            add(0x65, "ADC", AddressingMode::ZeroPage);
            add(0x66, "ROR", AddressingMode::ZeroPage);
            add(0x67, "*RRA", AddressingMode::ZeroPage);
            add(0x68, "PLA", AddressingMode::Implied);
            add(0x69, "ADC", AddressingMode::Immediate);
            add(0x6A, "ROR", AddressingMode::Accumulator); 
            add(0x6C, "JMP", AddressingMode::Indirect);
            add(0x6D, "ADC", AddressingMode::Absolute); 
            add(0x6E, "ROR", AddressingMode::Absolute); 
            add(0x6F, "*RRA", AddressingMode::Absolute);
            add(0x70, "BVS", AddressingMode::Relative);
            add(0x71, "ADC", AddressingMode::IndirectY);
            add(0x73, "*RRA", AddressingMode::IndirectY);
            add(0x74, "*NOP", AddressingMode::ZeroPageX);
            add(0x75, "ADC", AddressingMode::ZeroPageX);
            add(0x76, "ROR", AddressingMode::ZeroPageX);
            add(0x77, "*RRA", AddressingMode::ZeroPageX);
            add(0x78, "SEI", AddressingMode::Implied);
            add(0x79, "ADC", AddressingMode::AbsoluteY); 
            add(0x7A, "*NOP", AddressingMode::Implied);
            add(0x7B, "*RRA", AddressingMode::AbsoluteY);
            add(0x7C, "*NOP", AddressingMode::AbsoluteX);
            add(0x7D, "ADC", AddressingMode::AbsoluteX); 
            add(0x7E, "ROR", AddressingMode::AbsoluteX); 
            add(0x7F, "*RRA", AddressingMode::AbsoluteX);
            add(0x80, "*NOP", AddressingMode::Immediate);
            add(0x81, "STA", AddressingMode::IndirectX);
            add(0x83, "*SAX", AddressingMode::IndirectX);
            add(0x84, "STY", AddressingMode::ZeroPage);
            add(0x85, "STA", AddressingMode::ZeroPage);
            add(0x86, "STX", AddressingMode::ZeroPage);
            add(0x87, "*SAX", AddressingMode::ZeroPage);
            add(0x88, "DEY", AddressingMode::Implied);
            add(0x8A, "TXA", AddressingMode::Implied);
            add(0x8C, "STY", AddressingMode::Absolute);
            add(0x8D, "STA", AddressingMode::Absolute);
            add(0x8E, "STX", AddressingMode::Absolute);
            add(0x8F, "*SAX", AddressingMode::Absolute);
            add(0x90, "BCC", AddressingMode::Relative);
            add(0x91, "STA", AddressingMode::IndirectY);
            add(0x94, "STY", AddressingMode::ZeroPageX);
            add(0x95, "STA", AddressingMode::ZeroPageX);
            add(0x96, "STX", AddressingMode::ZeroPageY);
            add(0x97, "*SAX", AddressingMode::ZeroPageY);
            add(0x98, "TYA", AddressingMode::Implied);
            add(0x99, "STA", AddressingMode::AbsoluteY);
            add(0x9A, "TXS", AddressingMode::Implied);
            add(0x9D, "STA", AddressingMode::AbsoluteX);
            add(0xA0, "LDY", AddressingMode::Immediate);
            add(0xA1, "LDA", AddressingMode::IndirectX);
            add(0xA2, "LDX", AddressingMode::Immediate);
            add(0xA3, "*LAX", AddressingMode::IndirectX);
            add(0xA4, "LDY", AddressingMode::ZeroPage);
            add(0xA5, "LDA", AddressingMode::ZeroPage);
            add(0xA6, "LDX", AddressingMode::ZeroPage);
            add(0xA7, "*LAX", AddressingMode::ZeroPage);
            add(0xA8, "TAY", AddressingMode::Implied);
            add(0xA9, "LDA", AddressingMode::Immediate);
            add(0xAA, "TAX", AddressingMode::Implied); 
            add(0xAC, "LDY", AddressingMode::Absolute);
            add(0xAD, "LDA", AddressingMode::Absolute);
            add(0xAE, "LDX", AddressingMode::Absolute);
            add(0xAF, "*LAX", AddressingMode::Absolute);
            add(0xB0, "BCS", AddressingMode::Relative);
            add(0xB1, "LDA", AddressingMode::IndirectY);
            add(0xB3, "*LAX", AddressingMode::IndirectY);
            add(0xB4, "LDY", AddressingMode::ZeroPageX);
            add(0xB5, "LDA", AddressingMode::ZeroPageX);
            add(0xB6, "LDX", AddressingMode::ZeroPageY);
            add(0xB7, "*LAX", AddressingMode::ZeroPageY);
            add(0xB8, "CLV", AddressingMode::Implied);
            add(0xB9, "LDA", AddressingMode::AbsoluteY);
            add(0xBA, "TSX", AddressingMode::Implied);
            add(0xBC, "LDY", AddressingMode::AbsoluteX);
            add(0xBD, "LDA", AddressingMode::AbsoluteX);
            add(0xBE, "LDX", AddressingMode::AbsoluteY);
            add(0xBF, "*LAX", AddressingMode::AbsoluteY);
            add(0xC0, "CPY", AddressingMode::Immediate);
            add(0xC1, "CMP", AddressingMode::IndirectX);
            add(0xC3, "*DCP", AddressingMode::IndirectX);
            add(0xC4, "CPY", AddressingMode::ZeroPage);
            add(0xC5, "CMP", AddressingMode::ZeroPage);
            add(0xC6, "DEC", AddressingMode::ZeroPage);
            add(0xC7, "*DCP", AddressingMode::ZeroPage);
            add(0xC8, "INY", AddressingMode::Implied);
            add(0xC9, "CMP", AddressingMode::Immediate);
            add(0xCA, "DEX", AddressingMode::Implied); 
            add(0xCC, "CPY", AddressingMode::Absolute);
            add(0xCD, "CMP", AddressingMode::Absolute);
            add(0xCE, "DEC", AddressingMode::Absolute);
            add(0xCF, "*DCP", AddressingMode::Absolute);
            add(0xD0, "BNE", AddressingMode::Relative);
            add(0xD1, "CMP", AddressingMode::IndirectY);
            add(0xD3, "*DCP", AddressingMode::IndirectY);
            add(0xD4, "*NOP", AddressingMode::ZeroPageX);
            add(0xD5, "CMP", AddressingMode::ZeroPageX);
            add(0xD6, "DEC", AddressingMode::ZeroPageX);
            add(0xD7, "*DCP", AddressingMode::ZeroPageX);
            add(0xD8, "CLD", AddressingMode::Implied);
            add(0xD9, "CMP", AddressingMode::AbsoluteY);
            add(0xDA, "*NOP", AddressingMode::Implied);
            add(0xDB, "*DCP", AddressingMode::AbsoluteY);
            add(0xDC, "*NOP", AddressingMode::AbsoluteX);
            add(0xDD, "CMP", AddressingMode::AbsoluteX);
            add(0xDE, "DEC", AddressingMode::AbsoluteX);
            add(0xDF, "*DCP", AddressingMode::AbsoluteX);
            add(0xE0, "CPX", AddressingMode::Immediate);
            add(0xE1, "SBC", AddressingMode::IndirectX);
            add(0xE3, "*ISB", AddressingMode::IndirectX);
            add(0xE4, "CPX", AddressingMode::ZeroPage);
            add(0xE5, "SBC", AddressingMode::ZeroPage);
            add(0xE6, "INC", AddressingMode::ZeroPage);
            add(0xE7, "*ISB", AddressingMode::ZeroPage);
            add(0xE8, "INX", AddressingMode::Implied);
            add(0xE9, "SBC", AddressingMode::Immediate);
            add(0xEA, "NOP", AddressingMode::Implied);
            add(0xEB, "*SBC", AddressingMode::Immediate);
            add(0xEC, "CPX", AddressingMode::Absolute);
            add(0xED, "SBC", AddressingMode::Absolute);
            add(0xEE, "INC", AddressingMode::Absolute);
            add(0xEF, "*ISB", AddressingMode::Absolute);
            add(0xF0, "BEQ", AddressingMode::Relative);
            add(0xF1, "SBC", AddressingMode::IndirectY);
            add(0xF3, "*ISB", AddressingMode::IndirectY);
            add(0xF4, "*NOP", AddressingMode::ZeroPageX);
            add(0xF5, "SBC", AddressingMode::ZeroPageX);
            add(0xF6, "INC", AddressingMode::ZeroPageX);
            add(0xF7, "*ISB", AddressingMode::ZeroPageX);
            add(0xF8, "SED", AddressingMode::Implied);
            add(0xF9, "SBC", AddressingMode::AbsoluteY);
            add(0xFA, "*NOP", AddressingMode::Implied);
            add(0xFB, "*ISB", AddressingMode::AbsoluteY);
            add(0xFC, "*NOP", AddressingMode::AbsoluteX);
            add(0xFD, "SBC", AddressingMode::AbsoluteX);
            add(0xFE, "INC", AddressingMode::AbsoluteX);
            add(0xFF, "*ISB", AddressingMode::AbsoluteX);
        }
        instructions
    }

    fn get_address(&self, addr_mode: AddressingMode) -> (u16, u16) {
        match addr_mode {
            AddressingMode::ZeroPage => {
                (self.get_op(1) as u16, 0)
            }
            AddressingMode::ZeroPageX => {
                (self.get_op(1).wrapping_add(self.reg.x) as u16, 0)
            }
            AddressingMode::ZeroPageY => {
                (self.get_op(1).wrapping_add(self.reg.y) as u16, 0)
            }
            AddressingMode::Absolute => {
                (self.get_op_u16(), 0)
            }
            AddressingMode::AbsoluteX => {
                let address = self.get_op_u16();
                let oops = (address & 0xFF) + self.reg.x as u16 > 255;
                (address.wrapping_add(self.reg.x as u16), if oops {1} else {0})
            }
            AddressingMode::AbsoluteY => {
                let address = self.get_op_u16();
                let oops = (address & 0xFF) + self.reg.y as u16 > 255;
                (address.wrapping_add(self.reg.y as u16), if oops {1} else {0})
            }
            AddressingMode::Indirect => {
                let address = self.get_op_u16();
                let indirect_address_low = self.read_mem(address) as u16;
                let indirect_address_high = if (address & 0xFF) == 0xFF {
                    self.read_mem(address + 1 - 0x100) as u16
                }
                else {
                    self.read_mem(address + 1) as u16
                };
                let indirect_address = (indirect_address_high << 8) + indirect_address_low;
                (indirect_address, 0)
            }
            AddressingMode::IndirectX => {
                let address = self.get_op(1) as u16 + self.reg.x as u16;
                let indirect_address_low = self.read_mem(address & 0xff) as u16;
                let indirect_address_high = self.read_mem((address + 1) & 0xff) as u16;
                let indirect_address = (indirect_address_high << 8) + indirect_address_low;
                (indirect_address, 0)
            }
            AddressingMode::IndirectY => {
                let address = self.get_op(1) as u16;
                let indirect_address_low = self.read_mem(address) as u16;
                let indirect_address_high = self.read_mem((address + 1) & 0xff) as u16;
                let indirect_address = (indirect_address_high << 8) + indirect_address_low;
                let final_address = indirect_address.wrapping_add(self.reg.y as u16);
                let oops = (self.reg.y as u16).wrapping_add(indirect_address & 0xFF) > 255;
                (final_address, if oops {1} else {0})
            }
            _ => { panic!("Unsupported addressing mode"); }
        }
    }

    fn get_byte(&self, addr_mode: AddressingMode) -> (u8, u16) {
        match addr_mode {
            AddressingMode::Implied => {
                (0, 0)
            }
            AddressingMode::Accumulator => {
                (self.reg.a, 0)
            }
            AddressingMode::Immediate => {
                (self.get_op(1), 0)
            }
            AddressingMode::Absolute |
            AddressingMode::ZeroPage |
            AddressingMode::ZeroPageX |
            AddressingMode::ZeroPageY |
            AddressingMode::AbsoluteX |
            AddressingMode::AbsoluteY |
            AddressingMode::IndirectX |
            AddressingMode::IndirectY => {
                let (address, oops) = self.get_address(addr_mode);
                (self.read_mem(address), oops)
            }
            _ => { panic!("Unsupported addressing mode"); }
        }
    }

    fn set_byte(&mut self, addr_mode: AddressingMode, value: u8) {
        match addr_mode {
            AddressingMode::Accumulator => {
                self.reg.a = value;
            }
            AddressingMode::Absolute |
            AddressingMode::AbsoluteX |
            AddressingMode::AbsoluteY |
            AddressingMode::ZeroPage |
            AddressingMode::ZeroPageX |
            AddressingMode::ZeroPageY |
            AddressingMode::IndirectX |
            AddressingMode::IndirectY => {
                let (address, _) = self.get_address(addr_mode);
                self.write_mem(address, value);
            }
            _ => { panic!("Unsupported addressing mode"); }
        }
    }

    fn compute_sbc(&mut self, a: u8, m: u8) {
        let not_c = if self.get_status_flag(StatusFlag::Carry) {0} else {1};
        let result = (a as u16).wrapping_sub(m as u16).wrapping_sub(not_c);
        let ac = (a & 0xFF) as u8;
        let result_u8 = result as u8;
        let overflow = ((ac ^ result_u8) & 0x80 != 0) &&
            ((ac ^ (m as u8)) & 0x80 != 0);
        self.reg.a = (result & 0xFF) as u8;
        set_flag(&mut self.reg.status, StatusFlag::Overflow, overflow);
        set_flag(&mut self.reg.status, StatusFlag::Carry, result < 0x100);
        Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
    }

    fn compute_adc(&mut self, a: u8, m: u8) {
        let carry : u16 = if self.get_status_flag(StatusFlag::Carry) {1} else {0};
        let result = a as u16 + m as u16 + carry;
        set_flag(&mut self.reg.status, StatusFlag::Carry, result > 255);
        let overflow = (a & 0x80 != 0 && m & 0x80 != 0 &&
                        result & 0x80 == 0) ||
            (a & 0x80 == 0 && m & 0x80 == 0 &&
             result & 0x80 != 0);
        set_flag(&mut self.reg.status, StatusFlag::Overflow, overflow);
        self.reg.a = (result & 0xFF) as u8;
        Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
    }

    pub fn execute(&mut self) {
        if self.nmi_triggered {
            self.nmi_triggered = false;
            self.perform_interrupt(0xfffa, 0xfffb, true);
        }
        else {
            self.execute_instruction();
        }
    }

    fn perform_interrupt(&mut self, pcl_addr: u16, pch_addr: u16, write_to_stack: bool) {
        println!("performing interrupt!");
        if write_to_stack {
            let pch = (self.reg.pc >> 8) as u8;
            let pcl = (self.reg.pc & 0xff) as u8;
            self.push(pch);
            self.push(pcl);
            let status = self.reg.status;
            self.push(status);
        }
        let pch = self.read_mem(pch_addr) as u16;
        let pcl = self.read_mem(pcl_addr) as u16;
        let new_pc = (pch << 8) + pcl;
        self.reg.pc = new_pc;
    }

    fn execute_instruction(&mut self) {
        let op_code = self.read_mem(self.reg.pc);
        let addr_mode = self.instructions.get(&op_code).unwrap().addressing_mode.clone();
        match op_code {
            0x01 | 0x05 | 0x09 | 0x0D | 0x11 | 0x15 | 0x19 | 0x1D => { // ORA
                let (value, oops) = self.get_byte(addr_mode);
                self.reg.a = self.reg.a | value;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 4 + oops),
                    AddressingMode::IndirectX => (2, 6),
                    AddressingMode::IndirectY => (2, 5 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x03 | 0x07 | 0x0F | 0x13 | 0x17 | 0x1B | 0x1F => { // *SLO
                let (mut value, oops) = self.get_byte(addr_mode);
                let carry = value & 0x80 != 0;
                value <<= 1;
                set_flag(&mut self.reg.status, StatusFlag::Carry, carry);
                self.reg.a = self.reg.a | value;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
                self.set_byte(addr_mode, value);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 6 + oops),
                    AddressingMode::IndirectX => (2, 8),
                    AddressingMode::IndirectY => (2, 7 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x06 | 0x0A | 0x0E | 0x16 | 0x1E => { // ASL
                let mut value = self.get_byte(addr_mode).0;
                let carry = value & 0x80 != 0;
                value <<= 1;
                set_flag(&mut self.reg.status, StatusFlag::Carry, carry);
                Machine::update_zero_negative(&mut self.reg.status, value);
                self.set_byte(addr_mode, value);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Accumulator => (1, 2),
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX => (3, 7),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x08 => { // PHP
                let value = self.reg.status | 0x10; // Bit 4 should be set to one
                self.push(value);
                self.reg.pc += 1;
                self.step_cycle(3);
            }
            0x10 => { // BPL
                if !self.get_status_flag(StatusFlag::Negative) {
                    self.branch_immediate();
                }
                else {
                    self.reg.pc += 2;
                }
                self.step_cycle(2);
            }
            0x18 => { // CLC
                set_flag(&mut self.reg.status, StatusFlag::Carry, false);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0x20 => { // JSR
                let return_addr = self.reg.pc + 2;
                self.push((return_addr >> 8) as u8);
                self.push((return_addr & 0xFF) as u8);
                let new_pc =
                    self.get_op(2) as u16 * 256 + self.get_op(1) as u16;
                self.reg.pc = new_pc;
                self.step_cycle(6);
            }
            0x24 | 0x2C => { // BIT
                let value = self.get_byte(addr_mode).0;
                let mask = self.reg.a & value;
                set_flag(&mut self.reg.status, StatusFlag::Zero, mask == 0);
                set_flag(&mut self.reg.status, StatusFlag::Overflow, value & 0x40 != 0);
                set_flag(&mut self.reg.status, StatusFlag::Negative, value & 0x80 != 0);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::Absolute => (3, 4),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x28 => { // PLP
                // Bit 4 and 5 in status register should not be changed
                let value = self.pop() & 0xCF; // Clear bit 4 and 5
                self.reg.status &= 0x30; // Clear all, except bit 4 and 5
                self.reg.status |= value; // Copy all, except bit 4 and 5
                self.reg.pc += 1;
                self.step_cycle(4);
            }
            0x21 | 0x25 | 0x29 | 0x2D | 0x31 | 0x35 | 0x39 | 0x3D => { // AND
                let (value, oops) = self.get_byte(addr_mode);
                self.reg.a = self.reg.a & value;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 4 + oops),
                    AddressingMode::IndirectX => (2, 6),
                    AddressingMode::IndirectY => (2, 5 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x23 | 0x27 | 0x2F | 0x33 | 0x37 | 0x3B | 0x3F => { // *RLA
                let (mut value, oops) = self.get_byte(addr_mode);
                let new_carry = value & 0x80 != 0;
                value <<= 1;
                if self.get_status_flag(StatusFlag::Carry) {
                    value |= 0x01;
                }
                set_flag(&mut self.reg.status, StatusFlag::Carry, new_carry);
                self.set_byte(addr_mode, value);
                self.reg.a = self.reg.a & value;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 6 + oops),
                    AddressingMode::IndirectX => (2, 8),
                    AddressingMode::IndirectY => (2, 7 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x26 | 0x2A | 0x2E | 0x36 | 0x3E => { // ROL
                let mut value = self.get_byte(addr_mode).0;
                let new_carry = value & 0x80 != 0;
                value <<= 1;
                if self.get_status_flag(StatusFlag::Carry) {
                    value |= 0x01;
                }
                set_flag(&mut self.reg.status, StatusFlag::Carry, new_carry);
                Machine::update_zero_negative(&mut self.reg.status, value);
                self.set_byte(addr_mode, value);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Accumulator => (1, 2),
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX => (3, 7),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x30 => { // BMI
                if self.get_status_flag(StatusFlag::Negative) {
                    self.branch_immediate();
                }
                else {
                    self.reg.pc += 2;
                }
                self.step_cycle(2);
            }
            0x38 => { // SEC
                set_flag(&mut self.reg.status, StatusFlag::Carry, true);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0x40 => { // RTI
                // Ignore bit 4 and 5
                let status = self.pop() & 0xCF;
                self.reg.status &= 0x30;
                self.reg.status |= status;
                let pcl = self.pop() as u16;
                let pch = self.pop() as u16;
                self.reg.pc = (pch << 8) + pcl;
                self.step_cycle(6);
            }
            0x48 => { // PHA
                let value = self.reg.a;
                self.push(value);
                self.reg.pc += 1;
                self.step_cycle(3);
            }
            0x4C | 0x6C => { // JMP
                let new_pc = self.get_address(addr_mode).0;
                self.reg.pc = new_pc;
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Absolute => (0, 3),
                    AddressingMode::Indirect => (0, 5),
                    _ => panic!("Unexpected addressing mode"),
                    })
            }
            0x41 | 0x45 | 0x49 | 0x4D | 0x51 | 0x55 | 0x59 | 0x5D => { // EOR
                let (value, oops) = self.get_byte(addr_mode);
                self.reg.a = self.reg.a ^ value;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 4 + oops),
                    AddressingMode::IndirectX => (2, 6),
                    AddressingMode::IndirectY => (2, 5 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x43 | 0x47 | 0x4F | 0x53 | 0x57 | 0x5B | 0x5F => { // *SRE
                let (mut value, oops) = self.get_byte(addr_mode);
                let carry = value & 0x01 != 0;
                value >>= 1;
                set_flag(&mut self.reg.status, StatusFlag::Carry, carry);
                self.set_byte(addr_mode, value);
                self.reg.a = self.reg.a ^ value;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 6 + oops),
                    AddressingMode::IndirectX => (2, 8),
                    AddressingMode::IndirectY => (2, 7 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x46 | 0x4A | 0x4E | 0x56 | 0x5E => { // LSR
                let mut value = self.get_byte(addr_mode).0;
                let carry = value & 0x01 != 0;
                value >>= 1;
                set_flag(&mut self.reg.status, StatusFlag::Carry, carry);
                Machine::update_zero_negative(&mut self.reg.status, value);
                self.set_byte(addr_mode, value);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Accumulator => (1, 2),
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX => (3, 7),
                    _ => panic!("Unexpected addressing mode"),
                    })
            }
            0x50 => { // BVC
                if !self.get_status_flag(StatusFlag::Overflow) {
                    self.branch_immediate();
                }
                else {
                    self.reg.pc += 2;
                }
                self.step_cycle(2);
            }
            0x60 => { // RTS
                let low = self.pop() as u16;
                let high = self.pop() as u16;
                let return_addr = (high << 8) + low;
                self.reg.pc = return_addr + 1;
                self.step_cycle(6);
            }
            0x68 => { // PLA
                self.reg.a = self.pop();
                Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
                self.reg.pc += 1;
                self.step_cycle(4);
            }
            0x61 | 0x65 | 0x69 | 0x6D | 0x71 | 0x75 | 0x79 | 0x7D => { // ADC
                let a = self.reg.a;
                let (m, oops) = self.get_byte(addr_mode);
                self.compute_adc(a, m);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 4 + oops),
                    AddressingMode::IndirectX => (2, 6),
                    AddressingMode::IndirectY => (2, 5 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x63 | 0x67 | 0x6F | 0x73 | 0x77 | 0x7B | 0x7F => { // *RRA
                let (mut value, oops) = self.get_byte(addr_mode);
                let new_carry = value & 0x01 != 0;
                value >>= 1;
                if self.get_status_flag(StatusFlag::Carry) {
                    value |= 0x80;
                }
                set_flag(&mut self.reg.status, StatusFlag::Carry, new_carry);
                self.set_byte(addr_mode, value);
                let a = self.reg.a;
                self.compute_adc(a, value);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 6 + oops),
                    AddressingMode::IndirectX => (2, 8),
                    AddressingMode::IndirectY => (2, 7 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x66 | 0x6A | 0x6E | 0x76 | 0x7E => { // ROR
                let mut value = self.get_byte(addr_mode).0;
                let new_carry = value & 0x01 != 0;
                value >>= 1;
                if self.get_status_flag(StatusFlag::Carry) {
                    value |= 0x80;
                }
                set_flag(&mut self.reg.status, StatusFlag::Carry, new_carry);
                Machine::update_zero_negative(&mut self.reg.status, value);
                self.set_byte(addr_mode, value);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Accumulator => (1, 2),
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX => (3, 7),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x70 => { // BVS
                if self.get_status_flag(StatusFlag::Overflow) {
                    self.branch_immediate();
                }
                else {
                    self.reg.pc += 2;
                }
                self.step_cycle(2);
            }
            0x78 => { // SEI
                set_flag(&mut self.reg.status, StatusFlag::InterruptDisable, true);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0x81 | 0x85 | 0x8D | 0x91 | 0x95 | 0x99 | 0x9D => { // STA
                let (addr, _) = self.get_address(addr_mode);
                let value = self.reg.a;
                self.write_mem(addr, value);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 5),
                    AddressingMode::IndirectX => (2, 6),
                    AddressingMode::IndirectY => (2, 6),
                    _ => panic!("Unexpected addressing mode"),
                    })
            }
            0x83 | 0x87 | 0x8F | 0x97 => { // *SAX
                let (addr, _) = self.get_address(addr_mode);
                let  value = self.reg.a & self.reg.x;
                self.write_mem(addr, value); 
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageY => (2, 4),
                    AddressingMode::Absolute => (3, 4),
//                    AddressingMode::AbsoluteX |
//                    AddressingMode::AbsoluteY => (3, 5),
                    AddressingMode::IndirectX => (2, 6),
//                    AddressingMode::IndirectY => (2, 6),
                    _ => panic!("Unexpected addressing mode"),
                    })
            }
            0x84 | 0x8C | 0x94 => { // STY
                let (addr, _) = self.get_address(addr_mode);
                let value = self.reg.y;
                self.write_mem(addr, value);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    _ => panic!("Unexpected addressing mode"),
                    })
            }
            0x86 | 0x8E | 0x96 => { // STX
                let (addr, _) = self.get_address(addr_mode);
                let value = self.reg.x;
                self.write_mem(addr, value);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageY => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    _ => panic!("Unexpected addressing mode"),
                    })
            }
            0x88 => { // DEY
                self.reg.y = self.reg.y.wrapping_sub(1);
                Machine::update_zero_negative(&mut self.reg.status, self.reg.y);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0x8A => { // TXA
                self.reg.a = self.reg.x;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0x90 => { // BCC
                if !self.get_status_flag(StatusFlag::Carry) {
                    self.branch_immediate();
                }
                else {
                    self.reg.pc += 2;
                }
                self.step_cycle(2);
            }
            0x98 => { // TYA
                self.reg.a = self.reg.y;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0x9A => { // TXS
                self.reg.sp = self.reg.x;
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0xA0 | 0xA4 | 0xAC | 0xB4 | 0xBC => { // LDY
                let (value, oops) = self.get_byte(addr_mode);
                self.reg.y = value;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.y);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteX => (3, 4 + oops),
                    _ => unreachable!(),
                    });
            }
            0xA2 | 0xA6 | 0xAE | 0xB6 | 0xBE => { // LDX
                let (value, oops) = self.get_byte(addr_mode);
                self.reg.x = value;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.x);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageY => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteY => (3, 4 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xA3 | 0xA7 | 0xAF | 0xB3 | 0xB7 | 0xBF => { // *LAX
                let (value, oops) = self.get_byte(addr_mode);
                self.reg.a = value;
                self.reg.x = value;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.x);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageY => (2, 4),
                    AddressingMode::IndirectX => (2, 6),
                    AddressingMode::IndirectY => (2, 5 + oops),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteY => (3, 4 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xA8 => { // TAY
                self.reg.y = self.reg.a;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.y);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0xA1 | 0xA5 | 0xA9 | 0xAD | 0xB1 | 0xB5 | 0xB9 | 0xBD => { // LDA
                let (value, oops) = self.get_byte(addr_mode);
                self.reg.a = value;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.a);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 4 + oops),
                    AddressingMode::IndirectX => (2, 6),
                    AddressingMode::IndirectY => (2, 5 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xAA => { // TAX
                self.reg.x = self.reg.a;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.x);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0xB0 => { // BCS
                if self.get_status_flag(StatusFlag::Carry) {
                    self.branch_immediate();
                }
                else {
                    self.reg.pc += 2;
                }
                self.step_cycle(2);
            }
            0xB8 => { // CLV
                set_flag(&mut self.reg.status, StatusFlag::Overflow, false);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0xBA => { // TSX
                self.reg.x = self.reg.sp;
                Machine::update_zero_negative(&mut self.reg.status, self.reg.x);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0xC0 | 0xC4 | 0xCC => { // CPY
                let m = self.get_byte(addr_mode).0;
                let result = self.reg.y.wrapping_sub(m);
                set_flag(&mut self.reg.status, StatusFlag::Carry, self.reg.y >= m);
                Machine::update_zero_negative(&mut self.reg.status, result);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::Absolute => (3, 4),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xC8 => { // INY
                self.reg.y = self.reg.y.wrapping_add(1);
                Machine::update_zero_negative(&mut self.reg.status, self.reg.y);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0xC1 | 0xC5 | 0xC9 | 0xCD | 0xD1 | 0xD5 | 0xD9 | 0xDD => { // CMP
                let (m, oops) = self.get_byte(addr_mode);
                let result = self.reg.a.wrapping_sub(m);
                set_flag(&mut self.reg.status, StatusFlag::Carry, self.reg.a >= m);
                Machine::update_zero_negative(&mut self.reg.status, result);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 4 + oops),
                    AddressingMode::IndirectX => (2, 6),
                    AddressingMode::IndirectY => (2, 5 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xC3 | 0xC7 | 0xCF | 0xD3 | 0xD7 | 0xDB | 0xDF => { // *DCP
                let (mut m, oops) = self.get_byte(addr_mode);
                m = m.wrapping_sub(1);
                self.set_byte(addr_mode, m);
                let result = self.reg.a.wrapping_sub(m);
                set_flag(&mut self.reg.status, StatusFlag::Carry, self.reg.a >= m);
                Machine::update_zero_negative(&mut self.reg.status, result);
                self.step_pc_and_cycle(match addr_mode {
//                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 6 + oops),
                    AddressingMode::IndirectX => (2, 8),
                    AddressingMode::IndirectY => (2, 7 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xC6 | 0xCE | 0xD6 | 0xDE => { // DEC
                let mut m = self.get_byte(addr_mode).0;
                m = m.wrapping_sub(1);
                self.set_byte(addr_mode, m);
                Machine::update_zero_negative(&mut self.reg.status, m);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX => (3, 7),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xCA => { // DEX
                self.reg.x = self.reg.x.wrapping_sub(1);
                Machine::update_zero_negative(&mut self.reg.status, self.reg.x);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0xD0 => { // BNE
                if !self.get_status_flag(StatusFlag::Zero) {
                    self.branch_immediate();
                }
                else {
                    self.reg.pc += 2;
                }
                self.step_cycle(2);
            }
            0xD8 => { // CLD
                set_flag(&mut self.reg.status, StatusFlag::DecimalMode, false);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            0xE0 | 0xE4 | 0xEC => { // CPX
                let m = self.get_byte(addr_mode).0;
                let result = self.reg.x.wrapping_sub(m);
                set_flag(&mut self.reg.status, StatusFlag::Carry, self.reg.x >= m);
                Machine::update_zero_negative(&mut self.reg.status, result);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::Absolute => (3, 4),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xE3 | 0xE7 | 0xEF | 0xF3 | 0xF7 | 0xFB | 0xFF => { // *ISB
                let a = self.reg.a;
                let (mut m, oops) = self.get_byte(addr_mode);
                m = m.wrapping_add(1);
                self.set_byte(addr_mode, m);
                self.compute_sbc(a, m);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::IndirectX => (2, 8),
                    AddressingMode::IndirectY => (2, 7 + oops),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 6 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xE6 | 0xEE | 0xF6 | 0xFE => { // INC
                let mut m = self.get_byte(addr_mode).0;
                m = m.wrapping_add(1);
                self.set_byte(addr_mode, m);
                Machine::update_zero_negative(&mut self.reg.status, m);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::ZeroPage => (2, 5),
                    AddressingMode::ZeroPageX => (2, 6),
                    AddressingMode::Absolute => (3, 6),
                    AddressingMode::AbsoluteX => (3, 7),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xE8 => { // INX
                self.reg.x = self.reg.x.wrapping_add(1);
                Machine::update_zero_negative(&mut self.reg.status, self.reg.x);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            
            0xE1 | 0xE5 | 0xE9 | 0xED | 0xF1 | 0xF5 | 0xF9 | 0xFD | 0xEB => { // SBC
                let a = self.reg.a;
                let (m, oops) = self.get_byte(addr_mode);
                self.compute_sbc(a, m);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 4 + oops),
                    AddressingMode::IndirectX => (2, 6),
                    AddressingMode::IndirectY => (2, 5 + oops),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0x04 | 0x0C | 0x14 | 0x1A | 0x1C | 0x34 | 0x3A | 0x3C | 0x44 |
            0x54 | 0x5A | 0x5C | 0x64 | 0x74 | 0x7A | 0x7C | 0x80 | 0xD4 | 0xDA |
            0xDC | 0xEA | 0xF4 | 0xFA | 0xFC => { // NOP
                let (_, oops) = self.get_byte(addr_mode);
                self.step_pc_and_cycle(match addr_mode {
                    AddressingMode::Implied => (1, 2),
                    AddressingMode::Immediate => (2, 2),
                    AddressingMode::AbsoluteX |
                    AddressingMode::AbsoluteY => (3, 4 + oops),
                    AddressingMode::ZeroPage => (2, 3),
                    AddressingMode::ZeroPageX => (2, 4),
                    AddressingMode::Absolute => (3, 4),
                    _ => panic!("Unexpected addressing mode"),
                    });
            }
            0xF0 => { // BEQ
                if self.get_status_flag(StatusFlag::Zero) {
                    self.branch_immediate();
                }
                else {
                    self.reg.pc += 2;
                }
                self.step_cycle(2);
            }
            0xF8 => { // SED
                set_flag(&mut self.reg.status, StatusFlag::DecimalMode, true);
                self.reg.pc += 1;
                self.step_cycle(2);
            }
            _ => { panic!("unexpected opcode {:02X}", op_code); }
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