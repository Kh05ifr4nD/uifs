#![no_std]
#![no_main]

extern crate alloc;
use bytes::{BufMut, BytesMut};
use core::panic::PanicInfo;
use core::{mem::MaybeUninit, ptr::addr_of_mut};
use cortex_m_rt::entry;
use embedded_alloc::Heap;
use rtt_target::{rprintln, rtt_init_print};

use microbit::{
  hal::prelude::*,
  hal::uarte,
  hal::uarte::{Baudrate, Parity},
};

const HEAP_SIZE: usize = 8 * 1024;
static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

#[global_allocator]
static HEAP: Heap = Heap::empty();

static mut TX_BUF: [u8; 1] = [0; 1];
static mut RX_BUF: [u8; 1] = [0; 1];

#[entry]
fn main() -> ! {
  unsafe {
    HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE);
  }
  rtt_init_print!();
  let board = microbit::Board::take().unwrap();

  let serial =
    uarte::Uarte::new(board.UARTE0, board.uart.into(), Parity::EXCLUDED, Baudrate::BAUD115200);

  let (mut tx, mut rx) = serial
    .split(unsafe { &mut *addr_of_mut!(TX_BUF) }, unsafe { &mut *addr_of_mut!(RX_BUF) })
    .unwrap();

  let mut buf = BytesMut::with_capacity(100);

  buf.put_u8(0xC0);
  buf.put_u16(5 + 2 + 2);
  buf.put_u8(1);
  buf.put_u8(0);
  buf.put_u8(0);
  buf.put_u8(1);
  buf.put_u16(0);

  tx.bwrite_all(&buf).unwrap();
  tx.flush();
  buf.clear();

  // buf.put_u8(0xC0);
  // buf.put_u16(5 + 32 + 2);
  // buf.put_u8(0);
  // buf.put_u8(0);
  // buf.put_bytes(0xAF, 32);
  // buf.put_u16(0);

  // tx.bwrite_all(&buf).unwrap();
  // tx.flush();
  // buf.clear();

  loop {}
}

#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
  rprintln!("{}", info);
  loop {}
}
