//! Functions to communicate with the BH1750FVI digital 16bit light sensor

use embassy_stm32::mode::Async;

use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use embassy_sync::signal::Signal;

use embassy_stm32::i2c::I2c;

use cortex_m_semihosting::hprintln;

use crate::cmd::LightSensorCommands;

pub const BH1750_ADDR_H: u8 = 0x5C;
pub const BH1750_ADDR_L: u8 = 0x23;

// the following constants are private and their funcitonality is implemented in this module
const BH1750_OPC_POWDOWN: u8    = 0b0000;
const BH1750_OPC_POWUP: u8      = 0b0001;
const BH1750_OPC_CO_MES_HR1: u8 = 0b01_0000;
const BH1750_OPC_OT_MES_HR1: u8 = 0b10_0000;

#[allow(dead_code)]
const BH1750_OPC_RESET: u8      = 0b0111;
#[allow(dead_code)]
const BH1750_OPC_CO_MES_HR2: u8 = 0b01_0001;
#[allow(dead_code)]
const BH1750_OPC_CO_MES_LR1: u8 = 0b01_0011;
#[allow(dead_code)]
const BH1750_OPC_OT_MES_HR2: u8 = 0b10_0001;
#[allow(dead_code)]
const BH1750_OPC_OT_MES_LR1: u8 = 0b10_0011;
// todo change measurement time
// Write Format BH1750FVI is not able to accept plural command without stop condition. Please insert SP every 1 Opecode.

pub type LightCommandChannel<const N: usize> = Channel<ThreadModeRawMutex, LightSensorCommands, N>;
pub type LightCommandSender<const N: usize> = Sender<'static, ThreadModeRawMutex, LightSensorCommands, N>;
pub type LightCommandReceiver<const N: usize> = Receiver<'static, ThreadModeRawMutex, LightSensorCommands, N>;

pub type LightSensorValueType = u16;
pub type SyncedLightSensorValueType = Mutex<ThreadModeRawMutex, Option<LightSensorValueType>>;

pub type LightSensorStateSync = Mutex<ThreadModeRawMutex, LightSensorState>;
pub type LightSensorCollectSignal = Signal<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, ()>;

#[derive(Clone, Copy)]
pub enum LightSensorState {
    PowerOff,
    SingleMeasurement,
    ContiniousMeasurement,
}

impl LightSensorState {
    pub fn as_str(&self) -> &'static str {
        match self {
            LightSensorState::PowerOff => "off",
            LightSensorState::SingleMeasurement => "single",
            LightSensorState::ContiniousMeasurement => "on",
        }
    }
}

pub async fn single_measurement(
    addr: u8, 
    i2c: &mut I2c<'static, Async>, 
    shared_state: &'static LightSensorStateSync
) -> u16 {
    let mut buf_opcode: [u8; 1] = [0; 1];

    {
        let mut unlocked = shared_state.lock().await;
        *unlocked = LightSensorState::SingleMeasurement;
    }

    buf_opcode[0] = BH1750_OPC_POWUP;
    let res = i2c.write(addr, &buf_opcode).await;
    if let Err(err) = res {
        hprintln!("Write Error: {:?} at addr={}", err, addr);
        return 0;
    }
    buf_opcode[0] = BH1750_OPC_OT_MES_HR1;
    let res = i2c.write(addr, &buf_opcode).await;
    if let Err(err) = res {
        hprintln!("Write Error: {:?} at addr={}", err, addr);
        return 0;
    }

    let mut rx_buf: [u8; 2] = [0; 2];
    hprintln!("READ I2C 2 bytes");
    let res = i2c.read(addr, &mut rx_buf).await;
    if let Err(err) = res {
        hprintln!("Read Error: {:?} at addr={}", err, addr);
        return 0;
    }

    {
        let mut unlocked = shared_state.lock().await;
        *unlocked = LightSensorState::PowerOff;
    }

    hprintln!("Bytes {}_{}", rx_buf[0], rx_buf[1]);
    ((rx_buf[0] as u16) << 8) | rx_buf[1] as u16
}

pub async fn continious_measurement(
    addr: u8,
    i2c: &mut I2c<'static, Async>,
    shared_state: &'static LightSensorStateSync
) {
    {
        let mut unlocked = shared_state.lock().await;
        *unlocked = LightSensorState::ContiniousMeasurement;
    }

    let mut buf_opcode: [u8; 1] = [0; 1];
    buf_opcode[0] = BH1750_OPC_POWUP;
    let res = i2c.write(addr, &buf_opcode).await;
    if let Err(err) = res {
        hprintln!("Write Error: {:?} at addr={}", err, addr);
        return;
    }
    buf_opcode[0] = BH1750_OPC_CO_MES_HR1;
    let res = i2c.write(addr, &buf_opcode).await;
    if let Err(err) = res {
        hprintln!("Write Error: {:?} at addr={}", err, addr);
    }
}

pub async fn power_off(
    addr: u8,
    i2c: &mut I2c<'static, Async>,
    shared_state: &'static LightSensorStateSync
) {
    {
        let mut unlocked = shared_state.lock().await;
        *unlocked = LightSensorState::PowerOff;
    }

    let mut buf_opcode: [u8; 1] = [0; 1];
    buf_opcode[0] = BH1750_OPC_POWDOWN;
    let res = i2c.write(addr, &buf_opcode).await;
    if let Err(err) = res {
        hprintln!("Write Error: {:?} at addr={}", err, addr);
    }
}