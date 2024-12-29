//use core::str::Utf8Error;
//use thiserror::Error;

use heapless::String;

use embassy_stm32::mode::Async;
use embassy_stm32::usart::UartRx;

use cortex_m_semihosting::hprintln;

#[macro_export]
macro_rules! setup_usart_developer_console {
    ($p:ident, $irqs:ident) => {
        // setup usart
        Uart::new(
            $p.USART3,
            $p.PD9, // rx
            $p.PD8, // tx
            $irqs,
            $p.DMA1_CH4, // tx
            $p.DMA1_CH1, // rx
            Config::default(),
        ).expect("USART generation failed")
    }
}

/* 
#[derive(Error, Debug)]
pub enum UartParseError {
    #[error("UART stream was invalid utf8")]
    Utf8(#[from] Utf8Error),
    
    #[error("UART internal error")]
    Usart(#[from] embassy_stm32::usart::Error),
}
*/

pub async fn parse_uart_tx_as_utf8<'a, const N: usize>(
    usart_rx: &mut UartRx<'static, Async>, 
    buf: &'a mut [u8; N]) 
    -> Result<&'a str, String<64>> 
{
    let res = usart_rx.read_until_idle(buf).await;
    match res {
        Ok(len) => {
            match core::str::from_utf8(&buf[..len]) {
                Ok(msg) => {
                    Ok(msg.trim())
                },
                Err(_err) => {
                    hprintln!("Received invalid utf-8 over USART, ignore transmission");
                    //Err(err.into())
                    Err("Received invalid utf-8 over USART, ignore transmission".try_into().unwrap())
                },
            }
        }
        Err(_err) => {
            // todo: more error infos
            hprintln!("USART related error, ignore transmission");
            //Err(err.into())
            Err("USART related error, ignore transmission".try_into().unwrap())
        }
    }
}