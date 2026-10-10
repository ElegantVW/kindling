//! Tiny Gleam spark: write a line, smoor. House calls only. Not Linux.
#![no_std]
#![no_main]

const WRITE: u64 = 2;
const EXIT: u64 = 1;
const LINE: &[u8] = b"the rust is lit\n";

#[unsafe(link_section = ".text.entry")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    unsafe {
        core::arch::asm!(
            "int $0xE0",
            in("rax") WRITE,
            in("rdi") 1u64,
            in("rsi") LINE.as_ptr(),
            in("rdx") LINE.len() as u64,
            out("rcx") _,
            out("r11") _,
        );
        core::arch::asm!(
            "int $0xE0",
            in("rax") EXIT,
            in("rdi") 0u64,
            options(noreturn),
        );
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
