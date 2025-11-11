use core::{ffi::CStr,marker::PhantomData, ptr::{slice_from_raw_parts, NonNull}};

use super::dtb::{DeviceTreeBlob};



#[derive(Clone)]
pub(super) struct DtbWalker<'a> {
    current:Option<NonNull<u32>>,
    dt_end:Option<NonNull<u32>>,
    dt_strings:&'a [u8],
    unexpeted:Option<TokenType>,
    _p:PhantomData<&'a DeviceTreeBlob>
}

impl<'a> DtbWalker<'a> {
    pub(super) fn new(dtb:&'a DeviceTreeBlob) -> Self {
        let (current,dt_end) = dtb.dt_struct();
        let dt_strings = dtb.dt_strings();
        Self {
            current:Some(current),
            dt_end:Some(dt_end),
            dt_strings,
            unexpeted:None,
            _p:PhantomData
        }
    }
    pub(super) fn peek_prop(&self) -> bool {
        let mut ptr = match self.current {
            Some(p) => p, None => return false
        };
        let dt_end = match self.dt_end {
            Some(p) => p, None => return false
        };
        while ptr < dt_end {
            let token:TokenType = u32::from_be(unsafe {ptr.read()}).into();
            match token {
                TokenType::Nop => {unsafe {ptr=ptr.add(1)}},
                TokenType::Prop => {return true},
                _ => break
            }
        }
        false
    }
}

unsafe impl<'a> Send for DtbWalker<'a> {}
unsafe impl<'a> Sync for DtbWalker<'a> {}

impl<'a> Iterator for DtbWalker<'a> {
    type Item = DataType<'a>;
    fn next(&mut self) -> Option<Self::Item> {

        let mut ptr = self.current.take()?;
        let dt_end = self.dt_end?;
        while ptr < dt_end {
            let token:TokenType = u32::from_be(unsafe {ptr.read()}).into();
            unsafe {ptr = ptr.add(1);}
            if Some(token) == self.unexpeted {
                panic!("unexpected token {:?} at {:p}", token, ptr)
            }
            match token {
                TokenType::BeginNode => {
                    let max_len = (dt_end.addr().get() - ptr.addr().get()).min(32);
                    let str_buffer = slice_from_raw_parts(ptr.as_ptr() as *const u8, max_len);
                    let string = unsafe {
                        let string = CStr::from_bytes_until_nul(&*str_buffer).unwrap();
                        ptr = ptr.add(align_to_u32(string.count_bytes() + 1));
                        string
                    };
                    self.current = Some(ptr);
                    self.unexpeted = Some(TokenType::FdtEnd);
                    return Some(DataType::NodeStart(string));
                },
                TokenType::Nop => {continue;},
                TokenType::FdtEnd => {
                    assert_eq!(ptr,dt_end);
                    return None;
                }
                TokenType::Prop => {
                    let (name,data) = unsafe {
                        assert!(ptr.add(2) <= dt_end);
                        let len = u32::from_be(ptr.read()) as usize;
                        ptr = ptr.add(1);
                        let nameoff = u32::from_be(ptr.read()) as usize;
                        ptr = ptr.add(1);
                        assert!(ptr.addr().get() + len <= dt_end.addr().get());
                        let name = 
                            CStr::from_bytes_until_nul(&self.dt_strings[nameoff..])
                            .unwrap();
                        let data = &*slice_from_raw_parts(ptr.as_ptr() as *const u8, len);
                        ptr = ptr.add(align_to_u32(len));
                        (name,data)
                    };
                    self.current = Some(ptr);
                    self.unexpeted = Some(TokenType::FdtEnd);
                    return Some(DataType::Prop { name, data })
                },
                TokenType::EndNode => {
                    self.current = Some(ptr);
                    self.unexpeted = Some(TokenType::Prop);
                    return Some(DataType::NodeEnd)
                }
            }
        }

        panic!("unexpected end of dt_struct")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenType {
    BeginNode,
    EndNode,
    Prop,
    Nop,
    FdtEnd,
}

impl From<u32> for TokenType {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::BeginNode,
            2 => Self::EndNode,
            3 => Self::Prop,
            4 => Self::Nop,
            9 => Self::FdtEnd,
            unsupported => {
                panic!("unsupported token {unsupported}")
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum DataType<'a> {
    NodeStart(&'a CStr),
    Prop { name: &'a CStr, data: &'a [u8] },
    NodeEnd,
}



#[inline(always)]
fn align_to_u32(num:usize) -> usize {
    if num % size_of::<u32>() == 0 {
        num/4
    }else{
        1+(num/4)
    }
}