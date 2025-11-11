mod parser;

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

pub struct PropsIter<'a> {
    inner: Option<DtbWalker<'a>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Prop<'a> {
    pub name: &'a CStr,
    pub data: &'a [u8],
}

impl<'a> Iterator for PropsIter<'a> {
    type Item = Prop<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut walker = self.inner.take()?;

        if !walker.peek_prop() {
            return None;
        }

        match walker.next()? {
            DataType::Prop { name, data } => {
                self.inner = Some(walker);
                Some(Prop { name, data })
            }
            _ => None,
        }
    }
}

pub use dtb::{DeviceTreeBlob, MemRsvEntry, MemRsvMap};

use core::ffi::CStr;
use parser::{DataType, DtbWalker};
pub struct Node<'a> {
    name: &'a CStr,
    inner: DtbWalker<'a>,
}

impl<'a> Node<'a> {
    pub fn subnode_by_path(&self, path: &str) -> Option<Self> {
        let rela_path = RelaPath::new(path);
        let mut current_path = PathBuf::new();
        let mut depth = 0;
        let mut walker = self.inner.clone();

        while let Some(data) = walker.next() {
            match data {
                DataType::NodeEnd => {
                    depth -= 1;
                    current_path.pop();
                    if depth < 0 {
                        return None;
                    }
                }
                DataType::Prop { name: _, data: _ } => {
                    continue;
                }
                DataType::NodeStart(name) => {
                    depth += 1;
                    current_path.push(name);
                    if current_path == rela_path {
                        return Some(Self {
                            name,
                            inner: walker.clone(),
                        });
                    }
                }
            }
        }

        None
    }
    pub fn prop_by_name(&self, name: &str) -> Option<&'a [u8]> {
        let name = name.trim();
        let mut walker = self.inner.clone();
        while walker.peek_prop() {
            let Some(DataType::Prop {
                name: inner_name,
                data,
            }) = walker.next()
            else {
                break;
            };
            if inner_name.to_bytes().eq(name.as_bytes()) {
                return Some(data);
            }
        }
        None
    }

    pub fn props(&self) -> PropsIter<'a> {
        PropsIter {
            inner: Some(self.inner.clone()),
        }
    }

    pub fn name(&self) -> &'a CStr {
        self.name
    }
}

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
    pub fn root_node(&self) -> Node<'_> {
        let mut walker = DtbWalker::new(self);
        let mut root = None;
        while let Some(data) = walker.next() {
            if let DataType::NodeStart(name) = data {
                if name == c"" {
                    root = Some(Node {
                        name,
                        inner: walker,
                    });
                    break;
                }
            }
        }
        root.expect("missing root node")
    }
    pub fn node_by_path(&self, path: &str) -> Option<Node<'_>> {
        let mut path = path.trim();
        if let Some(index) = path.find(':') {
            (path, _) = path.split_at(index)
        }
        let mut pathbuf = PathBuf::new();
        let mut walker = DtbWalker::new(self);
        let mut node = None;
        while let Some(data) = walker.next() {
            if let DataType::NodeStart(name) = data {
                pathbuf.push(name);
                if pathbuf == path {
                    node = Some(Node {
                        name,
                        inner: walker,
                    });
                    break;
                }
            } else if let DataType::NodeEnd = data {
                pathbuf.pop();
            }
        }
        node
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
    pub fn nodes_with_prop<'a, 'b>(&'a self, prop: &'b str) -> NodesWithNamedProp<'a, 'b> {
        NodesWithNamedProp {
            inner: Some(DtbWalker::new(self)),
            prop_name: prop,
        }
    }
}

pub struct NodesWithNamedProp<'a, 'b> {
    inner: Option<DtbWalker<'a>>,
    prop_name: &'b str,
}

impl<'a, 'b> Iterator for NodesWithNamedProp<'a, 'b> {
    type Item = Node<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut walker = self.inner.take()?;
        let mut backup = None;
        while let Some(data) = walker.next() {
            match data {
                DataType::NodeEnd => {
                    backup = None;
                }
                DataType::NodeStart(name) => {
                    backup = Some((name, walker.clone()));
                }
                DataType::Prop { name, data: _ } => {
                    if name.to_bytes() == self.prop_name.as_bytes() {
                        self.inner = Some(walker);
                        let (name, inner) = backup?;
                        return Some(Node { name, inner });
                    }
                }
            }
        }

        None
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
struct RelaPath<'a> {
    path: &'a str,
}

impl<'a> RelaPath<'a> {
    fn new(path: &'a str) -> Self {
        let mut path = path.trim();
        if let Some(i) = path.find(':') {
            (path, _) = path.split_at(i);
        }
        path = path.trim_matches('/');
        Self { path }
    }
}

struct PathBuf {
    buffer: [u8; 256],
    len: u8,
}

impl PartialEq<CStr> for PathBuf {
    fn eq(&self, other: &CStr) -> bool {
        if self.len() != other.count_bytes() {
            return false;
        }
        self.buffer[..self.len()].eq(other.to_bytes())
    }
}

impl PartialEq<str> for PathBuf {
    fn eq(&self, other: &str) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.buffer[..self.len()].eq(other.as_bytes())
    }
}

impl PartialEq<RelaPath<'_>> for PathBuf {
    fn eq(&self, other: &RelaPath<'_>) -> bool {
        let len = self.len();

        if len == 0 {
            return false;
        }
        // skip the first '/'
        let bytes = &self.buffer[1..len];

        bytes.eq(other.path.as_bytes())
    }
}

impl PartialEq<&RelaPath<'_>> for PathBuf {
    fn eq(&self, other: &&RelaPath<'_>) -> bool {
        self.eq(*other)
    }
}

impl PartialEq<&str> for PathBuf {
    fn eq(&self, other: &&str) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.buffer[..self.len()].eq(other.as_bytes())
    }
}

impl PathBuf {
    fn push(&mut self, node_name: &CStr) {
        if (1 + node_name.count_bytes() + self.len as usize) > self.buffer.len() {
            return;
        }
        self.len += 1;
        self.buffer[self.len()] = b'/';
        let start = self.len();
        let end = start + node_name.count_bytes();
        self.buffer[start..end].copy_from_slice(node_name.to_bytes());
    }
    fn pop(&mut self) {
        let mut last_slash = self.len();

        while last_slash > 0 {
            last_slash = last_slash - 1;
            let is_slash = self.buffer[last_slash] == b'/';
            self.buffer[last_slash] = 0;
            if is_slash {
                break;
            }
        }

        if last_slash == 0 {
            self.buffer[0] = b'/';
            self.len = 1;
        } else {
            self.len = last_slash as u8;
        }
    }
    fn len(&self) -> usize {
        self.len as usize
    }
    fn new() -> Self {
        Self {
            buffer: [0; 256],
            len: 0,
        }
    }
}
