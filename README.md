# C++ Test Runner

A build-system agnostic CLI tool that finds and executes all [`GoogleTest`](https://github.com/google/googletest) and [`Catch2`](https://github.com/catchorg/Catch2) tests in a directory.

## Features

- Automatically finds all test executables
- Runs tests in parallel by default
- Supports generating a `launch.json` file for debugging in VSCode and Neovim (using [`nvim-dap`](https://github.com/mfussenegger/nvim-dap/blob/665d3569a86395fe0dab85efbdb26d7d2ee57e49/doc/dap.txt#L1378))
- Has an interactive mode where you can fuzzy-find tests

## Usage

For all options, use `--help` to print the help text.

### Finding and executing all tests

To find and execute all individual tests in the current directory, simply use the `run` subcommand.

```
cpp_test_runner run
```

### Listing all tests

To list all the tests in the current directory, simply use the `list` subcommand.

```
cpp_test_runner list
```

By default, this will output a Json object with all the tests that were found.
Json was chosen as a default since it is an easy format to use by other programs for scripting.

If you want a pretty-printed json, you can use `--output=pretty-json`, or pipe the program's output through `jq`.
You can also use `--output=plain` to have a newline-separated list of all the tests.

### Generating a `launch.json`

Generating a `launch.json` through `cpp_test_runner` file can be an easy way to be able to debug individual tests in your text editor, granted it supports it.
To do so, use the `launch-json` subcommand, and possibly redirect its output to `.vscode/launch.json`.

```
cpp_test_runner launch-json
```

Multiple options are available in order to modify how the `launch.json` file is generated, like `--stop-at-entry` to stop the program in the main function, and `--pretty-printing` to enable pretty-printing.
Use `--help` to see all available options.

### Specifying a test directory

To specify a test directory, use the `--test-dir` option. The test directory will be used as the root of the search for all the test executables.
If an absolute path is used, it will be used as-is, but if a relative path is used, the program will look up in parent directories until the test directory is found.
This way, the test-directory will can be found even if the cli is being used in a directory sibling to the test directory.

```
cpp_test_runner <run|list|launch-json> --test-dir <TEST-DIR>
```

### Filtering tests

To filter tests by their name, you can use the `--filter` option.

```
cpp_test_runner <run|list|launch-json> --filter <REGEX>
```

### Fuzzy-finding tests

To interactively fuzzy-find tests by their name, use the `--interactive` flag.

```
cpp_test_runner <run|list|launch-json> --interactive
```

This flag uses [`skim`](https://github.com/skim-rs/skim) internally.
If you are familiar with [`fzf`](https://github.com/junegunn/fzf), you should feel right at home using this flag.

### Treating executables as single tests

If you don't want the tool to parse individual tests inside the executables, you can use the you can use the `--executables-only` flag.

```
cpp_test_runner <run|list|launch-json> --executables-only
```

### Setting custom flags

To set custom flags when running the executables, the flags `--gtest-extra-args` and `--catch2-extra-args` can be used. For example:

```
cpp_test_runner <run|list|launch-json> --gtest-extra-args="--gtest_repeat=10,--gtest_shuffle" --catch2-extra-args="--durations"
```
