#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, Ordering};


use embassy_sync::channel::Channel;

use embassy_executor::Spawner;
use embassy_stm32::mode::Async;
use embassy_stm32::bind_interrupts;
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{AnyPin, Level, Output, Pin, Pull, Speed},
    usart::{Config, Uart, UartRx, UartTx},
};
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};
use cortex_m_semihosting::hprintln;

use nucleo_f767zi::led;
use nucleo_f767zi::led::{LedState, LedStateSync};
use nucleo_f767zi::led::LedSignal;

use nucleo_f767zi::cmd::str_to_command;
use nucleo_f767zi::cmd::{CommandChannel, CommandSender, CommandReceiver};
use nucleo_f767zi::cmd::Commands::*;

use nucleo_f767zi::setup_usart_developer_console;
use nucleo_f767zi::uart::parse_uart_tx_as_utf8;

static STATUS_INTERVAL_MS: AtomicU32 = AtomicU32::new(10000);

static LED_STATE_RED: LedStateSync = LedStateSync::new(LedState::Manual(false));
static LED_STATE_GREEN: LedStateSync = LedStateSync::new(LedState::Manual(false));
static LED_STATE_BLUE: LedStateSync = LedStateSync::new(LedState::Manual(false));

static SIGNAL_RED: LedSignal = LedSignal::new();
static SIGNAL_GREEN: LedSignal = LedSignal::new();
static SIGNAL_BLUE: LedSignal = LedSignal::new();

static CHANNEL_COMMANDS: CommandChannel = Channel::new();

#[embassy_executor::task(pool_size=3)]
async fn led_wrapper(pin: AnyPin, synced_state: &'static LedStateSync, signal: &'static LedSignal) {
    let led = Output::new(pin, Level::Low, Speed::Low);
    led::led_controller_simple(led, synced_state, signal).await;
}

#[embassy_executor::task]
async fn command_executor(
    command_receiver: CommandReceiver,
    signal_red: &'static LedSignal,
    signal_green: &'static LedSignal,
    signal_blue: &'static LedSignal,
) {
    loop {
        let cmd = command_receiver.receive().await;
        match cmd {
            UartStatusReport(ms) => {
                hprintln!("Changed UART reporting to {}ms", ms);
                STATUS_INTERVAL_MS.store(ms, Ordering::Relaxed);
            }
            Led(id, new_state) => {
                {
                    let (mut unlocked, sig) = match id {
                        1 => (LED_STATE_RED.lock().await, signal_red),
                        2 => (LED_STATE_GREEN.lock().await, signal_green),
                        3 => (LED_STATE_BLUE.lock().await, signal_blue),
                        _ => { return; }
                    };

                    *unlocked = new_state;
                    drop(unlocked);
                    sig.signal(());
                }
            }
            _ => {}
        }
    }
}

#[embassy_executor::task]
async fn uart_receiver_and_cmd_forwarder(mut usart_rx: UartRx<'static, Async>, command_sender: CommandSender) {
    let mut buf: [u8; 48] = [0; 48];
    loop {
        let msg = match parse_uart_tx_as_utf8(&mut usart_rx, &mut buf).await {
            Ok(msg) => msg,
            Err(_) => continue,
        };

        if let Some(cmd) = str_to_command(msg) {
            hprintln!("UART sends command");
            command_sender.send(cmd).await;
        }

        // Add a small delay to yield control back to the executor
        Timer::after(Duration::from_millis(10)).await;
    }
}

#[embassy_executor::task]
async fn uart_status_report_transmitter(mut usart_tx: UartTx<'static, Async>) {
    loop {
        let interval: u64 = STATUS_INTERVAL_MS.load(Ordering::Relaxed).into();
        if interval == 0 {
            Timer::after(Duration::from_millis(250)).await;
        } else {
            usart_tx.write(b"Status TODO\r\n").await.unwrap();
            Timer::after(Duration::from_millis(interval)).await;
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    //let button = Input::new(p.PC13, Pull::None);
    let mut button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);

    hprintln!("Hello, embedded world!");

    // spawn the main logic driven by a channel of commands
    spawner.spawn(command_executor(
        CHANNEL_COMMANDS.receiver(), 
        &SIGNAL_RED, 
        &SIGNAL_GREEN, 
        &SIGNAL_BLUE)).unwrap();

    // setup LED controllers, based on shared state data
    spawner.spawn(led_wrapper(p.PB14.degrade(), 
        &LED_STATE_RED, 
        &SIGNAL_RED)).unwrap();
    spawner.spawn(led_wrapper(p.PB0.degrade(), 
        &LED_STATE_GREEN, 
        &SIGNAL_GREEN)).unwrap();
    spawner.spawn(led_wrapper(p.PB7.degrade(), 
        &LED_STATE_BLUE, 
        &SIGNAL_BLUE)).unwrap();

    // start developer usart 
    bind_interrupts!(struct Irqs {
        USART3 => embassy_stm32::usart::InterruptHandler<embassy_stm32::peripherals::USART3>;
    });
    let mut usart = setup_usart_developer_console!(p, Irqs);
    usart.write(b"UART Controller started, write commands.\r\n").await.unwrap();
    
    // spawn a task for uart sending and receiving each
    let (tx, rx) = usart.split();
    spawner.spawn(uart_receiver_and_cmd_forwarder(rx, CHANNEL_COMMANDS.sender())).unwrap();
    spawner.spawn(uart_status_report_transmitter(tx)).unwrap();
    
    loop {
        button.wait_for_rising_edge().await;
    }
}
