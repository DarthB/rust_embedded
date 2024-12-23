# Embedded Rust on STM32-NUCLEO-F767ZI

This repository consists of projects on the development board STM32-NUCELO-F767ZI and documents my exploration of the Rust embedded world.

The used hardware is shown in the below picture: A STM32-NUCELO-F767ZI.

The repository consists of the following sub-projects:

- LED Counter based upon embedded Rust Book, uses PAC-level abstraction.
- LED Blinker based upon embassy starting example, uses async and embassy.
- Measurement Platform (TODO)

## How to get started

Assumptions: 

- Hardware sanity check with the official STM tools is done, e.g. STM Cube Programmer can be used to connect over STLink.
- Installation as described in [Rust Embedded Book - Hardware](https://docs.rust-embedded.org/book/start/hardware.html) which is based on the [Cortex Quickstart Template](https://github.com/rust-embedded/cortex-m-quickstart).

### LED Counter

As it's based upon the Embedded Rust Book

1. If not running, start OpenOCD `openocd -f interface/stlink.cfg -f target/stm32f7x.cfg`
2. Switch to folder `cd led_counter`
3. Execute `cargo run --release`

You should see `Breakpoint 4, 0x08000254 in main ()` in gdb. Type `continue` and the coounting started.

Observe the binary counting using GREEN LED for 1 bit, BLUE for 2nd bit and RED for 3rd bit. **It's to fast isn't it?** - we use busy loop here so you can redo it without release:

4. Execute `cargo run`

to reduce the counter frequency.

### LED Blinker

1. If not running, start OpenOCD `openocd -f interface/stlink.cfg -f target/stm32f7x.cfg`
2. Switch to folder `cd embassy`
3. Execute `cargo run --bin led_blinking --release`

## Used Documentation

TODO