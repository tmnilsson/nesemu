extern crate sdl2;
extern crate time;

use std::env;
use time::Duration;

mod nes;

#[cfg(test)]
use std::fs::File;

#[cfg(test)]
use std::io::{BufRead, BufReader};

use std::path::Path;

// Needs nestest.nes and nestest.log from wiki.nesdev.com in same directory
#[cfg(test)]
fn test_nestest_rom(verbose: bool) {
    let mut machine = nes::Machine::new(false);
    let mut cpu = nes::cpu::Cpu::new();
    let cartridge = nes::cartridge::Cartridge::load(Path::new("nestest.nes"));
    machine.load_cartridge(cartridge);
    cpu.reset(&mut machine);
    cpu.set_program_counter(0xc000);
    machine.set_scan_line(241);

    let baseline = File::open("nestest.log")
        .expect("Unable to open nestest.log");
    let mut baseline = BufReader::new(baseline);

    let mut line_no = 1;
    loop {
        if verbose {
            println!("{}", nes::get_state_string(&cpu, &mut machine));
        }

        let mut baseline_line = String::new();
        baseline.read_line(&mut baseline_line).unwrap();
        baseline_line = baseline_line.trim().to_string();

        if baseline_line == "" {
            break; // finished
        }
        if baseline_line != nes::get_state_string(&cpu, &mut machine) {
            assert!(false, "Mismatch at line {}!\n{}\nBaseline:\n{}\n",
                    line_no, nes::get_state_string(&cpu, &mut machine), baseline_line);
            break;
        }

        cpu.execute(&mut machine);
        line_no += 1;
    }
}

#[test]
fn nestest_rom() {
    test_nestest_rom(false);
}

fn main()
{
    let mut machine = nes::Machine::new(false);
    let mut cpu = nes::cpu::Cpu::new();
    let args: Vec<_> = env::args().collect();

    let cartridge = nes::cartridge::Cartridge::load(Path::new(&args[1]));
    machine.load_cartridge(cartridge);
    cpu.reset(&mut machine);

    if args.len() >= 3 && args[2] == "disassemble" {
        for line in cpu.disassemble(
            &mut machine,
            usize::from_str_radix(&args[3], 16).unwrap(),
            usize::from_str_radix(&args[4], 16).unwrap(),
        ) {
            println!("{}", line);
        }
        return;
    }

    'running: loop {
        match machine.handle_events() {
            Some(ref e) if *e == nes::SystemEvent::Quit => {
                break 'running;
            }
            Some(ref e) if *e == nes::SystemEvent::Reset => {
                cpu.reset(&mut machine);
            }
            None | Some(_) => {}
        }
        let prev_quarter_frame_count = machine.apu.quarter_frame_count;
        while machine.apu.quarter_frame_count == prev_quarter_frame_count {
            let prev_vblank = machine.ppu.vblank;
            cpu.execute(&mut machine);
            if machine.ppu.vblank && !prev_vblank {
                machine.present();
            }
        }
        const TARGET_BUFFER_SIZE_MS: i64 = 35;
        let sleep_time = machine.get_audio_queue_size_ms() as i64 - TARGET_BUFFER_SIZE_MS;
        if sleep_time > 0 {
            std::thread::sleep(Duration::milliseconds(sleep_time).to_std().unwrap());
        }
    }

    machine.save();
}
