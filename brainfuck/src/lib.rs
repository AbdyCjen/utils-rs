use std::{borrow::BorrowMut, collections::HashSet, fs::File, io::prelude::*};

#[derive(Debug)]
pub enum BFErr {
	IOErr,
	SegErr,
}

pub struct BF {
	st: [u8; 3000],
	text: Vec<u8>,
}

impl BF {
	pub fn from_reader<R: std::io::Read>(mut r: R) -> Result<BF, BFErr> {
		let mut text = Vec::new();
		r.read_to_end(&mut text).or(Err(BFErr::IOErr))?;
        Ok(BF { 
            st: [0_u8; 3000],
            text 
        })
	}

	pub fn from_slice_u8(slc: &[u8]) -> BF {
		BF { st: [0; 3000],
		     text: slc.iter().map(|c| *c).collect() }
	}

	pub fn run(&mut self) -> Result<(), BFErr> {
		let symbols: HashSet<u8> = b"><+-.,[]".iter().map(|c| *c).collect();
		let (mut ip, mut tp) = (0, 0);

		while let Some(&c) = self.text.get(ip) {
			if symbols.contains(&c) {
				match c {
					b'[' => {
						if 0 == *self.st.get(tp).ok_or(BFErr::SegErr)? {
							while let Some(&c) = self.text.get(ip) {
								if c == b']' { break }
								ip += 1;
							}
						}
					}
					b']' => {
						if 0 != *self.st.get(tp).ok_or(BFErr::SegErr)? {
							while let Some(&c) = self.text.get(ip) {
								if c == b'[' { break }
								ip -= 1;
							}
						}
					}
					b'<' => tp -= 1,
					b'>' => tp += 1,
					b'+' => *self.st.get_mut(tp).ok_or(BFErr::SegErr)? += 1,
					b'-' => *self.st.get_mut(tp).ok_or(BFErr::SegErr)? -= 1,
					b'.' => {
						std::io::stdout().write(&[*self.st.get(tp).ok_or(BFErr::SegErr)?])
						                 .or(Err(BFErr::IOErr))?;
					}
					b',' => {
						std::io::stdin().read(
						                      &mut self.st
						                               .get_mut(tp..tp + 1)
						                               .ok_or(BFErr::SegErr)?,
						)
						                .or(Err(BFErr::IOErr))?;
					}
					_ => unreachable!(),
				}
			}
			ip += 1
		}

		Ok(())
	}
	//fn reset(&mut self)  /* reset st */
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn hello_world() -> Result<(), BFErr> {
		let text = b"++++++++++[>+++++++>++++++++++>+++>+<<<<-]\
                    >++.>+.+++++++..+++.>++.<<+++++++++++++++.\
                    >.+++.------.--------.>+.>.";
		let mut bf = BF::from_slice_u8(text);
		bf.run()
	}

	#[test]
	fn echo_back() -> Result<(), BFErr> {
		let text = b",[.,]";
		let mut bf = BF::from_slice_u8(text);
		bf.run()
	}
}

/*fn main() -> Result<(), BFErr> {
	let file_name = std::env::args().skip(1).next().expect("plz specify a file");
	let f = File::open(file_name).or(Err(BFErr::IOErr))?;
	let mut bf = BF::from_reader(f)?;
	bf.run()
}*/
