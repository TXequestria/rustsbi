mod parser;
mod get_essentials;
mod dtb {

    mod fdt_header {
        macro_rules! fdt_header {
    ($vis:vis struct $name:ident {
        $(uint32_t $field:ident;)*
    }$(;)?) => {
        #[repr(C,align(8))]
        #[derive(Clone,Copy)]
        $vis struct $name {
            $($field:u32),*
        }
        impl $name {
            $($vis const fn $field(&self) -> usize {
                u32::from_be(self.$field) as usize
            })*
        }
    };
    }

        fdt_header! {
            pub(super) struct FdtHeader {
                uint32_t magic;
                uint32_t totalsize;
                uint32_t off_dt_struct;
                uint32_t off_dt_strings;
                uint32_t off_mem_rsvmap;
                uint32_t version;
                uint32_t last_comp_version;
                uint32_t boot_cpuid_phys;
                uint32_t size_dt_strings;
                uint32_t size_dt_struct;
            };
        }
    }

    use core::{
        marker::PhantomData,
        ptr::{NonNull, slice_from_raw_parts},
    };

    use fdt_header::FdtHeader;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct MemRsvEntry {
        address: u64,
        size: u64,
    }

    impl MemRsvEntry {
        pub const fn address(&self) -> usize {
            u64::from_be(self.address) as usize
        }
        pub const fn size(&self) -> usize {
            u64::from_be(self.size) as usize
        }
    }

    pub struct MemRsvMap<'a> {
        ptr: Option<NonNull<MemRsvEntry>>,
        _p: PhantomData<&'a DeviceTreeBlob>,
    }

    unsafe impl<'a> Sync for MemRsvMap<'a> {}

    unsafe impl<'a> Send for MemRsvMap<'a> {}

    impl<'a> Iterator for MemRsvMap<'a> {
        type Item = MemRsvEntry;
        fn next(&mut self) -> Option<Self::Item> {
            let ptr = self.ptr.take()?;
            if !ptr.is_aligned() {
                return None;
            }
            let entry = unsafe { ptr.read() };
            if entry.address() == 0 && entry.size() == 0 {
                None
            } else {
                unsafe { self.ptr = Some(ptr.add(1)) };
                Some(entry)
            }
        }
    }

    #[repr(C)]
    pub struct DeviceTreeBlob {
        header: FdtHeader,
        body: [u8],
    }

    impl DeviceTreeBlob {
        pub unsafe fn new<'a>(header: usize) -> Option<&'a Self> {
            let header = header as *const FdtHeader;
            if !header.is_aligned() {
                return None;
            }
            if header.is_null() {
                return None;
            }
            let header = unsafe { &*header };
            if header.magic() != 0xd00dfeed {
                return None;
            }
            assert_eq!(header.off_dt_struct() % 4, 0);
            assert_eq!(header.size_dt_struct() % 4, 0);
            assert_eq!(header.off_mem_rsvmap() % 8, 0);
            assert!(header.off_dt_strings() >= header.off_dt_struct() + header.size_dt_struct());
            assert!(header.off_dt_strings() + header.size_dt_strings() <= header.totalsize());
            assert!(header.off_mem_rsvmap() + size_of::<MemRsvEntry>() <= header.off_dt_struct());
            assert_eq!(header.last_comp_version(), 16);
            assert!(header.version() >= 16);

            // Safety: metadata in fat pointer is preserved during cast
            let body_len = header.totalsize() - size_of::<FdtHeader>();

            let slice = slice_from_raw_parts(header as *const _ as *const u8, body_len);

            let this = slice as *const Self;

            Some(unsafe { &*this })
        }
        pub(super) fn dt_struct(&self) -> (NonNull<u32>, NonNull<u32>) {
            let start_offset = self.header.off_dt_struct() - size_of::<FdtHeader>();
            let start: NonNull<u32> = NonNull::from_ref(&self.body[start_offset]).cast();
            let end = unsafe { start.byte_add(self.header.size_dt_struct()) };
            assert!(start.is_aligned());
            assert!(end.is_aligned());
            (start, end)
        }
        pub(super) fn dt_strings(&self) -> &[u8] {
            let start_offset = self.header.off_dt_strings() - size_of::<FdtHeader>();
            let len = self.header.size_dt_strings();
            &self.body[start_offset..start_offset + len]
        }
        pub fn mem_rsvmap(&self) -> MemRsvMap<'_> {
            let ptr = (self as *const Self).addr();
            let ptr = ptr + self.header.off_mem_rsvmap();
            let ptr = ptr as *const MemRsvEntry;
            MemRsvMap {
                ptr: NonNull::new(ptr.cast_mut()),
                _p: PhantomData,
            }
        }
        pub fn boot_cpu(&self) -> usize {
            self.header.boot_cpuid_phys()
        }
    }
}

pub struct CpuIter<'a> {
    inner: Option<DtbWalker<'a>>,
    depth: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct CpuInfo<'a> {
    pub isa_string: Option<&'a [u8]>,
    pub cpuid: u32,
    pub cbom: Option<u32>,
    pub enabled: bool,
}

impl<'a> Iterator for CpuIter<'a> {
    type Item = CpuInfo<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut walker = self.inner.take()?;
        let mut is_cpu = false;
        let mut enabled = true;
        let mut cpuid = None;
        let mut isa_string = None;
        let mut cbom = None;
        while let Some(data) = walker.next() {
            match data {
                DataType::NodeStart(_) => {
                    self.depth += 1;
                    if is_cpu {
                        let info = CpuInfo {
                            isa_string,
                            cpuid: cpuid?,
                            cbom,
                            enabled,
                        };
                        self.inner = Some(walker);
                        return Some(info);
                    }
                    is_cpu = false;
                    enabled = true;
                    cpuid = None;
                    isa_string = None;
                    cbom = None;
                },
                DataType::Prop { name, data } => {
                    if name == c"reg" {
                        let bytes: [u8; 4];
                        if data.len() == 4 {
                            bytes = *data.first_chunk().unwrap();
                        } else {
                            bytes = *data[4..].first_chunk().unwrap();
                        }
                        cpuid = Some(u32::from_be_bytes(bytes));
                    } else if name == c"riscv,isa-extensions" {
                        isa_string = Some(data);
                    } else if name == c"riscv,cbom-block-size" {
                        let bytes: [u8; 4] = *data.first_chunk().unwrap();
                        cbom = Some(u32::from_be_bytes(bytes))
                    } else if name == c"device_type" {
                        is_cpu = data == b"cpu\0";
                    } else if name == c"status" && data != b"okay\0" && data != b"ok\0" {
                        enabled = false;
                    }
                },
                DataType::NodeEnd => {
                    self.depth -= 1;
                    if self.depth < 0 {
                        return None;
                    }
                    if !is_cpu {
                        enabled = true;
                        cpuid = None;
                        isa_string = None;
                        cbom = None;
                        continue;
                    }
                    let info = CpuInfo {
                        isa_string,
                        cpuid: cpuid?,
                        cbom,
                        enabled,
                    };
                    self.inner = Some(walker);
                    return Some(info);
                },
            }
        }

        None
    }
}

pub use dtb::{DeviceTreeBlob, MemRsvEntry, MemRsvMap};
use parser::{DataType, DtbWalker};
impl DeviceTreeBlob {
    pub fn cpus(&self) -> CpuIter<'_> {
        let mut walker = DtbWalker::new(self);

        while let Some(data) = walker.next() {
            if let DataType::NodeStart(name) = data {
                if name == c"cpus" {break;}
            }
        }

        while walker.peek_prop() {
            let _ = walker.next();
        }

        CpuIter { inner:Some(walker), depth:0 }
    }
    pub fn memory(&self) -> (usize,usize) {
        let mut walker = DtbWalker::new(self);
        for data in &mut walker {
            match data {
                DataType::NodeStart(name) => {
                    if name.to_bytes().starts_with(b"memory") {
                        break;
                    }
                },
                _ => {continue;}
            }
        }
        let mut is_mem = false;
        let mut mem = None;
        for data in &mut walker {
            match data {
                DataType::Prop { name, data } => {
                    if name == c"device_type" {
                        is_mem = true;
                    }else if name == c"reg" {
                        if data.len() == size_of::<u64>() * 2 {
                            let start = data as *const _ as *const [u8;8];
                            let end = unsafe {start.add(1)};
                            unsafe {
                                mem = Some((
                                    u64::from_be_bytes(*start) as usize,
                                    u64::from_be_bytes(*end) as usize
                                ))
                            }
                        }else if data.len() == size_of::<u32>()*2 {
                            let start = data as *const _ as *const [u8;4];
                            let end = unsafe {start.add(1)};
                            unsafe {
                                mem = Some((
                                    u32::from_be_bytes(*start) as usize,
                                    u32::from_be_bytes(*end) as usize
                                ))
                            }
                        }else{
                            panic!("nonstandard reg size")
                        }
                    }
                },
                _ => break,
            }
        }
        if !is_mem {mem = None;}
        mem.unwrap()
    }
}
