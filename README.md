# ULog file parser for Rust

A ULog file parser for Rust written with a small memory footprint.
Reading the file is implemented in a streaming manner, where possible.

## Features
All ULog file format features are supported, except for:
- Resuming from file corruptions by searching for the synchronization message

Writing ULog files is not supported.

## Design goals

The API should be streaming, meaning:

  * Store minimum possible amount of data in memory
  * Everything should be implemented as iterators
  * Data should be read directly from file, where possible

Other goals are:

  * Don't panic
  * Don't use unsafe

## Development

For development, install the pre-commit scripts:
```shell
pre-commit install
```
