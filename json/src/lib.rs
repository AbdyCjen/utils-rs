/*
 * 一个json parser的实验性实现
 * 参考资料: http://www.ecma-international.org/publications/files/ECMA-ST/ECMA-404.pdf
 */

use std::{collections::HashMap, io::Read, string::String};
use thiserror::Error;

#[derive(Debug, PartialEq)]
pub enum Value {
	Null,
	Bool(bool),
	Number(i64),
	String(String),
	Array(Vec<Value>),
	Object(HashMap<String, Value>),
}

#[derive(Error, Debug, PartialEq)]
pub enum ParseError {
	#[error("Default Error")]
	DefaultErr,
	#[error("Unexpected Symbol {0}")]
	UnexpectSymErr(u8),
	#[error("Unexpected EOF")]
	UnexpectedEof,
	#[error("Utf-8 Decode Error")]
	Utf8Err,
}

#[inline]
fn skip_space<I: Iterator<Item = u8>>(
	it: &mut I,
) -> std::iter::SkipWhile<&mut I, impl FnMut(&u8) -> bool> {
	it.by_ref().skip_while(u8::is_ascii_whitespace)
}

use ParseError::*;
impl Value {
	// 写个impl From<Iterator<Item=u8>> for Iterator<Item=char>
	#[inline]
	fn get_string<I>(it: &mut I) -> Result<String, ParseError>
	where I: Iterator<Item = u8> {
		let mut esc = None;
		let mut buf = Vec::new();
		for c in it {
			if esc.take().is_some() {
				match c {
					b'"' => buf.push(b'"'),
					b'\\' => buf.push(b'\\'),
					b'/' => buf.push(b'/'),
					b'b' => buf.push(b'\x08'),
					b'f' => buf.push(b'\x0c'),
					b'n' => buf.push(b'\n'),
					b'r' => buf.push(b'\r'),
					b't' => buf.push(b'\t'),
					b'u' => todo!("支持Unicode码点表示"),
					_ => return Err(UnexpectSymErr(c)),
				}
			} else {
				// FIXME: support utf-8 input stream or unescaped non-ascii char will panic
				match c {
					0x0..=0x1f => return Err(UnexpectSymErr(c)),
					b'"' => return String::from_utf8(buf).map_err(|_| Utf8Err),
					b'\\' => esc = Some(()),
					c => buf.push(c),
				}
			}
		}
		Err(UnexpectedEof)
	}

	//TODO : 支持类似+3.2e+32和普通的浮点数/整数
	#[inline]
	fn get_number<I>(first: u8, it: &mut std::iter::Peekable<I>) -> i64
	where I: Iterator<Item = u8> {
		let mut v = (first - b'0') as i64;
		while let Some(c) = it.next_if(u8::is_ascii_digit) {
			v = 10 * v + (c - b'0') as i64;
			it.next();
		}
		v
	}

	fn get_object_content<I>(
		it: &mut std::iter::Peekable<I>,
	) -> Result<HashMap<String, Value>, ParseError>
	where I: Iterator<Item = u8> {
		let mut m = HashMap::new();
		'outter: loop {
			match skip_space(it).next().ok_or(UnexpectedEof)? {
				b'"' => {
					let k = Self::get_string(it)?;
					if !skip_space(it).next().ok_or(UnexpectedEof)?.eq(&b':') {
						return Err(DefaultErr);
					}
					m.entry(k).or_insert(Self::from_bytes(it)?);
					'inner: loop {
						match it.peek().ok_or(UnexpectedEof)? {
							c if c.is_ascii_whitespace() => {
								it.next();
								continue 'inner;
							}
							b',' => {
								it.next();
								break 'inner;
							}
							b'}' => {
								it.next();
								break 'outter Ok(m);
							}
							c => return Err(UnexpectSymErr(*c)),
						};
					}
				}
				c => return Err(UnexpectSymErr(c)),
			}
		}
	}

	fn get_object<I>(
		it: &mut std::iter::Peekable<I>,
	) -> Result<HashMap<String, Value>, ParseError>
	where I: Iterator<Item = u8> {
		loop {
			match it.peek().ok_or(UnexpectedEof)? {
				c if c.is_ascii_whitespace() => {
					it.next();
				}
				&c => {
					match c {
						b'"' => break Self::get_object_content(it),
						b'}' => {
							it.next();
							break Ok(HashMap::new());
						}
						c => break Err(UnexpectSymErr(c)),
					}
				}
			}
		}
	}

	#[inline]
	fn get_array_content<I>(it: &mut std::iter::Peekable<I>) -> Result<Vec<Value>, ParseError>
	where I: Iterator<Item = u8> {
		let mut v = vec![Self::from_bytes(it)?];
		loop {
			match skip_space(it).next().ok_or(UnexpectedEof)? {
				b']' => break Ok(v),
				b',' => v.push(Self::from_bytes(it)?),
				c => break Err(UnexpectSymErr(c)),
			}
		}
	}

	fn get_array<I>(it: &mut std::iter::Peekable<I>) -> Result<Vec<Value>, ParseError>
	where I: Iterator<Item = u8> {
		loop {
			match it.peek().ok_or(UnexpectedEof)? {
				b'{' | b'[' | b'"' | b't' | b'f' | b'n' | b'0'..=b'9' => {
					break Self::get_array_content(it)
				}
				&c => {
					match it.next().unwrap() {
						c if c.is_ascii_whitespace() => {}
						b']' => break Ok(Vec::new()),
						_ => break Err(UnexpectSymErr(c)),
					}
				}
			}
		}
	}

	pub fn from_bytes<I>(it: &mut std::iter::Peekable<I>) -> Result<Value, ParseError>
	where I: Iterator<Item = u8> {
		// 迭代器匹配字符串字面值
		#[inline]
		fn match_lit_str<I: Iterator<Item = u8>>(s: &str, it: &mut I) -> Result<(), ParseError> {
			let s = &s[1..];
			if !s.bytes().eq(it.by_ref().take(s.len())) {
				Err(DefaultErr)
			} else {
				Ok(())
			}
		}
		match skip_space(it).next().ok_or(UnexpectedEof)? {
			b'{' => Self::get_object(it).map(Self::Object),
			b'[' => Self::get_array(it).map(Self::Array),
			b'"' => Self::get_string(it).map(Self::String),
			b't' => match_lit_str("true", it).map(|_| Self::Bool(true)),
			b'f' => match_lit_str("false", it).map(|_| Self::Bool(false)),
			b'n' => match_lit_str("null", it).map(|_| Self::Null),
			c @ b'0'..=b'9' => Ok(Self::Number(Self::get_number(c, it))),
			c => Err(UnexpectSymErr(c)),
		}
	}

	pub fn from_reader<R: Read>(r: R) -> Result<Value, ParseError> {
		// FIXME: 用take_while和map捕获出现的IO错误
		let mut it = r.bytes().map(|c| c.unwrap()).peekable();
		Self::from_bytes(&mut it)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn it_works() {
		let value_from_str = |json_str: &str| -> Result<Value, ParseError> {
			let mut it = json_str.bytes().peekable();
			let v = Value::from_bytes(it.by_ref());
			match &v {
				Ok(v) => println!("Done parsing: {} => \n{}", json_str, v),
				Err(e) => println!("Done parsing: {} => {}", json_str, e),
			};
			v
		};
		let test_fun = |json_str: &str, val| {
			let v = value_from_str(json_str);
			assert_eq!(v.as_ref(), Ok(&val));
		};
		test_fun(r#""""#, Value::String("".to_owned()));
		test_fun(r#""a string""#, Value::String("a string".to_owned()));
		test_fun("\"a \\nstring\"", Value::String("a \nstring".to_owned()));
		test_fun(r#" true"#, Value::Bool(true));
		test_fun(r#" false"#, Value::Bool(false));
		test_fun("3244443214", Value::Number(3244443214));
		//test_fun("", Value::Null);
		value_from_str("[12 , 34 , 45 ,]").unwrap_err();
		value_from_str("[12 ,	 34 , 45 ]").unwrap();
		value_from_str("[12 , 34 45 ]").unwrap_err();
		value_from_str("[}").unwrap_err();
		value_from_str("[{]").unwrap_err();
		value_from_str("[1 , 2 , 3 , {]").unwrap_err();
		value_from_str("[]").unwrap();
		value_from_str(r#"{}"#).unwrap();
		value_from_str(r#"[{"a": 12]}"#).unwrap_err();
		value_from_str(r#"{"a":[12, 34 , 45]}"#).unwrap();
		value_from_str(r#"{"a":[12 , 34 , 45], "b": 32,}"#).unwrap_err();
		value_from_str(r#""abcded	fdsa""#).unwrap_err();
		value_from_str(r#"{ "method": "check_sign", "version": "2.0", "datas": [ "8311016124", "6D7F7F0276962D125AA27AA8DAFE9F26", "2AC2E3F68FC8CCFF23B5D246DA881ECD" ], "src": "posbill", "auth": "B800A6D044EC6324663F75B21A8786F3", "datas_sign": "5AE6549F61D73DCADEE8781D9702DE8E" }"#).unwrap();
		let s = r#"{ "license": "MIT", "private": true, "engines": { "node": ">=8" }, "devDependencies": { "autoprefixer": "9.6.1", "eslint": "6.0.1", "less": "3.9.0", "less-plugin-clean-css": "1.5.1", "postcss-cli": "6.1.3", "stylelint": "10.1.0", "stylelint-config-standard": "18.3.0", "updates": "8.5.0" }, "browserslist": [ "> 1%", "last 2 firefox versions", "last 2 safari versions", "ie 11" ] }"#;
		value_from_str(s).unwrap();
	}
	use std::fmt;
	// FIXME: ugly and buggy
	impl fmt::Display for Value {
		fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
			fn value_fmt(v: &Value, f: &mut fmt::Formatter, _idt_cnt: i32) -> fmt::Result {
				use Value::*;
				match v {
					Null => write!(f, "Null"),
					Bool(b) => write!(f, "{}", b),
					Number(n) => write!(f, "{}", n),
					String(s) => write!(f, "\"{}\"", s),
					Array(v) => {
						write!(f, "[")?;
						let mut it = v.iter();
						it.by_ref().take(1).fold(Ok(()), |_, v| write!(f, "{}", v))?;
						for v in it {
							write!(f, ", {}", v)?
						}
						write!(f, "]")
					}
					Object(o) => {
						write!(f, "{{")?;
						let mut it = o.iter();
						it.by_ref()
							.take(1)
							.fold(Ok(()), |_, (k, v)| write!(f, "\"{}\": {}", k, v))?;
						for (k, v) in it {
							write!(f, ",\"{}\": {}", k, v)?
						}
						write!(f, "}}")
					}
				}
			}
			value_fmt(self, f, 0)
		}
	}
}
