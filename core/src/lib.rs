#![no_std]
// stable in 1.95
#![feature(assert_matches)]

pub mod activities;
pub mod application;
pub mod battery;
pub mod container;
pub mod display;
pub mod framebuffer;
pub mod fs;
pub mod input;
pub mod layout;
pub mod res;

extern crate alloc;
extern crate embedded_zip as zip;
extern crate embedded_xml as xml;
