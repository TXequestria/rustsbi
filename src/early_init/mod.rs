mod assembly;
use sbi_firmware::{FwBootInfo, get_fw_boot_info};
use sbi_platform::platform::MemorySegment;
use bump_alloc::BumpAlloc;
use core::mem::MaybeUninit;
use serial::uart16550::Builder;
use crate::serial::init_serial;
static mut FW_BOOT_INFO: FwBootInfo = FwBootInfo::new();
static mut MEMORY_LAYOUT: MemorySegment = MemorySegment::new();
static mut FDT_PTR: usize = 0;
static mut BUMP_ALLOCATOR: MaybeUninit<BumpAlloc> = MaybeUninit::zeroed();

unsafe extern "C" fn early_init(fdt: usize, a2: usize) {
    use sbi_alloc::set_global_allocator;
    use bump_alloc::BumpAlloc;
    unsafe {
        let info = get_fw_boot_info(a2).unwrap();
        FW_BOOT_INFO = info;
        FDT_PTR = fdt;
        let layout = MemorySegment::init_memory_region(fdt);
        let alloc = (&raw mut BUMP_ALLOCATOR) as *mut BumpAlloc;
        alloc.write(
            BumpAlloc::new(
                layout.heap_start()
                , layout.heap_start() + 1024*1024
            ).unwrap()
        );
        MEMORY_LAYOUT = layout;
        set_global_allocator(alloc);
        init_serial(Builder::new());
    };
}

