use sbi_config::{LINK_CONFIG};
use std::env::var;
use std::fs::write;
use std::path::PathBuf;
fn main() {
    let link_start = LINK_CONFIG.start_address;
    let out_dir = PathBuf::from(var("OUT_DIR").unwrap());
    let linker = out_dir.join("link_script.x");
    if var("CARGO_FEATURE_DYNAMIC").is_ok() {
        write(&linker,format!(
"OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{{
    . = {link_start};
{LINKER_SCRIPT_BASE}
	PROVIDE(_fw_reloc_end = .);
}}"
        )).unwrap()
    }else if var("CARGO_FEATURE_PAYLOAD").is_ok() {
        let payload_addr;
        if let Some(offset) = LINK_CONFIG.payload_offset {
            payload_addr = format!(". = {}",link_start + offset);
        }else{
            payload_addr = format!(". = ALIGN({})",LINK_CONFIG.payload_align.unwrap());
        }
        write(&linker,format!(
"OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{{
    . = {link_start};

{LINKER_SCRIPT_BASE}

    {payload_addr};
	.payload :
	{{
		PROVIDE(_payload_start = .);
		*(.payload)
		. = ALIGN(8);
		PROVIDE(_payload_end = .);
    }}

	PROVIDE(_fw_reloc_end = .);
}}"
        )).unwrap()
    }else{
        write(&linker,format!(
"OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{{
    . = {link_start};
{LINKER_SCRIPT_BASE}
	PROVIDE(_fw_reloc_end = .);
}}"
        )).unwrap()
    }
    println!("cargo:rustc-link-arg=-T{}",linker.display());
    println!("cargo:rustc-link-search={}", out_dir.display());
}

const LINKER_SCRIPT_BASE:&str = 
"/*
 * copied from opensbi/firmware/fw_base.ldS
 * SPDX-License-Identifier: BSD-2-Clause
 *
 * Copyright (c) 2019 Western Digital Corporation or its affiliates.
 *
 * Authors:
 *   Anup Patel <anup.patel@wdc.com>
 */

	/* Don't add any section between FW_TEXT_START and _fw_start */
	PROVIDE(_fw_start = .);

	. = ALIGN(0x1000); /* Need this to create proper sections */

	/* Beginning of the code section */

	.text :
 	{
		PROVIDE(_text_start = .);
		*(.entry)
		*(.text)
		*(.text.*)
		. = ALIGN(8);
		PROVIDE(_text_end = .);
	}

	/* End of the code sections */

	. = ALIGN(0x1000); /* Ensure next section is page aligned */

	/* Beginning of the read-only data sections */

	PROVIDE(_rodata_start = .);

	.rodata :
	{
		*(.rodata .rodata.*)
		. = ALIGN(8);
	}

	.dynsym :
	{
		*(.dynsym)
	}

	. = ALIGN(0x1000); /* Ensure next section is page aligned */

	.rela.dyn : {
		PROVIDE(__rela_dyn_start = .);
		*(.rela*)
		PROVIDE(__rela_dyn_end = .);
	}

	PROVIDE(_rodata_end = .);

	/* End of the read-only data sections */

	/*
	 * PMP regions must be to be power-of-2. RX/RW will have separate
	 * regions, so ensure that the split is power-of-2.
	 */
	. = ALIGN(1 << LOG2CEIL((SIZEOF(.rodata) + SIZEOF(.text)
				+ SIZEOF(.dynsym) + SIZEOF(.rela.dyn))));

	PROVIDE(_fw_rw_start = .);

	/* Beginning of the read-write data sections */

	.data :
	{
		PROVIDE(_data_start = .);

		*(.sdata)
		*(.sdata.*)
		*(.data)
		*(.data.*)
		*(.readmostly.data)
		*(*.data)
		. = ALIGN(8);

		PROVIDE(_data_end = .);
	}

	. = ALIGN(0x1000); /* Ensure next section is page aligned */

	.bss :
	{
		PROVIDE(_bss_start = .);
		*(.sbss)
		*(.sbss.*)
		*(.bss)
		*(.bss.*)
		. = ALIGN(8);
		PROVIDE(_bss_end = .);
	}

	/* End of the read-write data sections */

	. = ALIGN(0x1000); /* Need this to create proper sections */

	PROVIDE(_fw_end = .);";

