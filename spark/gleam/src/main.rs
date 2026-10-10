//! Tiny Gleam spark: write, flush the glass, smoor. House calls only. Not Linux.
#![no_std]
#![no_main]

const WRITE: u64 = 2;
const EXIT: u64 = 1;
const FLUSH: u64 = 7;
const LINE: &[u8] = b"the rust is lit\n";
const GLASS: &[u8] = b"the glass is ours\n";

unsafe fn knock(n: u64, a0: u64, a1: u64, a2: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "int $0xE0",
            inout("rax") n => ret,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            out("rcx") _,
            out("r11") _,
        );
    }
    ret
}

fn smoor(code: u64) -> ! {
    unsafe {
        core::arch::asm!(
            "int $0xE0",
            in("rax") EXIT,
            in("rdi") code,
            options(noreturn),
        );
    }
}

#[unsafe(link_section = ".text.entry")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    unsafe {
        let _ = knock(WRITE, 1, LINE.as_ptr() as u64, LINE.len() as u64);
        if (knock(FLUSH, 0, 0, 0) as i64) < 0 {
            smoor(1);
        }
        let n = GLASS.len() as u64;
        if knock(FLUSH, GLASS.as_ptr() as u64, n, 0) != n {
            smoor(1);
        }
        let _ = knock(WRITE, 1, GLASS.as_ptr() as u64, n);
        smoor(0);
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
