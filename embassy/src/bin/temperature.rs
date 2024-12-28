#![no_std]
#![no_main]

use core::fmt::Write;
use core::str::FromStr;
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use embassy_stm32::mode::Async;

use embassy_sync::channel::Channel;
use embassy_sync::channel::Sender;
use embassy_sync::channel::Receiver;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;

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

use shared::LEDState;
use shared::NucleoLEDState;

use shared::Commands::UART_STATUS_REPORT;
use shared::Commands::LED;

static STATUS_INTERVAL_MS: AtomicU32 = AtomicU32::new(10000);

static LED_STATE: Mutex<ThreadModeRawMutex, NucleoLEDState> = Mutex::new(NucleoLEDState {
    red: LEDState::Manual(false),
    green: LEDState::Manual(false),
    blue: LEDState::Manual(false),
});

static SEND_OVER: AtomicU8 = AtomicU8::new(0);
/*
const SEND_OVER_NONE: u8 = 0;
const SEND_OVER_UART: u8 = 1;
const SEND_OVER_ETH: u8 = 2;
const SEND_OVER_CAN: u8 = 4;
 */

type CommandChannel = Channel<ThreadModeRawMutex, shared::Commands, 64>;
type CommandSender = Sender<'static, ThreadModeRawMutex, shared::Commands, 64>;
type CommandReceiver = Receiver<'static, ThreadModeRawMutex, shared::Commands, 64>;
static CHANNEL_COMMANDS: CommandChannel = Channel::new();

fn led_func(led_out: &mut Output, state: &LEDState) {
    match state {
        LEDState::Manual(flag) => {
            if *flag {
                led_out.set_high();
            } else {
                led_out.set_low();
            }
        }
        _ => {}
    }
}

#[embassy_executor::task]
async fn led_task(red_led: AnyPin, green_led: AnyPin, blue_led: AnyPin) {
    let mut red_led = Output::new(red_led, Level::Low, Speed::Low);
    let mut green_led = Output::new(green_led, Level::Low, Speed::Low);
    let mut blue_led = Output::new(blue_led, Level::Low, Speed::Low);

    loop {
        Timer::after(Duration::from_millis(250)).await;
        
        let (r,g,b) = {
            let state = LED_STATE.lock().await;
            (state.red, state.green, state.blue)
        };


        led_func(&mut red_led, &r);
        led_func(&mut green_led, &g);
        led_func(&mut blue_led, &b);
    }
}

#[embassy_executor::task]
async fn command_executor(command_receiver: CommandReceiver) {
    loop {
        let cmd = command_receiver.receive().await;
        match cmd {
            UART_STATUS_REPORT(ms) => {
                hprintln!("Changed UART reporting to {}ms", ms);
                STATUS_INTERVAL_MS.store(ms, Ordering::Relaxed);
            }
            LED(id, new_state) => {
                {
                    let mut unlocked = LED_STATE.lock().await;
                    let state: &mut LEDState = match id {
                        1 => &mut unlocked.red,
                        2 => &mut unlocked.green,
                        3 => &mut unlocked.blue,
                        _ => { return; }
                    };
                    *state = new_state;    
                }
            }
            _ => {}
        }
    }
}

fn str_to_led_state(txt: &str) -> Option<shared::LEDState> {
    match txt {
        "off" => Some(shared::LEDState::Manual(false)),
        "on" => Some(shared::LEDState::Manual(true)),
        _ => None
    }
}

fn str_to_command(msg: &str) -> Option<shared::Commands> {
     // Compare the trimmed message string
    if  msg.starts_with("light") {
        let color = msg.split(' ').nth(1).unwrap();
        let func = msg.split(' ').nth(2).unwrap();
        let inner = str_to_led_state(func);
        if inner.is_none() {
            return None;
        }
        let inner = inner.unwrap();
        match color {
            "red" => {
                Some(shared::Commands::LED(1, inner))
            },
            "green" => {
                Some(shared::Commands::LED(2, inner))
            }
            "blue" => {
                Some(shared::Commands::LED(3, inner))
            }
            _ => None,
        }
    } else if msg.starts_with("status") {
        let number = msg.split(' ').nth(1).unwrap();
        let number: u32 = number.parse().unwrap();
        Some(shared::Commands::UART_STATUS_REPORT(number))
    } else {
        hprintln!("{} command unknown!", msg);
        None
    }
}

#[embassy_executor::task]
async fn uart_cmd_receiver(mut usart_rx: UartRx<'static, Async>, command_sender: CommandSender) {
    
    let mut buf: [u8; 16] = [0; 16];

    loop {
        let res = usart_rx.read_until_idle(&mut buf).await;
        let msg = match res {
            Ok(len) => {
                match core::str::from_utf8(&buf[..len]) {
                    Ok(msg) => {
                        msg.trim()
                    },
                    Err(_) => {
                        hprintln!("Received invalid utf-8 over USART, ignore transmission");
                        continue;
                    },
                }
            }
            Err(_e) => {
                hprintln!("USART related error, ignore transmission");
                continue;
            }
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
async fn uart_status_report(mut usart_tx: UartTx<'static, Async>) {
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
    
    // spawn the main logic driven by a channel of commands
    spawner.spawn(command_executor(CHANNEL_COMMANDS.receiver())).unwrap();

    // spawn a task for uart sending and receiving each
    let (tx, rx) = usart.split();
    spawner.spawn(uart_cmd_receiver(rx, CHANNEL_COMMANDS.sender())).unwrap();
    spawner.spawn(uart_status_report(tx)).unwrap();
    

    loop {
        button.wait_for_rising_edge().await;
    }
}
