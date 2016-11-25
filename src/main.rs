mod nes;

use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

fn test_nestest_rom(verbose: bool) {
    let mut machine = nes::Machine::new();
    let mut cpu = nes::cpu::Cpu::new();
    let rom = nes::read_nes_file("nestest.nes");
    machine.load_rom(rom);
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
    let mut machine = nes::Machine::new();
    let mut cpu = nes::cpu::Cpu::new();
    let rom = nes::read_nes_file("nestest.nes");
    machine.load_rom(rom);
    cpu.reset(&mut machine);

    loop {
        println!("{}", nes::get_state_string(&cpu, &mut machine));
        cpu.execute(&mut machine);
    }
}
