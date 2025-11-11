
#[cfg(target_arch = "riscv64")]
const _:() = if size_of::<usize>() != 8 {panic!()};
#[cfg(target_arch = "riscv32")]
const _:() = if size_of::<usize>() != 4 {panic!()};

#[cfg(target_arch = "riscv64")]
macro_rules! save {
    ($reg:ident => $ptr:ident [$($expr:expr),*]) => {
        ::core::concat!($(::core::stringify!(
            sd $reg, (($expr)*8)($ptr)
        ),"\n"),*)
    };
    ($reg:ident => $ptr:ident [$(b:$expr:expr),*]) => {
        ::core::concat!($(::core::stringify!(
            sd $reg, ($expr)($ptr)
        ),"\n"),*)
    };
}

#[cfg(target_arch = "riscv32")]
macro_rules! save {
    ($reg:ident => $ptr:ident [$($expr:expr),*]) => {
        ::core::concat!($(::core::stringify!(
            sw $reg, (($expr)*4)($ptr)
        ),"\n"),*)
    };
    ($reg:ident => $ptr:ident [$(b:$expr:expr),*]) => {
        ::core::concat!($(::core::stringify!(
            sw $reg, ($expr)($ptr)
        ),"\n"),*)
    };
}

#[cfg(target_arch = "riscv64")]
macro_rules! load {
    ($reg:ident <= $ptr:ident [$($expr:expr),*]) => {
        ::core::concat!($(::core::stringify!(
            ld $reg, (($expr)*8)($ptr)
        ),"\n"),*)
    };
    ($reg:ident <= $ptr:ident [$(b:$expr:expr),*]) => {
        ::core::concat!($(::core::stringify!(
            ld $reg, ($expr)($ptr)
        ),"\n"),*)
    };
}

#[cfg(target_arch = "riscv32")]
macro_rules! load {
    ($reg:ident <= $ptr:ident [$($expr:expr),*]) => {
        ::core::concat!($(::core::stringify!(
            lw $reg, (($expr)*4)($ptr)
        ),"\n"),*)
    };
    ($reg:ident <= $ptr:ident [$(b:$expr:expr),*]) => {
        ::core::concat!($(::core::stringify!(
            lw $reg, ($expr)($ptr)
        ),"\n"),*)
    };
}