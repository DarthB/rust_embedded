//! Supports hardware agnostic control and synchronized state management for LEDs 
//! 
//! The LED control is simplistic and contans the states off/on/toggle(ms) as encoded in [LedState]. 
//! [LedStateSync] can be used to access the LED state from different tasks. 
//! 
//! The function [led_controller_simple] may be wrapped by an embassy task to add the functionality to an LED

use embassy_stm32::gpio::Output;

use embassy_time::{Duration, Timer};

use embassy_sync::signal::Signal;
use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;

use embassy_futures::select::select;

pub type LedSignal = Signal<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, ()>;

/// LedState protected by Mutex
pub type LedStateSync = Mutex<ThreadModeRawMutex, LedState>;

#[derive(Clone, Copy)]
pub enum LedState {
    /// the led is either on or off
    Manual(bool),

    /// the led toogles every specified amount of ms
    Toggle(u32),
}

impl Default for LedState {
    fn default() -> Self {
        LedState::Manual(false)
    }
}

/// Supports on/off/toggle(ms) useful for LEDs. 
/// 
/// 
pub async fn led_controller_simple(mut led: Output<'_>, synced_state: &LedStateSync, signal: &LedSignal) {
    loop {
        // get a copy of the state
        let state = {
            *synced_state.lock().await
        };

        // change led state over hardware
        led_update_simple(&mut led, &state);

        // setup futures based on state
        if let LedState::Toggle(ms) = state {
            let f1 = Timer::after(Duration::from_millis(ms.into()));
            let f2 = signal.wait();
            select(f1, f2).await;
            
        } else {
            signal.wait().await;
        }
    }
}

pub fn str_to_led_state(txt: &str) -> Option<LedState> {
    match txt {
        "off" => Some(LedState::Manual(false)),
        "on" => Some(LedState::Manual(true)),
        other => {
            match other.parse() {
                Ok(ms) => Some(LedState::Toggle(ms)),
                Err(_) => None 
            }
        }
    }
}

fn led_update_simple(led_out: &mut Output, state: &LedState) {
    match state {
        LedState::Manual(flag) => {
            if *flag {
                led_out.set_high();
            } else {
                led_out.set_low();
            }
        }
        LedState::Toggle(_) => {
            led_out.toggle();
        }
    }
}
