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

fn sleep_frame(machine: &nes::Machine, prev_time: PreciseTime, frame_index: usize) {
    // The expected frame time is 16.67 ms (since the NES used 60 fps),
    // but if we blindly sleep to match that we will slowly get out of sync,
    // which could result in audio gaps/blips. Instead dynamically adjust the
    // emulation speed to make sure that the audio sample queue always is
    // kept large enough (but not too large which would cause extra latency)

    if frame_index < 100 {
        // Let the audio queue to fill up in the beginning,
        // to avoid gaps while it is still not full
        return;
    }

    let now = PreciseTime::now();
    let duration = prev_time.to(now);
    let duration_target = if machine.get_audio_queue_size_ms() > 50 {
        18  // slightly slow down
    } else {
        15  // slightly speed up
    };
    let sleep_time = if duration.num_milliseconds() < duration_target {
        Duration::milliseconds(duration_target) - duration
    } else {
        println!("Warning: exceeded time budget (duration {} ms)", duration.num_milliseconds());
        Duration::milliseconds(0)
    };
    std::thread::sleep(sleep_time.to_std().unwrap());
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
    let mut frame_index = 0;
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
        while machine.ppu.vblank {
            cpu.execute(&mut machine);
        }
        while !machine.ppu.vblank {
            cpu.execute(&mut machine);
        }
        machine.present();
        sleep_frame(&machine, prev_time, frame_index);
        prev_time = PreciseTime::now();
        frame_index += 1;
    }

    machine.save();
}
