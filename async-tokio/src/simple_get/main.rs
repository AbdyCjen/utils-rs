use std::{collections::HashMap, env, error::Error, io::Write, net::Shutdown};
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::TcpStream,
};
use url::{ParseError, Position, Url};

const MAX_LINE_LEN: usize = 1024;

// 看看能不能通过实现tokio::codec 来简化代码
struct HttpReq<'a> {
	host: &'a str,
	port: u16,
	path: &'a str,
	reader: Option<TcpStream>,
	header: Vec<(String, String)>,
}

// 杀妈了, http头的换行是\r\n
async fn read_line(reader: &mut TcpStream, buf: &mut [u8]) -> Result<usize, Box<dyn Error>> {
	let n = reader.peek(buf).await?;
	let mut m = 0;
	for (i, &c) in buf[..n].iter().enumerate() {
		if c == b'\r' {
			m = i;
			break;
		}
	}
	reader.read_exact(&mut buf[..m + 2]).await?;
	Ok(m)
}

impl<'a> HttpReq<'a> {
	pub fn from_url(url: &'a Url) -> Result<HttpReq<'a>, Box<dyn Error>> {
		let host = url.host_str().ok_or(ParseError::EmptyHost)?;
		let port = url.port_or_known_default().ok_or(ParseError::InvalidPort)?;
		let path = &url[Position::BeforePath..];
		let mut header = Vec::new();
		// build basic header
		header.push(("Host".to_owned(), format!("{}:{}", host, port)));
		header.push(("User-Agent".to_owned(), "curl/7.67.0".to_owned()));
		header.push(("Accept".to_owned(), "*/*".to_owned()));
		Ok(HttpReq { host,
		             port,
		             path,
		             reader: None,
		             header })
	}

	// TODO: send不直接打印结果, 返回一个reader回去呢;
	pub async fn send(&mut self) -> Result<(), Box<dyn Error>> {
		self.reader = Some(TcpStream::connect((self.host, self.port)).await?);
		let (_, mut writer) = self.reader.as_mut().unwrap().split();

		let mut req = Vec::new();
		writeln!(&mut req, "GET {} HTTP/1.1", self.path)?;
		for (k, v) in self.header.iter() {
			writeln!(&mut req, "{}: {}", k, v)?;
		}
		writeln!(&mut req)?;
		writer.write(&req).await?;
		TcpStream::shutdown(self.reader.as_mut().unwrap(), Shutdown::Write)?;

		let resp_header = self.read_header().await?;
		if let Some(v) = resp_header.get(b"Transfer-Encoding".as_ref()) {
			if v.contains(&b"chunked".to_vec()) {
				println!("yoxi: chunked");
				self.read_chunk().await?;
			} else {
				println!("Unknown tranfer encoding");
			}
		} else if let Some(v) = resp_header.get(b"Content-Length".as_ref()) {
			let len: usize = std::str::from_utf8(&v[0])?.parse()?;
			println!("Yoxi: length {}", len);
			self.read_sized(len).await?;
		}
		Ok(())
	}

	async fn read_header(&mut self) -> Result<HashMap<Vec<u8>, Vec<Vec<u8>>>, Box<dyn Error>> {
		fn split_header(line: &[u8]) -> Option<(&[u8], &[u8])> {
			let i = line.iter().enumerate().find(|(_, &c)| c == b':')?.0;
			let j = line[i + 1..].iter()
			                     .enumerate()
			                     .find(|(_, &c)| c == b' ')?
			                     .0;
			Some((&line[..i], &line[i + j + 1..]))
		}

		let reader = self.reader.as_mut().unwrap();
		let mut header = HashMap::new();

		let mut first_line = [0; MAX_LINE_LEN];
		let _ = read_line(reader, &mut first_line).await?;

		loop {
			let mut buf = [0_u8; MAX_LINE_LEN];
			let n = read_line(reader, &mut buf).await?;
			let line = &buf[..n];
			if n > 1 {
				let (k, v) = split_header(line).unwrap();
				header.entry(k.to_vec())
				      .or_insert_with(Vec::new)
				      .push(v.to_vec());
			} else {
				break Ok(header);
			}
		}
	}

	async fn read_all(self: &mut Self) -> Result<(), Box<dyn Error>> {
		let mut buf = [0_u8; 1024];
		let reader = self.reader.as_mut().unwrap();
		loop {
			let n = reader.read(&mut buf).await?;
			if n == 0 {
				break Ok(());
			}
			std::io::stdout().write_all(&buf[..n])?;
		}
	}

	async fn read_sized(self: &mut Self, mut to_read: usize) -> Result<(), Box<dyn Error>> {
		let reader = self.reader.as_mut().unwrap();
		let mut buf = [0; 1024];
		while to_read != 0 {
			let cur_buf = std::cmp::min(to_read, buf.len());
			let n = reader.read(&mut buf[..cur_buf]).await?;
			if n == 0 {
				break;
			}
			to_read -= n;
			std::io::stdout().write_all(&buf[..n])?;
		}
		Ok(())
	}

	async fn read_chunk(self: &mut Self) -> Result<usize, Box<dyn Error>> {
		let reader = self.reader.as_mut().unwrap();
		let mut total_read: usize = 0;
		loop {
			let mut size_line = [0_u8; 16];
			let n = read_line(reader, &mut size_line).await?;
			let mut to_read = usize::from_str_radix(std::str::from_utf8(&size_line[..n])?, 16)?;
			if to_read == 0 {
				break Ok(total_read);
			}

			let mut buf = [0_u8; 1024];
			while to_read != 0 {
				let cur_buf = std::cmp::min(buf.len(), to_read);
				let n = reader.read(&mut buf[..cur_buf]).await?;
				if n == 0 {
					break;
				}
				std::io::stdout().write_all(&buf[..n])?;
				to_read -= n;
				total_read += n;
			}
			// http nmsl, chunk读完还有一个换行;
			read_line(reader, &mut size_line).await?;
		}
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let url_str = env::args().nth(1)
	                         .unwrap_or_else(|| "http://127.0.0.1:1080/SomePath".to_string());
	let url = Url::parse(&url_str)?;
	let mut req = HttpReq::from_url(&url)?;
	req.send().await?;

	Ok(())
}
