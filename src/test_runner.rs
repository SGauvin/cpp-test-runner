use colored::Colorize;
use std::{
    ops::Deref,
    process::Command,
    sync::{atomic::AtomicUsize, Mutex},
};

use crate::types::Test;
use anyhow::Result;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

pub fn run_all(tests: &[Test], use_color: bool) -> Result<()> {
    let test_number = Mutex::<u32>::default(); // Use a mutex to lock during printing
    let num_tests_passed = AtomicUsize::default();

    tests.par_iter().for_each(|test| {
        let mut args = test.arguments.clone();
        if use_color {
            args.push("--gtest_color=yes".to_string());
        }

        let output = Command::new(&test.executable.path)
            .args(args)
            .output()
            .unwrap();

        let test_passed = output.status.success();

        if test_passed {
            num_tests_passed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        let mut test_num = test_number.lock().unwrap();
        *test_num += 1;

        const DESIRED_LINE_LEN: usize = 120;
        let to_print_first_part = std::format!("[{}/{}] {} ", test_num, tests.len(), test.name);
        let to_print_last_part = if test_passed { " PASSED" } else { " FAILED" };

        let number_of_chars_missing =
            DESIRED_LINE_LEN - to_print_first_part.len() - to_print_last_part.len();
        let filling = ".".repeat(number_of_chars_missing);

        let to_print = if test_passed {
            format!("{to_print_first_part}{filling}{to_print_last_part}")
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout).deref().to_string();
            format!("{to_print_first_part}{filling}{to_print_last_part}\n\n{stdout}")
        };

        let to_print = match (use_color, test_passed) {
            (true, true) => to_print.green(),
            (true, false) => to_print.red(),
            (false, _) => to_print.into(),
        };

        println!("{to_print}");
    });

    let num_tests_passed = num_tests_passed.load(std::sync::atomic::Ordering::Relaxed);
    println!(
        "{} tests passed, {} tests failed",
        num_tests_passed,
        tests.len() - num_tests_passed
    );

    Ok(())
}
