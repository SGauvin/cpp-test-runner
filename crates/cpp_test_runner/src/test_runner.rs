use colored::Colorize;
use std::{
    process::Command,
    sync::{atomic::AtomicUsize, Mutex},
};

use crate::types::{ExecutableType, Test};
use anyhow::Result;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

pub fn run_all(tests: &[Test], use_color: bool) -> Result<()> {
    let test_number = Mutex::<u32>::default(); // Use a mutex to lock during printing
    let num_tests_passed = AtomicUsize::default();

    tests.par_iter().for_each(|test| {
        let mut args = test.arguments.clone();

        match (use_color, &test.executable.executable_type) {
            (true, ExecutableType::Gtest) => {
                args.push("--gtest_color=yes".to_string());
            }
            (false, ExecutableType::Gtest) => {
                args.push("--gtest_color=no".to_string());
            }
            (true, ExecutableType::Catch2) => {
                args.push("--colour-mode=ansi".to_string());
            }
            (false, ExecutableType::Catch2) => {
                args.push("--colour-mode=none".to_string());
            }
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

        let color_output = |output: &str| -> String {
            match (use_color, test_passed) {
                (true, true) => output.green().to_string(),
                (true, false) => output.red().to_string(),
                (false, _) => output.to_string(),
            }
        };

        let first_line = color_output(&format!(
            "{to_print_first_part}{filling}{to_print_last_part}"
        ));

        let to_print = if test_passed {
            first_line
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            format!("{first_line}\n\n{}\n", stdout.trim())
        };

        println!("{to_print}");
    });

    let num_tests_passed = num_tests_passed.load(std::sync::atomic::Ordering::Relaxed);
    let num_tests_failed = tests.len() - num_tests_passed;
    println!(
        "{} {} passed, {} {} failed",
        num_tests_passed,
        if num_tests_passed > 1 {
            "tests"
        } else {
            "test"
        },
        num_tests_failed,
        if num_tests_failed > 1 {
            "tests"
        } else {
            "test"
        },
    );

    Ok(())
}
