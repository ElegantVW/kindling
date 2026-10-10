//! Cairn — leaves + sparks at 0x100000, laid by the loader (docs/CAIRN.md).
//! The kernel re-verifies everything: KMAP scratch magic, cairn checksum,
//! header, walk bounds, per-leaf xor. Absent or corrupt is refusal, never
//! partial bytes. BIOS loader lays the slot; EFI plants `EFI/BOOT/CAIRN`
//! the same way. Absent is `-ENODEV`.

use crate::start::{hcf, serial_print, serial_u64};

pub const GLEAN: u64 = 8;
pub const STOW: u64 = 9;

pub const CAIRN_RAM: u64 = 0x100000;
const KMAP_RAM: u64 = 0x8400;
const KMAP_MAGIC: u32 = 0x50414D4B;
const CAIR_MAGIC: u32 = 0x52494143;
const MAX_SECTORS: u32 = 1024;
const MAX_LEAVES: u32 = 256;
const MAX_DATA: u64 = 1 << 20;

const EPERM: u64 = 1;
const ENOENT: u64 = 2;
const EIO: u64 = 5;
const EAGAIN: u64 = 11;
const ENODEV: u64 = 19;

fn err(n: u64) -> u64 {
    0u64.wrapping_sub(n)
}

fn align_up(x: u64, a: u64) -> u64 {
    (x + a - 1) & !(a - 1)
}

struct Cairn {
    base: u64,
    len: u64,
}

/// Once verified, by base+len. Sparks share the cairn's roof and may spit on
/// their own floorboards (tale's buf lives inside its own record) — so the
/// full-image xor runs ONCE here; the per-leaf xor in `find` still guards
/// every read after that.
static mut SEALED_BASE: u64 = 0;
static mut SEALED_LEN: u64 = 0;
static mut SEALED: bool = false;

/// Plant a cairn already sitting at [`CAIRN_RAM`]. EFI copies `EFI/BOOT/CAIRN`
/// there before ExitBootServices, then calls this so `open` sees a KMAP.
#[allow(dead_code)]
pub fn offer(len: u64) {
    if len == 0 || len % 512 != 0 {
        return;
    }
    let csec = (len / 512) as u32;
    if csec == 0 || csec > MAX_SECTORS {
        return;
    }
    let mut x = 0u32;
    let mut off = 0u64;
    unsafe {
        while off < len {
            x ^= ((CAIRN_RAM + off) as *const u32).read_volatile();
            off += 4;
        }
        let clba = 1u32;
        let csum = clba ^ csec ^ x ^ CAIR_MAGIC;
        let k = KMAP_RAM as *mut u32;
        k.write_volatile(KMAP_MAGIC);
        k.add(1).write_volatile(0);
        k.add(2).write_volatile(0);
        k.add(3).write_volatile(0);
        k.add(4).write_volatile(0);
        k.add(5).write_volatile(clba);
        k.add(6).write_volatile(csec);
        k.add(7).write_volatile(csum);
        core::ptr::addr_of_mut!(SEALED).write(false);
    }
}

/// Locate + verify the cairn via the KMAP scratch the loader left at 0x8400.
fn open() -> Option<Cairn> {
    unsafe {
        if core::ptr::addr_of!(SEALED).read() {
            let base = core::ptr::addr_of!(SEALED_BASE).read();
            let len = core::ptr::addr_of!(SEALED_LEN).read();
            if (base as *const u32).read_volatile() != CAIR_MAGIC {
                return None;
            }
            return Some(Cairn { base, len });
        }
    }
    let c = open_verify()?;
    unsafe {
        core::ptr::addr_of_mut!(SEALED_BASE).write(c.base);
        core::ptr::addr_of_mut!(SEALED_LEN).write(c.len);
        core::ptr::addr_of_mut!(SEALED).write(true);
    }
    Some(c)
}

/// Full verification: KMAP scratch magic, cairn checksum, header shape.
fn open_verify() -> Option<Cairn> {
    unsafe {
        let k = KMAP_RAM as *const u32;
        if k.read_volatile() != KMAP_MAGIC {
            return None;
        }
        let clba = k.add(5).read_volatile();
        let csec = k.add(6).read_volatile();
        let csum = k.add(7).read_volatile();
        if clba == 0 || csec == 0 || csec > MAX_SECTORS {
            return None;
        }
        let len = (csec as u64) * 512;
        let base = CAIRN_RAM;
        let mut x = 0u32;
        let mut off = 0u64;
        while off < len {
            x ^= ((base + off) as *const u32).read_volatile();
            off += 4;
        }
        if (clba ^ csec ^ x ^ CAIR_MAGIC) != csum {
            return None;
        }
        let h = base as *const u32;
        if h.read_volatile() != CAIR_MAGIC {
            return None;
        }
        if h.add(1).read_volatile() != 1 {
            return None;
        }
        if h.add(2).read_volatile() > MAX_LEAVES {
            return None;
        }
        Some(Cairn { base, len })
    }
}

fn read_u32_at(base: u64, off: u64) -> u32 {
    unsafe { ((base + off) as *const u32).read_volatile() }
}

fn read_u8_at(base: u64, off: u64) -> u8 {
    unsafe { ((base + off) as *const u8).read_volatile() }
}

/// Find a record by name + kind. Ok(addr, len) | Err(ENOENT) | Err(EIO).
fn find(c: &Cairn, name: &[u8], kind_want: u32) -> Result<(u64, u64), u64> {
    let n = read_u32_at(c.base, 8);
    let mut off = 12u64;
    for _ in 0..n {
        if off + 8 > c.len {
            return Err(EIO);
        }
        let kind = read_u32_at(c.base, off);
        let nl = read_u8_at(c.base, off + 4) as u64;
        if nl == 0 || nl > 64 {
            return Err(EIO);
        }
        let de = align_up(off + 5 + nl, 4);
        if de + 4 > c.len {
            return Err(EIO);
        }
        let dl = read_u32_at(c.base, de) as u64;
        if dl > MAX_DATA {
            return Err(EIO);
        }
        let da = de + 4;
        let xe = align_up(da + dl, 4);
        if xe + 4 > c.len {
            return Err(EIO);
        }
        if kind == kind_want && nl == name.len() as u64 {
            let mut same = true;
            for i in 0..nl {
                if read_u8_at(c.base, off + 5 + i) != name[i as usize] {
                    same = false;
                    break;
                }
            }
            if same {
                let mut x = 0u32;
                let mut q = 0u64;
                while q < align_up(dl, 4) {
                    x ^= read_u32_at(c.base, da + q);
                    q += 4;
                }
                if x != read_u32_at(c.base, xe) {
                    return Err(EIO);
                }
                return Ok((c.base + da, dl));
            }
        }
        off = xe + 4;
    }
    Err(ENOENT)
}

/// Read a NUL-terminated leaf name (≤64, plain ascii, no `/`).
/// Err(EPERM) on any cheek: null, empty, overlong, unterminated, slashy.
fn take_name(name_ptr: u64) -> Result<([u8; 64], usize), u64> {
    if name_ptr == 0 {
        return Err(EPERM);
    }
    let mut name = [0u8; 64];
    let mut nl = 0usize;
    loop {
        if nl >= 65 {
            return Err(EPERM);
        }
        let b = unsafe { ((name_ptr + nl as u64) as *const u8).read_volatile() };
        if b == 0 {
            break;
        }
        if nl >= 64 || b == b'/' || !(0x20..=0x7E).contains(&b) {
            return Err(EPERM);
        }
        name[nl] = b;
        nl += 1;
    }
    if nl == 0 {
        return Err(EPERM);
    }
    Ok((name, nl))
}

/// Gather a leaf's bytes: `rdi` = name, `rsi` = buf, `rdx` = len (cap 1 MiB).
/// Cairn first. Book leaves (`hands` / `twin` / `hand`) skip the cairn when a
/// KINDLOG slice is named — the paved pack still carries wax copies, and those
/// must not hide the house. Other names, if missing from the cairn, glean the
/// FAT volume's root. Returns bytes
/// copied (a short read when the leaf is longer is honest, not an error) or
/// `-errno`. v0 trusts mapped RAM for pointers; spawn's realm still lets the
/// kernel copy (CPL0 ignores U/S). User pointer checks wait.
pub fn glean(name_ptr: u64, buf: u64, len: u64) -> u64 {
    if buf == 0 || len == 0 || len > MAX_DATA {
        return err(EPERM);
    }
    let (name, nl) = match take_name(name_ptr) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let nm = &name[..nl];
    if crate::kindlog::is_book_leaf(nm) && crate::kindlog::named() {
        return glean_volume(nm, buf, len, open().is_some());
    }
    match open() {
        Some(c) => match find(&c, nm, 0) {
            Ok((da, dl)) => copy_leaf(da, dl, buf, len),
            Err(e) if e == EIO => err(EIO),
            Err(_) => glean_volume(nm, buf, len, true),
        },
        None => glean_volume(nm, buf, len, false),
    }
}

fn copy_leaf(da: u64, dl: u64, buf: u64, len: u64) -> u64 {
    let n = len.min(dl);
    let mut i = 0u64;
    while i < n {
        let b = unsafe { ((da + i) as *const u8).read_volatile() };
        unsafe { ((buf + i) as *mut u8).write_volatile(b) };
        i += 1;
    }
    n
}

fn glean_volume(name: &[u8], buf: u64, len: u64, had_cairn: bool) -> u64 {
    if crate::kindlog::is_book_leaf(name) {
        if crate::kindlog::live() {
            return crate::kindlog::glean(name, buf, len);
        }
        if crate::kindlog::named() {
            return err(ENOENT);
        }
    }
    if crate::usb::fat_live() {
        crate::usb::glean_fat(name, buf, len)
    } else if had_cairn {
        err(ENOENT)
    } else {
        err(ENODEV)
    }
}

static mut SECTOR_SCRATCH: [u16; 256] = [0; 256]; // 512 B, 2-aligned for the word loop

fn sector_eq(ram: u64, disk: u64) -> bool {
    let mut i = 0u64;
    while i < 512 {
        unsafe {
            if ((ram + i) as *const u8).read_volatile() != ((disk + i) as *const u8).read_volatile()
            {
                return false;
            }
        }
        i += 1;
    }
    true
}

fn cairn_xor(base: u64, len: u64) -> u32 {
    let mut x = 0u32;
    let mut off = 0u64;
    while off < len {
        x ^= unsafe { ((base + off) as *const u32).read_volatile() };
        off += 4;
    }
    x
}

/// Lay bytes down. Cairn first (kind 0): re-ink a leaf with the same
/// measure (`rdx` must equal the leaf's `datalen`). Book leaves skip the
/// cairn when a KINDLOG slice is named, then KINDLOG when live. A named
/// slice that is not live paints `the log is dark` for those names and
/// does not fall through to FAT.
/// Other names, if missing from the cairn, stow the
/// FAT volume's root — WRITE(10) each sector, wait, READ(10) compare, KINDLING
/// `85C7-AA81` only. FAT32 creates a missing or empty root file of `len`,
/// and grows or shrinks a present file to `len`. FAT16 re-inks same measure.
/// Cairn ink still goes through ATA, re-reads, and rewrites LBA0. Returns
/// bytes laid or `-errno` (`EPERM` args/shape/wrong volume/wrong measure,
/// `ENOENT` FAT16 missing, `ENODEV` no cairn and no volume, `EIO` disk
/// error or verify mismatch). Ink is for leaves — sparks refuse.
pub fn stow(name_ptr: u64, buf: u64, len: u64) -> u64 {
    if buf == 0 || len == 0 || len > MAX_DATA {
        return err(EPERM);
    }
    let (name, nl) = match take_name(name_ptr) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let nm = &name[..nl];
    if crate::kindlog::is_book_leaf(nm) && crate::kindlog::named() {
        return stow_volume(nm, buf, len, open().is_some());
    }
    match open() {
        Some(c) => match find(&c, nm, 0) {
            Ok((da, dl)) => {
                if len != dl {
                    return err(EPERM);
                }
                stow_cairn(&c, da, dl, buf)
            }
            Err(e) if e == EIO => err(EIO),
            Err(_) => stow_volume(nm, buf, len, true),
        },
        None => stow_volume(nm, buf, len, false),
    }
}

fn stow_volume(name: &[u8], buf: u64, len: u64, had_cairn: bool) -> u64 {
    if crate::kindlog::is_book_leaf(name) {
        if crate::kindlog::live() {
            return crate::kindlog::stow(name, buf, len);
        }
        if crate::kindlog::named() {
            crate::kindlog::speak_dark();
            return err(ENODEV);
        }
    }
    if !crate::usb::fat_live() {
        return if had_cairn { err(ENOENT) } else { err(ENODEV) };
    }
    crate::usb::stow_fat(name, buf, len)
}

fn stow_cairn(c: &Cairn, da: u64, dl: u64, buf: u64) -> u64 {
    // KMAP scratch fields the loader left: cairn_lba + sectors at +20/+24.
    let (clba, csec) = unsafe {
        let k = KMAP_RAM as *const u32;
        (k.add(5).read_volatile(), k.add(6).read_volatile())
    };
    // Re-ink the RAM copy first (volatile both ways — DMA-adjacent).
    let mut i = 0u64;
    while i < dl {
        let b = unsafe { ((buf + i) as *const u8).read_volatile() };
        unsafe { ((da + i) as *mut u8).write_volatile(b) };
        i += 1;
    }
    // Leaf xor field sits at the 4-aligned tail of the data.
    let xe = align_up(da - c.base + dl, 4);
    let mut x = 0u32;
    let mut q = 0u64;
    while q < align_up(dl, 4) {
        x ^= unsafe { ((da + q) as *const u32).read_volatile() };
        q += 4;
    }
    unsafe { ((c.base + xe) as *mut u32).write_volatile(x) };
    // Fresh cairn sum over the whole image, into the RAM KMAP scratch.
    let sum = clba ^ csec ^ cairn_xor(c.base, c.len) ^ CAIR_MAGIC;
    unsafe { ((KMAP_RAM + 28) as *mut u32).write_volatile(sum) };
    // Sectors touched: leaf bytes through the xor field (offsets, not addrs).
    let first = (da - c.base) / 512;
    let last = (xe + 4 - 1) / 512;
    let mut idx = first;
    while idx <= last {
        let ram = c.base + idx * 512;
        if !crate::ata::write_sectors(clba + idx as u32, 1, ram as *mut u8) {
            return err(EIO);
        }
        let scratch = core::ptr::addr_of_mut!(SECTOR_SCRATCH) as *mut u8;
        if !crate::ata::read_sectors(clba + idx as u32, 1, scratch) {
            return err(EIO);
        }
        if !sector_eq(ram, scratch as u64) {
            return err(EIO);
        }
        idx += 1;
    }
    // LBA0 carries the new sum — without it the next boot distrusts the cairn.
    if !crate::ata::write_sectors(0, 1, KMAP_RAM as *mut u8) {
        return err(EIO);
    }
    let scratch = core::ptr::addr_of_mut!(SECTOR_SCRATCH) as *mut u8;
    if !crate::ata::read_sectors(0, 1, scratch) {
        return err(EIO);
    }
    if !sector_eq(KMAP_RAM, scratch as u64) {
        return err(EIO);
    }
    dl
}

fn spawn_fail(n: u64) -> ! {
    serial_print("kindling: spawn FAIL ");
    serial_u64(n);
    serial_print("\n");
    hcf();
}

const SPARK_RAW_MAX: u64 = 65536;
const SPARK_ELF_MAX: u64 = 256 * 1024;
const ELF_MAGIC0: u8 = 0x7f;
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 62;
const PT_LOAD: u32 = 1;
const PT_INTERP: u32 = 3;
const PT_DYNAMIC: u32 = 2;
const PH_MAX: usize = 16;
const EHDR: u64 = 64;
const PHDR: u64 = 56;
/// Gleam ELF floor: 1.25 GiB. EFI well caps at 1 GiB; QEMU GOP sits at
/// 2 GiB. 1 GiB collided with iron GOP (user walk had no U/S). 4 GiB
/// does not fit the small `core` (`R_X86_64_32S`).
const USER_VA_LO: u64 = 0x5000_0000;
const USER_VA_HI: u64 = 0x6000_0000;

fn b8(src: u64, off: u64) -> u8 {
    unsafe { ((src + off) as *const u8).read_volatile() }
}

fn b16(src: u64, off: u64) -> u16 {
    u16::from_le_bytes([b8(src, off), b8(src, off + 1)])
}

fn b32(src: u64, off: u64) -> u32 {
    u32::from_le_bytes([
        b8(src, off),
        b8(src, off + 1),
        b8(src, off + 2),
        b8(src, off + 3),
    ])
}

fn b64(src: u64, off: u64) -> u64 {
    u64::from_le_bytes([
        b8(src, off),
        b8(src, off + 1),
        b8(src, off + 2),
        b8(src, off + 3),
        b8(src, off + 4),
        b8(src, off + 5),
        b8(src, off + 6),
        b8(src, off + 7),
    ])
}

fn is_elf(src: u64, len: u64) -> bool {
    len >= 4
        && b8(src, 0) == ELF_MAGIC0
        && b8(src, 1) == b'E'
        && b8(src, 2) == b'L'
        && b8(src, 3) == b'F'
}

#[derive(Clone, Copy)]
struct PhLoad {
    vaddr: u64,
    offset: u64,
    filesz: u64,
    memsz: u64,
}

fn load_raw(src: u64, len: u64) -> Result<crate::mm::SparkRealm, u64> {
    if len == 0 || len > SPARK_RAW_MAX {
        return Err(EPERM);
    }
    let Some(dst) = copy_out(src, len) else {
        spawn_fail(4);
    };
    Ok(crate::mm::place_spark(dst, len))
}

fn load_elf(src: u64, len: u64) -> Result<crate::mm::SparkRealm, u64> {
    if !(EHDR..=SPARK_ELF_MAX).contains(&len) {
        return Err(EPERM);
    }
    if b8(src, 4) != ELFCLASS64 || b8(src, 5) != ELFDATA2LSB || b8(src, 6) != 1 {
        return Err(EPERM);
    }
    if b16(src, 16) != ET_EXEC || b16(src, 18) != EM_X86_64 || b32(src, 20) != 1 {
        return Err(EPERM);
    }
    if b16(src, 52) != EHDR as u16 || b16(src, 54) != PHDR as u16 {
        return Err(EPERM);
    }
    let entry = b64(src, 24);
    let phoff = b64(src, 32);
    let phnum = b16(src, 56) as u64;
    if phnum == 0 || phnum > PH_MAX as u64 {
        return Err(EPERM);
    }
    let ph_bytes = phnum.saturating_mul(PHDR);
    let Some(ph_end) = phoff.checked_add(ph_bytes) else {
        return Err(EPERM);
    };
    if ph_end > len {
        return Err(EPERM);
    }
    let mut loads = [PhLoad {
        vaddr: 0,
        offset: 0,
        filesz: 0,
        memsz: 0,
    }; PH_MAX];
    let mut nload = 0usize;
    let mut i = 0u64;
    while i < phnum {
        let o = phoff + i * PHDR;
        let p_type = b32(src, o);
        if p_type == PT_INTERP || p_type == PT_DYNAMIC {
            return Err(EPERM);
        }
        if p_type == PT_LOAD {
            if nload >= PH_MAX {
                return Err(EPERM);
            }
            let vaddr = b64(src, o + 16);
            let offset = b64(src, o + 8);
            let filesz = b64(src, o + 32);
            let memsz = b64(src, o + 40);
            if filesz > memsz {
                return Err(EPERM);
            }
            let Some(file_end) = offset.checked_add(filesz) else {
                return Err(EPERM);
            };
            if filesz != 0 && file_end > len {
                return Err(EPERM);
            }
            let Some(va_end) = vaddr.checked_add(memsz) else {
                return Err(EPERM);
            };
            if vaddr < USER_VA_LO || va_end > USER_VA_HI {
                return Err(EPERM);
            }
            loads[nload] = PhLoad {
                vaddr,
                offset,
                filesz,
                memsz,
            };
            nload += 1;
        }
        i += 1;
    }
    if nload == 0 {
        return Err(EPERM);
    }
    let mut min_va = u64::MAX;
    let mut max_va = 0u64;
    let mut n = 0usize;
    while n < nload {
        if loads[n].vaddr < min_va {
            min_va = loads[n].vaddr;
        }
        let end = loads[n].vaddr + loads[n].memsz;
        if end > max_va {
            max_va = end;
        }
        n += 1;
    }
    if min_va >= max_va {
        return Err(EPERM);
    }
    let va_page = min_va & !(4096 - 1);
    let end_page = align_up(max_va, 4096);
    let span = end_page - va_page;
    if span == 0 || span > SPARK_ELF_MAX {
        return Err(EPERM);
    }
    if entry < va_page || entry >= end_page {
        return Err(EPERM);
    }
    let Some(dst) = take_pages(span) else {
        spawn_fail(4);
    };
    unsafe {
        core::ptr::write_bytes(dst as *mut u8, 0, span as usize);
    }
    let mut n = 0usize;
    while n < nload {
        let rel = loads[n].vaddr - va_page;
        let mut k = 0u64;
        while k < loads[n].filesz {
            let b = b8(src, loads[n].offset + k);
            unsafe {
                ((dst + rel + k) as *mut u8).write_volatile(b);
            }
            k += 1;
        }
        n += 1;
    }
    Ok(crate::mm::place_spark_at(dst, span, va_page, entry))
}

fn load_spark(src: u64, len: u64) -> Result<crate::mm::SparkRealm, u64> {
    if is_elf(src, len) {
        load_elf(src, len)
    } else {
        load_raw(src, len)
    }
}

/// Contiguous pool run for a spark. None = scattered.
fn take_pages(len: u64) -> Option<u64> {
    if len == 0 {
        return None;
    }
    crate::mm::page_alloc_run(len.div_ceil(4096))
}

/// Copy a spark onto a contiguous pool run. None = scattered.
fn copy_out(src: u64, len: u64) -> Option<u64> {
    let dst = take_pages(len)?;
    let mut i = 0u64;
    while i < len {
        let b = unsafe { ((src + i) as *const u8).read_volatile() };
        unsafe { ((dst + i) as *mut u8).write_volatile(b) };
        i += 1;
    }
    Some(dst)
}

fn kindle_bytes(src: u64, len: u64) -> u64 {
    match load_spark(src, len) {
        Ok(realm) => {
            crate::house::kindle(realm);
            0
        }
        Err(e) => err(e),
    }
}

fn spawn_volume(name: &[u8], had_cairn: bool) -> u64 {
    if !crate::usb::fat_live() {
        return if had_cairn { err(ENOENT) } else { err(ENODEV) };
    }
    let n = crate::usb::fat_file_len(name);
    if (n as i64) < 0 {
        return n;
    }
    if n == 0 || n > SPARK_ELF_MAX {
        return err(EPERM);
    }
    let Some(tmp) = take_pages(n) else {
        spawn_fail(4);
    };
    let got = crate::usb::load_fat(name, tmp, n);
    if got != n {
        crate::mm::release_run(tmp, n);
        if (got as i64) < 0 {
            return got;
        }
        return err(EIO);
    }
    let realm = match load_spark(tmp, n) {
        Ok(r) => r,
        Err(e) => {
            crate::mm::release_run(tmp, n);
            return err(e);
        }
    };
    crate::mm::release_run(tmp, n);
    crate::house::kindle(realm);
    0
}

/// House call 5: named spark onto private pages, own cup, own CR3.
/// Cairn first (kind 1). If that name is missing (or there is no cairn) and
/// a FAT volume is live, the volume's root by the same Gleam name. Raw
/// `nasm -f bin` stays 64 KiB, identity-mapped. A Gleam ELF64 ET_EXEC (not
/// a Linux ABI) may be 256 KiB, mapped at its own VA. Kindles; when it
/// smoors, this call returns the last word in rax. One spark at a time —
/// a live Light is `-EAGAIN`.
pub fn spawn(name_ptr: u64) -> u64 {
    let (name, nl) = match take_name(name_ptr) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    if crate::house::light_live() {
        return err(EAGAIN);
    }
    let nm = &name[..nl];
    match open() {
        Some(c) => match find(&c, nm, 1) {
            Ok((src, len)) => kindle_bytes(src, len),
            Err(e) if e == EIO => err(EIO),
            Err(_) => spawn_volume(nm, true),
        },
        None => spawn_volume(nm, false),
    }
}

/// Loose `ember`: verify, copy, private CR3 + cup, enter at CPL3.
/// Ember kindles `wick`; the Light stays. Never returns (ember `exit`s).
#[cfg(feature = "spawn-test")]
pub fn run_spawn(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    let (src, len) = {
        let Some(c) = open() else {
            spawn_fail(1);
        };
        match find(&c, b"ember", 1) {
            Ok(v) => v,
            Err(_) => spawn_fail(2),
        }
    };
    let realm = match load_spark(src, len) {
        Ok(r) => r,
        Err(_) => spawn_fail(3),
    };
    serial_print("kindling: spawn ok\n");
    crate::house::enter_user_in(realm.entry, realm.cup_top, realm.cr3);
}

/// Loose `leaf`: gleans `LEAF`, writes the page, waits for `q` or `r`.
/// Never returns.
#[cfg(feature = "leaf-test")]
pub fn run_leaf(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    let (src, len) = {
        let Some(c) = open() else {
            spawn_fail(1);
        };
        match find(&c, b"leaf", 1) {
            Ok(v) => v,
            Err(_) => spawn_fail(2),
        }
    };
    let realm = match load_spark(src, len) {
        Ok(r) => r,
        Err(_) => spawn_fail(3),
    };
    serial_print("kindling: leaf ok\n");
    crate::house::enter_user_in(realm.entry, realm.cup_top, realm.cr3);
}

/// Loose `ingle`: greeter spark, private CR3 + cup, waits for a key.
/// Never returns.
#[cfg(feature = "ingle-test")]
pub fn run_ingle(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    let (src, len) = {
        let Some(c) = open() else {
            spawn_fail(1);
        };
        match find(&c, b"ingle", 1) {
            Ok(v) => v,
            Err(_) => spawn_fail(2),
        }
    };
    let realm = match load_spark(src, len) {
        Ok(r) => r,
        Err(_) => spawn_fail(3),
    };
    serial_print("kindling: ingle ok\n");
    crate::house::enter_user_in(realm.entry, realm.cup_top, realm.cr3);
}

/// Paved path: light `ingle` when the cairn has it. Missing cairn or spark
/// writes `no ingle` on glass + serial — never a FAIL line on the happy kernel.
#[allow(dead_code)]
pub fn light_ingle(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    if let Some(c) = open()
        && let Ok((src, len)) = find(&c, b"ingle", 1)
        && let Ok(realm) = load_spark(src, len)
    {
        serial_print("kindling: ingle ok\n");
        crate::house::enter_user_in(realm.entry, realm.cup_top, realm.cr3);
    }
    serial_print("kindling: no ingle\n");
    let msg = b"no ingle\n";
    crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
    hcf();
}

/// Loose rustc Gleam ELF: verify, map at its own VA, enter at `e_entry`.
/// Never returns (the spark `write`s then `exit`s).
#[cfg(feature = "gleam-test")]
pub fn run_gleam(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    let (src, len) = {
        let Some(c) = open() else {
            spawn_fail(1);
        };
        match find(&c, b"gleam", 1) {
            Ok(v) => v,
            Err(_) => spawn_fail(2),
        }
    };
    let realm = match load_spark(src, len) {
        Ok(r) => r,
        Err(_) => spawn_fail(3),
    };
    serial_print("kindling: gleam ok\n");
    crate::house::enter_user_in(realm.entry, realm.cup_top, realm.cr3);
}

/// Loose the `tale` spark: verify, copy it out of the cairn onto private
/// pool pages, point RSP0 home, enter at CPL3. The copy matters: the spark's
/// buf lives inside its own record, and running in place lets its writes
/// perturb the image the next boot verifies — the floor must be its own.
/// Never returns (the spark exits via the house gate).
#[cfg(feature = "tale-test")]
pub fn run_tale(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    let (src, len) = {
        let Some(c) = open() else {
            serial_print("kindling: tale FAIL no-cairn\n");
            hcf();
        };
        match find(&c, b"tale", 1) {
            Ok(v) => v,
            Err(_) => {
                serial_print("kindling: tale FAIL no-spark\n");
                hcf();
            }
        }
    };
    // Sparks are small by law (64 KiB); the first pop off a fresh pool is a
    // contiguous run, asserted below — no silent scatter.
    if len == 0 || len > 65536 {
        serial_print("kindling: tale FAIL bad-spark\n");
        hcf();
    }
    let pages = len.div_ceil(4096);
    let mut dst = 0u64;
    let mut prev = 0u64;
    let mut i = 0u64;
    while i < pages {
        let p = crate::mm::page_alloc();
        if dst == 0 {
            dst = p;
        } else if p != prev + 4096 {
            serial_print("kindling: tale FAIL scattered\n");
            hcf();
        }
        prev = p;
        let mut j = 0u64;
        while j < 4096 && i * 4096 + j < len {
            let b = unsafe { ((src + i * 4096 + j) as *const u8).read_volatile() };
            unsafe { ((p + j) as *mut u8).write_volatile(b) };
            j += 1;
        }
        i += 1;
    }
    serial_print("kindling: tale ok\n");
    crate::house::enter_user(dst);
}
