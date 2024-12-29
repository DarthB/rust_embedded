//! First embassy example: Using LED blinking and switching frequency with the user button.
//! 
//! This runs on a NUCLEO-F767ZI 

#![no_std]
#![no_main]

use core::fmt::Write;
use core::sync::atomic::{AtomicU32, Ordering};

use embassy_executor::Spawner;
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{AnyPin, Level, Output, Pin, Pull, Speed},
    usart::{Config, UartTx},
};
use embassy_time::{Duration, Timer};
use heapless::String;
use {defmt_rtt as _, panic_probe as _};

static BLINK_MS: AtomicU32 = AtomicU32::new(0);

#[embassy_executor::task]
async fn led_task(led: AnyPin) {
    let mut led = Output::new(led, Level::High, Speed::Low);

    loop {
        let del_var = BLINK_MS.load(Ordering::Relaxed);
        Timer::after(Duration::from_millis(del_var.into())).await;
        led.toggle();
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    //let button = Input::new(p.PC13, Pull::None);
    let mut button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);

    // store standard frequency
    let mut del_var = 2000;
    BLINK_MS.store(del_var, Ordering::Relaxed);

    spawner.spawn(led_task(p.PB7.degrade())).unwrap();

    let mut usart = UartTx::new(p.USART3, p.PD8, p.DMA1_CH4, Config::default()).unwrap();

    let mut value: u8 = 0;
    let mut msg: String<16> = String::new();
    loop {
        button.wait_for_rising_edge().await;
        del_var -= 300_u32;
        if del_var < 500_u32 {
            del_var = 2000_u32;
        }
        BLINK_MS.store(del_var, Ordering::Relaxed);

        // Format Message
        core::writeln!(&mut msg, "\rVAL: {:03}", value).unwrap();

        // Transmit Message
        usart.write(msg.as_bytes()).await.unwrap();

        // Update Value Parameter
        value = value.wrapping_add(1);

        // Clear String for next message
        msg.clear();
    }
}
