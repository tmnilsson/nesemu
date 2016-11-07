mod machine;

use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;


fn test_nestest_rom(verbose: bool) {
    let mut m = machine::Machine::new();
    let rom = machine::read_nes_file("nestest.nes");
    m.load_rom(rom);

    let baseline = File::open("nestest.log")
        .expect("Unable to open nestest.log");
    let mut baseline = BufReader::new(baseline);

    let mut line_no = 1;
    loop {
        if verbose {
            println!("{}", m.get_state_string());
        }

        let mut baseline_line = String::new();
        baseline.read_line(&mut baseline_line).unwrap();
        baseline_line = baseline_line.trim().to_string();

        if baseline_line == "" {
            break; // finished
        }
        if baseline_line != m.get_state_string() {
            println!("Mismatch at line {}! Baseline:", line_no);
            println!("{}", baseline_line);
            break;
        }

        m.execute_instruction();
        line_no += 1;
    }
}

#[test]
fn nestest_rom() {
    test_nestest_rom(false);
}

fn main()
{
    test_nestest_rom(true);
}
