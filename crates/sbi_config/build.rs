use std::env::{var,current_dir};
use std::path::PathBuf;
use toml::{from_str,Table};
use std::fs::{read_to_string,write};
fn main() -> Result<(),Box<dyn std::error::Error>>{
    let out_dir = PathBuf::from(var("OUT_DIR")?);
    let config_dir = current_dir()?.join("../../config");
    println!("cargo::rerun-if-changed={}",config_dir.display());
    let sbi_config:Table = from_str(&read_to_string(config_dir.join("config.toml"))?)?;
    
    let firmware = sbi_config.get("firmware");

    let jump_address = firmware
        .and_then(|v| v.get("jump-address"))
        .and_then(|i| i.as_integer())
        .map(|i| i as usize);

    write(out_dir.join("fw_config.in"),
    format!("FwConfig {{ jump_address:{:?} }}",
        jump_address
    ))?;

    let link_start = sbi_config.get("link")
        .and_then(|v| v.get("link-address"))
        .and_then(|i| i.as_integer())
        .map(|i| i as usize)
        .unwrap_or(0x8000_0000);
    let payload_offset = sbi_config.get("link")
        .and_then(|v| v.get("payload-offset"))
        .and_then(|i| i.as_integer())
        .map(|i| i as usize);
    
    let payload_align = sbi_config.get("link")
        .and_then(|v| v.get("payload-align"))
        .and_then(|i| i.as_integer())
        .map(|i| i as usize);
    write(out_dir.join("link_config.in"),format!(
        "LinkConfig {{ start_address:{link_start},payload_offset:{payload_offset:?}, payload_align:{payload_align:?} }}"
    ))?;

    let platform = sbi_config.get("platform").expect("undefined [platform] entry");
    let stack_size = platform
        .get("stack-size")
        .expect("stack-size is not defined in [platform]")
        .as_integer().unwrap() as usize;
    let heap_size = platform
        .get("heap-size")
        .and_then(|i| i.as_integer())
        .map(|i| i as usize);
    let max_harts = platform.get("max-harts")
        .and_then(|v| v.as_integer())
        .map(|i| i as usize)
        .unwrap_or(512);
    let page_size = 4096;
    write(out_dir.join("platform_config.in"),format!(
        "PlatformConfig {{ stack_size:{stack_size}, heap_size:{heap_size:?}, max_harts:{max_harts}, page_size:{page_size} }}"
    ))?;
    Ok(())
}