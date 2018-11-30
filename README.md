PX4 Ulog file parser for Rust
=============================

NOTE: Before 1.0 we will not follow semantic versioning.

A ULog file parser for Rust written with a small memory footprint.
Reading the file is implemented in a streaming manner, where possible.


Contributing
------------

### Design goals

The API should be streaming, meaning:

  * Store minimum possible amount of data in memory
  * Everything should be implemented as iterators
  * Data should be read directly from file, where possible

Other goals are:

  * Don't panic
  * Don't use unsafe

