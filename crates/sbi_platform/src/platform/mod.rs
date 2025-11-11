use core::{
    arch::{asm, naked_asm},
    mem::offset_of,
    ops::Range,
};

use crate::fdt_helper::DeviceTreeBlob;
use sbi_config::PLATFORM_CONFIG;
use sbi_trap::SbiScratch;

pub struct MemorySegment {
    scratch_start: usize,
    stack_top: usize,
    enable_mask: HartEnableMask,
    total_hart_count: u32,
    cbom: u32,
    heap_start: usize,
    heap_end: usize,
    memory_start: usize,
    memory_end: usize,
}

const OFF_SCRATCH: usize = offset_of!(MemorySegment, scratch_start) / size_of::<usize>();
const OFF_STACK: usize = offset_of!(MemorySegment, stack_top) / size_of::<usize>();

#[cfg(target_arch = "riscv64")]
const _: () = const {
    if offset_of!(MemorySegment, scratch_start) % 8 != 0 {
        panic!("unaligned")
    }
    if offset_of!(MemorySegment, stack_top) % 8 != 0 {
        panic!("unaligned")
    }
};
#[cfg(target_arch = "riscv32")]
const _: () = const {
    if offset_of!(MemorySegment, scratch_start) % 4 != 0 {
        panic!("unaligned")
    }
    if offset_of!(MemorySegment, stack_top) % 4 != 0 {
        panic!("unaligned")
    }
};

#[cfg(target_arch = "riscv64")]
macro_rules! load {
    ($ptr:ident [$($expr:expr)?] => $reg:ident) => {
        stringify!(ld $reg, $((8*($expr)))?($ptr))
    };
}
#[cfg(target_arch = "riscv32")]
macro_rules! load {
    ($ptr:ident [$($expr:expr)?] => $reg:ident) => {
        stringify!(lw $reg, $((4*($expr)))?($ptr))
    };
}

impl MemorySegment {
    pub fn memory_range(&self) -> Range<usize> {
        self.memory_start..self.memory_end
    }
    pub const fn new() -> Self {
        Self {
            scratch_start: 0,
            stack_top: 0,
            enable_mask: HartEnableMask::new(),
            total_hart_count: 0,
            cbom: 0,
            heap_end: 0,
            heap_start: 0,
            memory_start: 0,
            memory_end: 0,
        }
    }
    pub fn memory_end(&self) -> usize {
        self.heap_end
    }
    pub fn heap_start(&self) -> usize {
        self.heap_start
    }
    pub fn cbom(&self) -> usize {
        self.cbom as usize
    }
    #[unsafe(naked)]
    pub unsafe extern "C" fn locate_stack_scratch(
        mem_segment: *const Self,
        hartid: usize,
    ) -> HartStackScratch {
        naked_asm!(
            ".option push",
            ".option arch,+a",
            ".option arch,+m",
            "li t0,{max_hart_id}",
            "bgeu a1,t0,114f",
            // t0 = a1/8
            "srli t0,a1,3",
            // t1 = a1%8
            "andi t1,a1,7",

            "add t2,a0,t0",
            "lbu t2,{off_mask}(t2)",
            "srl t2,t2,t1",
            "andi t2,t2,1",
            "beqz t2,114f",
            "lla t0,{index}",
            "li t1,1",
            "amoadd.w t0,t1,(t0)",
            "lw t1,{off_active_harts}(a0)",
            "bgeu t0,t1,114f",

            load!(a0[{off_scratch}] => t1),
            load!(a0[{off_stack}] => t2),

            // t0:index t1:scratch start, t2:stack top

            "li t3,-{stack_size}",
            "mul a0,t0,t3",
            "add a0,a0,t2",
            "li t3,{scratch_size}",
            "mul a1,t0,t3",
            "add a1,a1,t1",
            "ret",
            "114:",
            "wfi",
            "j 114b",
            ".option pop",
            off_stack = const OFF_STACK,
            off_scratch = const OFF_SCRATCH,
            index = sym INDEX,
            off_active_harts = const offset_of!(MemorySegment,total_hart_count),
            off_mask = const offset_of!(MemorySegment,enable_mask),
            max_hart_id = const MAX_HARTS * 8,
            stack_size = const STACK_SIZE,
            scratch_size = const size_of::<SbiScratch>(),
        )
    }
    pub unsafe fn init_memory_region(fdt: usize) -> Self {
        let fdt = unsafe { DeviceTreeBlob::new(fdt).unwrap() };

        let mut cbom = 0;
        let mut enable_mask = HartEnableMask::new();
        let mut max_harts = 0;
        for cpu in fdt.cpus() {
            if cpu.enabled {
                cbom = cbom.max(cpu.cbom.unwrap_or_default());
                enable_mask.enable(cpu.cpuid);
                max_harts += 1;
            }
        }
        let page_size = PLATFORM_CONFIG.page_size;
        let fw_end: usize;

        unsafe {
            asm!("la {},_fw_end",out(reg) fw_end);
        }
        let (scratch_start, stack_end, heap_start, heap_end) = {
            let max_harts = max_harts as usize;
            let scratch_start =
                (fw_end + align_of::<SbiScratch>() - 1) & !(align_of::<SbiScratch>() - 1);
            let scratch_end = scratch_start + max_harts * size_of::<SbiScratch>();
            let stack_start = (scratch_end + 15) & !15;
            let stack_size = (PLATFORM_CONFIG.stack_size + 15) & !15;
            let stack_end = stack_start + max_harts * stack_size;
            let heap_size = PLATFORM_CONFIG
                .heap_size
                .unwrap_or_else(|| (8 + max_harts) * page_size + 128 * max_harts * max_harts);
            let heap_size = (heap_size + page_size - 1) & !(page_size - 1);
            let heap_start = (stack_end + page_size - 1) & !(page_size - 1);
            let heap_end = heap_start + heap_size;
            (scratch_start, stack_end, heap_start, heap_end)
        };
        let (memory_start, memory_end) = fdt.memory();
        MemorySegment {
            scratch_start,
            stack_top: stack_end,
            enable_mask,
            total_hart_count: max_harts,
            cbom,
            heap_start,
            heap_end,
            memory_start,
            memory_end,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HartStackScratch {
    /// a0
    pub stack_top: usize,
    /// a1
    pub scratch: *mut SbiScratch,
}

static mut INDEX: u32 = 0;
const MAX_HARTS: usize = (PLATFORM_CONFIG.max_harts + 7) / 8;
const STACK_SIZE: usize = PLATFORM_CONFIG.stack_size;
#[repr(transparent)]
struct HartEnableMask {
    mask: [u8; MAX_HARTS],
}

impl HartEnableMask {
    fn enable(&mut self, cpuid: u32) {
        let byte_offset = (cpuid / 8) as usize;
        if byte_offset >= self.mask.len() {
            return;
        }
        let bit_num = (cpuid & 0b111) as usize;
        self.mask[byte_offset] |= 1 << bit_num;
    }
    const fn new() -> Self {
        Self {
            mask: [0; MAX_HARTS],
        }
    }
}
