#![no_std]

use embassy_stm32::gpio::Output;

#[derive(Clone, Copy)]
pub enum LEDState {
    /// the led is either on or off
    Manual(bool),

    /// the led toogles every specified amount of ms
    Toogle(u32)
}

pub struct LED<'a> {
    /// the output can be changed at runtime
    output: Option<Output<'a>>,

    state: LEDState,
}

impl Default for LEDState {
    fn default() -> Self { LEDState::Manual(false) }
}

#[derive(Clone, Copy)]
pub struct NucleoLEDState {
    pub red: LEDState,
    pub green: LEDState,
    pub blue: LEDState,
}

impl Default for NucleoLEDState {
    fn default() -> Self {
        NucleoLEDState {
            red: LEDState::default(),
            green: LEDState::default(),
            blue: LEDState::default(),
        }
    }
}

pub enum Commands {
    /// sets the interval for the uart status report
    UART_STATUS_REPORT(u32),

    /// sets a new LEDState for the given LED id
    LED(u8, LEDState),

    TEMPERATURE_SENSOR,

    LIGHT_SENSOR
}
