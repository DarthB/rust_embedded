#![no_std]
#![no_main]

use core::fmt::Write;
use core::str::FromStr;
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use embassy_stm32::mode::Async;

use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    exti::ExtiInput,
    gpio::{AnyPin, Level, Output, Pin, Pull, Speed},
    usart::{Config, Uart, UartRx, UartTx},
};
use embassy_time::{Duration, Timer};
use heapless::String;
use {defmt_rtt as _, panic_probe as _};

use cortex_m_semihosting::{debug, hprintln};

static STATUS_INTERVAL_MS: AtomicU32 = AtomicU32::new(10000);

static STATE_LIGHT_SENSOR: AtomicU8 = AtomicU8::new(0);
/*
const STATE_LIGHT_OFF: u8 = 0;
const STATE_LIGHT_ON_BUT_NOT_CONFIGURED: u8 = 1;
const STATE_LIGHT_SENDING: u8 = 2;
 */

static SEND_OVER: AtomicU8 = AtomicU8::new(0);
/*
const SEND_OVER_NONE: u8 = 0;
const SEND_OVER_UART: u8 = 1;
const SEND_OVER_ETH: u8 = 2;
const SEND_OVER_CAN: u8 = 4;
 */

#[embassy_executor::task]
async fn led_task(red_led: AnyPin, green_led: AnyPin, blue_led: AnyPin) {
    let mut red_led = Output::new(red_led, Level::Low, Speed::Low);
    let mut green_led = Output::new(green_led, Level::Low, Speed::Low);
    let mut blue_led = Output::new(blue_led, Level::Low, Speed::Low);

    loop {
        Timer::after(Duration::from_millis(250)).await;
        
        if STATE_LIGHT_SENSOR.load(Ordering::Relaxed) == 1 {
            green_led.set_low();
            blue_led.set_low();
            red_led.set_high();
        } else {
            // just turn the green led one for now
            green_led.toggle();
        }
    }
}

#[embassy_executor::task]
async fn uart_controller(mut usart_rx: UartRx<'static, Async>) {
    
    let mut buf: [u8; 16] = [0; 16];
    let lon: String<16> = String::try_from("light on").unwrap();

    loop {
        let res = usart_rx.read_until_idle(&mut buf).await;
        match res {
            Ok(len) => {
                match core::str::from_utf8(&buf[..len]) {
                    Ok(msg) => {
                        let msg = msg.trim();
                        
                        // Compare the trimmed message string
                        if  msg == "light on" {
                            hprintln!("On!");
                            STATE_LIGHT_SENSOR.store(1, Ordering::Relaxed);
                        } else if msg.starts_with("status") {
                            let number = msg.split(' ').nth(1).unwrap();
                            let number: u32 = number.parse().unwrap();
                            STATUS_INTERVAL_MS.store(number, Ordering::Relaxed);
                        } else {
                            hprintln!("Anything!");
                            STATE_LIGHT_SENSOR.store(0, Ordering::Relaxed);
                        }
                    },
                    Err(_) => {
                        hprintln!("Received invalid utf8 over USART");
                        continue;
                    },
                }
            }
            Err(_e) => {
                hprintln!("Another USART related error");
                continue;
            }
        };
        
        // Add a small delay to yield control back to the executor
        Timer::after(Duration::from_millis(10)).await;
    }
}

#[embassy_executor::task]
async fn uart_monitor(mut usart_tx: UartTx<'static, Async>) {
    loop {
        Timer::after(Duration::from_millis(STATUS_INTERVAL_MS.load(Ordering::Relaxed).into())).await;
        
        usart_tx.write(b"Status TODO\r\n").await.unwrap();
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    //let button = Input::new(p.PC13, Pull::None);
    let mut button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);

    hprintln!("Hello, world!");

    // show status via LEDs
    spawner.spawn(led_task(p.PB14.degrade(), p.PB0.degrade(), p.PB7.degrade())).unwrap();

    bind_interrupts!(struct Irqs {
        USART3 => embassy_stm32::usart::InterruptHandler<embassy_stm32::peripherals::USART3>;
    });
    
    // setup usart for controller
    let mut usart = Uart::new(
        p.USART3,
        p.PD9, // rx
        p.PD8, // tx
        Irqs,
        p.DMA1_CH4, // tx
        p.DMA1_CH1, // rx
        Config::default(),
    ).expect("USART generation failed");

    usart.write(b"UART Controller started, write commands.\r\n").await.unwrap();
    
    // spawn a task for sending and receiving each
    let (tx, rx) = usart.split();
    spawner.spawn(uart_controller(rx)).unwrap();
    spawner.spawn(uart_monitor(tx)).unwrap();

    loop {
        button.wait_for_rising_edge().await;
    }
}
