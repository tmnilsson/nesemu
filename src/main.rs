extern crate sdl2;
extern crate time;

use std::env;
use time::{Duration, PreciseTime};

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

    let mut prev_time = PreciseTime::now();
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
        cpu.execute_until_nmi(&mut machine);
        machine.present();
        let now = PreciseTime::now();
        let duration = prev_time.to(now);
        if duration.num_milliseconds() < 16 {
            std::thread::sleep((Duration::milliseconds(16) - duration).to_std().unwrap());
        }
        prev_time = PreciseTime::now();
    }

    machine.save();
}
