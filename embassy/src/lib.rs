#![no_std]

//! This crate provides a software-abstraction-layer (SAL) for typical 
//! hardware tasks and a board support package for the NUCLEO-F767ZI
//! 
//! The SAL supports
//! 
//! - [x] LED handling
//! - [x] UART Status Reports
//! - [x] UART Command Interpretation

pub mod led;
pub mod uart;
pub mod cmd;
