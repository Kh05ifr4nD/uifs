use core::time::Duration;

use bytes::BytesMut;

#[tokio::main]
async fn main() {
  let buf = [0u8;65536];
  let mut buf = BytesMut::from(buf.as_slice());
  loop {
    tokio::time::sleep(Duration::from_secs(1)).await;
    let mut sp = serialport::new("COM3", 115_200).timeout(Duration::from_secs(1)).open().unwrap();
    
      println!("{:?}",sp.read(buf.as_mut()));
    
  
  }
}
