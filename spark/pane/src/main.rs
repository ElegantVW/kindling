//! Tiny pane: the first TUI. Flush owns the left glass. House calls only.
//! No slice panic, no libcall memset — those pulled a GOT at address 8 (bunny).
#![no_std]
#![no_main]
#![allow(suspicious_runtime_symbol_definitions)]

use core::ffi::c_void;

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

fn paint(text: *const u8, n: usize) {
    let mut frame = core::mem::MaybeUninit::<[u8; 280]>::uninit();
    let p = frame.as_mut_ptr() as *mut u8;
    unsafe {
        p.write(0x0C);
        let mut i = 1usize;
        let mut k = 0usize;
        while k < OPEN.len() && i < 280 {
            p.add(i).write(*OPEN.as_ptr().add(k));
            i += 1;
            k += 1;
        }
        k = 0;
        while k < n && i < 280 {
            core::ptr::write_volatile(p.add(i), *text.add(k));
            i += 1;
            k += 1;
        }
        let _ = knock(FLUSH, p as u64, i as u64, 0);
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
        let mut text = core::mem::MaybeUninit::<[u8; 256]>::uninit();
        let tp = text.as_mut_ptr() as *mut u8;
        let mut n = 0usize;
        let mut key = 0u8;
        loop {
            paint(tp, n);
            if (knock(READ, 0, &mut key as *mut u8 as u64, 1) as i64) <= 0 {
                smoor(1);
            }
            let b = key;
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
                if n < 256 {
                    tp.add(n).write(if b == b'\r' { b'\n' } else { b });
                    n += 1;
                }
            }
        }
        let _ = knock(WRITE, 1, SHUT.as_ptr() as u64, SHUT.len() as u64);
        smoor(0);
    }
}

/// LLVM may still emit `memset`/`memcpy` libcalls. Direct symbols, no GOT.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let p = s as *mut u8;
    let mut i = 0usize;
    while i < n {
        unsafe {
            p.add(i).write(c as u8);
        }
        i += 1;
    }
    s
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let d = dst as *mut u8;
    let s = src as *const u8;
    let mut i = 0usize;
    while i < n {
        unsafe {
            d.add(i).write(s.add(i).read());
        }
        i += 1;
    }
    dst
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    unsafe {
        if (dst as usize) < (src as usize) {
            memcpy(dst, src, n)
        } else {
            let d = dst as *mut u8;
            let s = src as *const u8;
            let mut i = n;
            while i > 0 {
                i -= 1;
                d.add(i).write(s.add(i).read());
            }
            dst
        }
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
