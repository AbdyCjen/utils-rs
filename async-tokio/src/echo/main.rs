use tokio::net::TcpListener;

use std::{env, error::Error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let addr = env::args().nth(1)
	                      .unwrap_or_else(|| "127.0.0.1:8080".to_string());

	let mut listener = TcpListener::bind(&addr).await?;
	loop {
		let (mut socket, peer_addr) = listener.accept().await?;
		tokio::spawn(async move {
			let (mut reader, mut writer) = socket.split();
			tokio::io::copy(&mut reader, &mut writer).await.unwrap();
			println!("{} disconnect", peer_addr);
		});
	}
}
