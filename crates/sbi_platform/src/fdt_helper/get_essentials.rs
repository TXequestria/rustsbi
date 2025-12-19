use super::parser::{DataType, DtbWalker};

use core::ffi::CStr;

struct PathBuf {
    len: u8,
    buf: [u8; 255],
}

impl PathBuf {
    #[inline(always)]
    const fn new() -> Self {
        let mut buf = [0; 255];
        buf[0] = b'/';
        Self { len: 1, buf }
    }
    #[inline(always)]
    const fn len(&self) -> usize {
        self.len as _
    }
    #[inline(always)]
    fn push(&mut self, path: &CStr) {
        let path = path.to_bytes().trim_ascii();
        match (path.first(), path.last()) {
            (Some(first), Some(last)) => {
                assert_ne!(*first, b'/');
                assert_ne!(*last, b'/')
            }
            // path is empty
            _ => {
                return;
            }
        }
        let new_len = 1 + path.len() + self.len();
        if new_len > 255 {
            panic!("Path too long!")
        }
        self.buf[self.len()] = b'/';
        let start = self.len() + 1;
        self.buf[start..new_len].copy_from_slice(path);
    }
    #[inline(always)]
    fn pop(&mut self) {
        if self.len() <= 1 {
            return;
        }
        let mut slash_count = 0;
        loop {
            // safety: checked len > 1
            let new_len = unsafe { self.len.unchecked_sub(1) };
            if new_len == 0 {
                return;
            }
            self.len = new_len;
            let last_char = unsafe { *self.buf.get_unchecked(new_len as usize) };
            if last_char == b'/' {
                slash_count += 1;
                if slash_count >= 2 {
                    return;
                }
            }
        }
    }
    #[inline(always)]
    fn starts_with(&self, prefix: &str) -> bool {
        let prefix = prefix.trim().as_bytes();
        self.buf[0..self.len()].starts_with(prefix)
    }
}

impl PartialEq<str> for PathBuf {
    fn eq(&self, other: &str) -> bool {
        let other = other.trim();
        if self.len() != other.len() {
            return false;
        }
        self.buf[0..self.len()].eq(other.as_bytes())
    }
}

impl PartialEq<&str> for PathBuf {
    fn eq(&self, other: &&str) -> bool {
        let other = other.trim();
        if self.len() != other.len() {
            return false;
        }
        self.buf[0..self.len()].eq(other.as_bytes())
    }
}

pub(super) fn parse_essentials(dtb: DtbWalker) {
    let mut dtb = dtb;
    let mut path = PathBuf::new();
    let mut node_start = dtb.clone();
    let mut memory = None;
    while let Some(data) = dtb.next() {
        let (name, data) = match data {
            DataType::NodeStart(name) => {
                path.push(name);
                node_start = dtb.clone();
                if path.starts_with("/memory") {
                    memory = parse_memory(&mut dtb);
                }
                continue;
            }
            DataType::Prop { name, data } => (name, data),
            DataType::NodeEnd => {
                path.pop();
                continue;
            }
        };
    }
}

#[inline]
fn parse_memory(dtb: &mut DtbWalker) -> Option<(usize, usize)> {
    let mut memory = None;
    let mut is_mem = false;
    while let Some(data) = dtb.next() {
        match data {
            DataType::Prop { name, data } => {
                if name == c"device_type" {
                    is_mem = data == b"memory\0";
                } else if name == c"reg" {
                    if data.len() == 16 {
                        let data = data as *const _ as *const [u8; 8];
                        let addr = u64::from_be_bytes(unsafe { data.read() });
                        let size = u64::from_be_bytes(unsafe { data.add(1).read() });
                        memory = Some((addr as _, size as _))
                    } else if data.len() == 8 {
                        let data = data as *const _ as *const [u8; 4];
                        let addr = u32::from_be_bytes(unsafe { data.read() });
                        let size = u32::from_be_bytes(unsafe { data.add(1).read() });
                        memory = Some((addr as _, size as _))
                    }
                }
            }
            _ => {
                break;
            }
        }
    }
    if is_mem { memory } else { None }
}
