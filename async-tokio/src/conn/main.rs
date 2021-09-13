use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::TcpStream,
};

use std::{env, error::Error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let addr = env::args().nth(1)
	                      .unwrap_or_else(|| "127.0.0.1:8080".to_string());
	let mut stream = TcpStream::connect(addr).await?;
	println!("Stream Created!");
	let result = stream.write(b"Hello World!\n").await;
	println!("wrote to stream; success={:?}", result.is_ok());
	let mut buf1 = [0_u8; 128];
	let mut buf2 = [0_u8; 128];
	let n = stream.peek(&mut buf1).await?;
	println!("RECV: {}", std::str::from_utf8(&buf1[..n]).unwrap());
	assert_eq!(n, stream.read(&mut buf2[..n]).await?);
	Ok(())
}
