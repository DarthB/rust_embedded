#![no_std]
#![no_main]

// pick a panicking behavior
use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
                     // use panic_abort as _; // requires nightly
                     // use panic_itm as _; // logs messages over ITM; requires ITM support
                     //use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

use cortex_m::asm;
use cortex_m_rt::entry;

use stm32f7::stm32f7x7;

fn setup_button(pep: &stm32f7x7::Peripherals) {
    pep.RCC.ahb1enr.modify(|_r, w| w.gpiocen().set_bit());
    pep.GPIOC.moder.write(|w| w.moder13().input());
}

fn setup_leds(pep: &stm32f7x7::Peripherals) {
    // enable PORT B clock without that there is no communication at all.
    pep.RCC.ahb1enr.modify(|_, w| w.gpioben().set_bit());

    pep.GPIOB
        .moder
        .write(|w| w.moder7().output().moder14().output().moder0().output());
}

fn set_green_led(pep: &stm32f7x7::Peripherals, set: bool) {
    pep.GPIOB.bsrr.write(|w| {
        if set {
            w.bs0().set_bit()
        } else {
            w.br0().set_bit()
        }
    });
}

fn set_red_led(pep: &stm32f7x7::Peripherals, set: bool) {
    pep.GPIOB.bsrr.write(|w| {
        if set {
            w.bs14().set_bit()
        } else {
            w.br14().set_bit()
        }
    });
}

fn set_blue_led(pep: &stm32f7x7::Peripherals, set: bool) {
    pep.GPIOB.bsrr.write(|w| {
        if set {
            w.bs7().set_bit()
        } else {
            w.br7().set_bit()
        }
    });
}

#[entry]
fn main() -> ! {
    asm::nop(); // To not have main optimize to abort in release mode, remove when you add code

    let mut counter = 1u8;

    let pep = stm32f7x7::Peripherals::take().unwrap();
    setup_leds(&pep);
    setup_button(&pep);

    const COUNTER: i32 = 50_000;

    loop {
        for _ in 0..COUNTER {
            // Assuming 168 MHz clock and some cycles per loop
            if pep.GPIOC.idr.read().idr13().bit_is_set() {
                counter = 0;
            }
            cortex_m::asm::nop(); // No operation, just wait
        }

        let bit1 = counter % 2 > 0;
        let bit2 = (counter / 2) % 2 > 0;
        let bit3 = (counter / 4) % 2 > 0;

        set_green_led(&pep, bit1);
        set_blue_led(&pep, bit2);
        set_red_led(&pep, bit3);

        counter += 1;
        if counter > 7 {
            counter = 0;
        }
    }
}
