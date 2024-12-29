//! A Software Abstraction Layer for Commands based upon the [Commands] enum, the parsing
//! method [str_to_command] and communication with commmand executors via a [Channel]
//! synchronisation. 
//! 
//! Supports the management of LED states on/off/toggle(ms).
//! 
//! Supports adaption of the interval of a [Commands::UartStatusReport]

use crate::led::{str_to_led_state, LedState};

use cortex_m_semihosting::hprintln;

use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};

pub type CommandChannel = Channel<ThreadModeRawMutex, Commands, 64>;
pub type CommandSender = Sender<'static, ThreadModeRawMutex, Commands, 64>;
pub type CommandReceiver = Receiver<'static, ThreadModeRawMutex, Commands, 64>;

pub enum Commands {
    /// sets the interval for the uart status report
    UartStatusReport(u32),

    /// sets a new LEDState for the given LED id
    Led(u8, LedState),

    /// using a I2C connection to a BH1750FVI
    LightSensor,

    /// using a one-way connection to a DS18B20
    TemperatureSensor,
}

pub fn str_to_command(msg: &str) -> Option<Commands> {
    // Compare the trimmed message string
   if  msg.starts_with("led") || msg.starts_with("light") {
       let color = msg.split(' ').nth(1).unwrap();
       let func = msg.split(' ').nth(2).unwrap();
       let inner = str_to_led_state(func)?;

       match color {
           "r" | "red" => {
               Some(Commands::Led(1, inner))
           },
           "g" | "green" => {
               Some(Commands::Led(2, inner))
           }
           "b" | "blue" => {
               Some(Commands::Led(3, inner))
           }
           _ => None,
       }
   } else if msg.starts_with("status") {
       let number = msg.split(' ').nth(1).unwrap();
       let number: u32 = number.parse().unwrap();
       Some(Commands::UartStatusReport(number))
   } else {
       hprintln!("{} command unknown!", msg);
       None
   }
}