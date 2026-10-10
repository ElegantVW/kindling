//! Tiny pane: the first TUI. Flush owns the left glass. House calls only.
#![no_std]
#![no_main]

const WRITE: u64 = 2;
const EXIT: u64 = 1;
const FLUSH: u64 = 7;
const READ: u64 = 10;
const CAST: u64 = 15;
const OPEN: &[u8] = b"the pane is open\n";
const SHUT: &[u8] = b"the pane is shut\n";

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

fn paint(text: &[u8], n: usize) {
    let mut frame = [0u8; 280];
    frame[0] = 0x0C;
    let mut i = 1usize;
    for &b in OPEN {
        if i >= frame.len() {
            break;
        }
        frame[i] = b;
        i += 1;
    }
    let mut k = 0usize;
    while k < n && i < frame.len() {
        frame[i] = text[k];
        i += 1;
        k += 1;
    }
    unsafe {
        let _ = knock(FLUSH, frame.as_ptr() as u64, i as u64, 0);
    }
}

#[unsafe(link_section = ".text.entry")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    unsafe {
        let _ = knock(WRITE, 1, OPEN.as_ptr() as u64, OPEN.len() as u64);
        if (knock(FLUSH, 0, 0, 0) as i64) < 0 {
            smoor(1);
        }
        let mut text = [0u8; 256];
        let mut n = 0usize;
        let mut key = [0u8; 1];
        loop {
            paint(&text, n);
            if (knock(READ, 0, key.as_mut_ptr() as u64, 1) as i64) <= 0 {
                smoor(1);
            }
            let b = key[0];
            if b == b'q' {
                break;
            }
            if b == b'r' {
                let _ = knock(CAST, 0, 0, 0);
                smoor(1);
            }
            if b == 8 || b == 0x7F {
                if n > 0 {
                    n -= 1;
                }
                continue;
            }
            if b == b'\n' || b == b'\r' || (0x20..0x7F).contains(&b) {
                if n < text.len() {
                    text[n] = if b == b'\r' { b'\n' } else { b };
                    n += 1;
                }
            }
        }
        let _ = knock(WRITE, 1, SHUT.as_ptr() as u64, SHUT.len() as u64);
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
