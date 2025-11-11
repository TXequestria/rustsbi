use crate::FwBootInfo;

const PAYLOAD_BYTES:&[u8] = include_bytes!("../../../config/payload.bin");

#[repr(C,align(0x1000))]
struct Payload<const N:usize> {
    bytes:[u8;N]
}

impl<const N:usize> Payload<N> {
    const fn new(bytes:&[u8]) -> Self {
        let mut new = Self {bytes:[0;N]};
        new.bytes.copy_from_slice(bytes);
        new
    }
}

#[unsafe(link_section = ".payload")]
#[used]
static PAYLOAD:Payload<{PAYLOAD_BYTES.len()}> = Payload::new(PAYLOAD_BYTES);

pub unsafe fn get_fw_boot_info(opaque:usize) -> Option<FwBootInfo> {
    Some(FwBootInfo{
        option:0,
        next_mode:crate::NextMode::PrivS,
        next_addr:&raw const PAYLOAD as usize
    })
}