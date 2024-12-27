#![no_std]
#![no_main]
//#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::usart::{Config, Uart};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    bind_interrupts!(struct Irqs {
        USART3 => embassy_stm32::usart::InterruptHandler<embassy_stm32::peripherals::USART3>;
    });
    

    let mut usart = Uart::new(
        p.USART3,
        p.PD9, // rx
        p.PD8, // tx
        Irqs,
        p.DMA1_CH4, // tx
        p.DMA1_CH1, // rx
        Config::default(),
    ).expect("USART generation failed");

    usart.write(b"Starting Echo\r\n").await.unwrap();

    let mut msg: [u8; 8] = [0; 8];

    loop {
        usart.read(&mut msg).await.unwrap();
        usart.write(&msg).await.unwrap();
    }
    
}
