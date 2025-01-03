#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, Ordering};
use core::fmt::Write;
use heapless::String;

use static_cell::{StaticCell};

use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex;

use embassy_futures::select::select;
use embassy_futures::select::Either;

use embassy_executor::Spawner;
use embassy_stm32::mode::Async;
use embassy_stm32::bind_interrupts;
use embassy_stm32::i2c::I2c;
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{AnyPin, Level, Output, Pin, Pull, Speed},
    usart::{Uart, UartRx, UartTx},
};
use embassy_stm32::usart::Config as UsartConfig;
use embassy_stm32::i2c::Config as I2cConfig;

use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};
use cortex_m_semihosting::hprintln;


use nucleo_f767zi::led;
use nucleo_f767zi::led::{LedState, LedStateSync};
use nucleo_f767zi::led::LedSignal;

use nucleo_f767zi::cmd::str_to_command;
use nucleo_f767zi::cmd::{CommandChannel, CommandSender, CommandReceiver};
use nucleo_f767zi::cmd::Commands::*;
use nucleo_f767zi::cmd::Commands;
use nucleo_f767zi::cmd::LightSensorCommands;

use nucleo_f767zi::bh1750fvi::LightSensorState;
use nucleo_f767zi::bh1750fvi::LightSensorStateSync;
use nucleo_f767zi::bh1750fvi::SyncedLightSensorValueType;
use nucleo_f767zi::bh1750fvi::LightSensorCollectSignal;

use nucleo_f767zi::bh1750fvi::{single_measurement, continious_measurement, power_off};
use nucleo_f767zi::bh1750fvi::BH1750_ADDR_L;

use nucleo_f767zi::setup_usart_developer_console;
use nucleo_f767zi::uart::parse_uart_tx_as_utf8;

use embassy_stm32::time::Hertz;

type I2cAsyncMutex = mutex::Mutex<CriticalSectionRawMutex, I2c<'static, Async>>;

static STATUS_INTERVAL_MS: AtomicU32 = AtomicU32::new(10000);

static LED_STATE_RED: LedStateSync = LedStateSync::new(LedState::Manual(false));
static LED_STATE_GREEN: LedStateSync = LedStateSync::new(LedState::Manual(false));
static LED_STATE_BLUE: LedStateSync = LedStateSync::new(LedState::Manual(false));

static SIGNAL_RED: LedSignal = LedSignal::new();
static SIGNAL_GREEN: LedSignal = LedSignal::new();
static SIGNAL_BLUE: LedSignal = LedSignal::new();

static LIGHT_SENSOR_STATE: LightSensorStateSync = LightSensorStateSync::new(LightSensorState::PowerOff);
static LIGHT_SENSOR_VALUE: SyncedLightSensorValueType = SyncedLightSensorValueType::new(None);
static LIGHT_SENSOR_SIGNAL: LightSensorCollectSignal = LightSensorCollectSignal::new();

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
    signal_light: &'static LightSensorCollectSignal,
    i2c: &'static I2cAsyncMutex,
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
            LightSensor(sub_cmd) => {
                match sub_cmd {
                    LightSensorCommands::Off => {
                        signal_light.signal(());
                        power_off(BH1750_ADDR_L, &mut *(i2c.lock().await), &LIGHT_SENSOR_STATE).await;
                    }
                    LightSensorCommands::SingleMeasurment => {
                        signal_light.signal(());
                        let lux = single_measurement(BH1750_ADDR_L, &mut *(i2c.lock().await), &LIGHT_SENSOR_STATE).await;
                        hprintln!("{} Lux light intensity", lux);

                        {
                            let mut unlock = LIGHT_SENSOR_VALUE.lock().await;
                            *unlock = Some(lux);
                        }
                    }
                    LightSensorCommands::ContiniousMeasurement => {
                        hprintln!("Light Continous");
                        signal_light.signal(());
                        continious_measurement(BH1750_ADDR_L, &mut *(i2c.lock().await), &LIGHT_SENSOR_STATE).await;

                    }
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
            hprintln!("UART Report!");

            let mut msg: String<256> = String::new();
            msg.push_str("Status: Light Sensor ").unwrap();
            {
                let unlocked = LIGHT_SENSOR_STATE.lock().await;
                let temp: &str = (*unlocked).as_str();
                msg.push_str(temp).unwrap();
                
            }

            {
                let unlocked = LIGHT_SENSOR_VALUE.lock().await;
                let mut buf: String<16> = String::new();
                if let Some(value) = *unlocked {
                    core::write!(&mut buf, " - {} Lux", value).unwrap();
                    msg.push_str(buf.as_str()).unwrap();
                } else {
                    msg.push_str(" - No sensor value yet").unwrap();
                }
            }

            msg.push_str("\r\n").unwrap();
            usart_tx.write(&msg.into_bytes()).await.unwrap();
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

    // bind interrupts
    bind_interrupts!(struct Irqs {
        USART3 => embassy_stm32::usart::InterruptHandler<embassy_stm32::peripherals::USART3>;
        I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<embassy_stm32::peripherals::I2C1>;
        I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<embassy_stm32::peripherals::I2C1>;
    });
    
    // start i2c for sensor:
    let i2c: I2c<'static, Async> = I2c::new(
        p.I2C1,
        p.PB8, // scl
        p.PB9, // sda
        Irqs,
        p.DMA1_CH6, // dma tx
        p.DMA1_CH0, // dma rx
        Hertz(100_000), // SCL clock frequency !?
        I2cConfig::default(),
    );
    static I2C: StaticCell<I2cAsyncMutex> = StaticCell::new();
    let i2c = I2C.init(mutex::Mutex::new(i2c));

    // start developer usart 
    let mut usart = setup_usart_developer_console!(p, Irqs, UsartConfig::default());
    usart.write(b"UART Controller started, write commands.\r\n").await.unwrap();
    
    // spawn a task for uart sending and receiving each
    let (tx, rx) = usart.split();
    spawner.spawn(uart_receiver_and_cmd_forwarder(rx, CHANNEL_COMMANDS.sender())).unwrap();
    spawner.spawn(uart_status_report_transmitter(tx)).unwrap();
    
    spawner.spawn(process_light_sensor(
        &LIGHT_SENSOR_SIGNAL,
        i2c
    )).unwrap();

    // spawn the main logic driven by a channel of commands
    spawner.spawn(command_executor(
        CHANNEL_COMMANDS.receiver(), 
        &SIGNAL_RED, 
        &SIGNAL_GREEN, 
        &SIGNAL_BLUE,
        &LIGHT_SENSOR_SIGNAL,
        i2c)).unwrap();

    loop {
        button.wait_for_rising_edge().await;
        CHANNEL_COMMANDS.sender().send(Commands::LightSensor(LightSensorCommands::SingleMeasurment)).await;
        Timer::after(Duration::from_millis(50)).await;
    }
}

#[embassy_executor::task]
async fn process_light_sensor(signal: &'static LightSensorCollectSignal, i2c: &'static I2cAsyncMutex) {
    loop {
        let state = {
            *(LIGHT_SENSOR_STATE.lock().await)
        };

        match state {
            LightSensorState::ContiniousMeasurement => {
                let mut rx_buf: [u8; 2] = [0; 2];
                let res = {
                    let i2c = &mut (*i2c.lock().await);
                    let f1 = i2c.read(BH1750_ADDR_L, &mut rx_buf);
                    let f2 = signal.wait();
                    select(f1, f2).await
                };

                if let Either::First(res) = res {
                    if let Err(err) = res {
                        hprintln!("Write Error: {:?} at addr={}", err, BH1750_ADDR_L);
                    } else {
                        let mut unlocked = LIGHT_SENSOR_VALUE.lock().await;
                        *unlocked = Some(((rx_buf[0] as u16) << 8) | rx_buf[1] as u16);
                    }
                } else {
                    hprintln!("Continious i2c reading interrupted by signal");
                }
                
                Timer::after(Duration::from_millis(150)).await;
            }
            LightSensorState::PowerOff | LightSensorState::SingleMeasurement => signal.wait().await,
        }
    }
}