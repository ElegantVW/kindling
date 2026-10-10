# Cairn — where Gleam keeps leaves and sparks

The cairn is the pile of stones on the disk: every leaf (file) and spark
(executable) the loader lays by the kernel. Packed at cast time by
`scripts/mkimg.py`, delivered to RAM by `ld/cerne-ld.asm`, read by Kindling.
Gleam-shaped. `glean` gathers a named leaf from the cairn first; when that
name is missing and a FAT volume is live (the Databar's medium), it gathers
from the volume's **root** by the same Gleam name (1–64, no `/`). FAT 8.3
and LFN fold into that name. No `open`, no paths this sitting.

## Disk

```
LBA 0        KMAP  (magic, checksum, kernel_* — plus cairn_* below)
LBA 1..32    loader slot (16 KiB, 'LDOK' trailer)
LBA 33..     kernel (sector-padded)
...          cairn (sector-padded; location in KMAP)
```

## KMAP (disk LBA 0)

The loader reads only offsets 0–16 and checks the old checksum — those never
move, so the loader's trust is untouched. Cairn fields ride after, with their
own checksum the kernel checks:

| Off | Size | |
|---|---|---|
| 0 | u32 | `'KMAP'` (`0x50414D4B`) |
| 4 | u32 | checksum = `kernel_lba xor kernel_sectors xor kernel_xor xor 'KMAP'` |
| 8 | u32 | `kernel_lba` |
| 12 | u32 | `kernel_sectors` |
| 16 | u32 | `kernel_xor` |
| 20 | u32 | `cairn_lba` (0 = no cairn packed) |
| 24 | u32 | `cairn_sectors` |
| 28 | u32 | `cairn_sum` = `cairn_lba xor cairn_sectors xor cairn_xor xor 'CAIR'` |

`cairn_xor` is the XOR of every dword of the sector-padded cairn, same rite
as the kernel. A missing cairn (`cairn_lba == 0`) is honest: `glean` then
walks a live FAT volume's root, or refuses `-ENODEV` when that volume is
dark too. A cairn that is present but has no such name is `-ENOENT` (or
the volume leaf, when FAT is live). A bad cairn xor stays `-EIO` — never
partial bytes, never a silent fall-through.

## Cairn image (at `cairn_lba`, in RAM at `0x100000`)

| Off | Size | |
|---|---|---|
| 0 | u32 | `'CAIR'` (`0x52494143`) |
| 4 | u32 | version (`1`) |
| 8 | u32 | `nleaves` |

Then `nleaves` records, each 4-aligned:

| Field | Size |
|---|---|
| kind | u32 (`0` = leaf/data, `1` = spark/executable) |
| namelen | u8 (1–64; house names are short) |
| name | bytes (no NUL, no `/` — flat grove, no paths yet) |
| datalen | u32 (≤ 1 MiB) |
| data | bytes |
| xor | u32 (XOR of every dword of the 4-padded data) |

The kernel verifies magic + walk + per-leaf xor on every read. A bad xor is
`-EILSEQ` territory reported as refusal, never partial bytes.

## Loader's oath

`cerne-ld` reads the KMAP, loads the kernel as before, then — only if
`cairn_sectors > 0` — loads the cairn straight to `0x100000` the same way
(128 sectors per command). A disk failure there refuses like any other:
cause line, house line, `hlt`. Never half a jump. The kernel re-verifies;
the loader only carries.

The BIOS loader lays the cairn from disk when packed. EFI plants the same
bytes from `\EFI\BOOT\CAIRN`: GetProtocol SimpleFileSystem on the loaded
image's device, `LocateDevicePath` on that device path, then on the
loaded-image device path (USB parent is too short; the file path includes
the HD node), then every SimpleFileSystem handle until `CAIRN` opens
(never exclusive — Insyde hung on exclusive GOP). Nested `EFI` / `BOOT`
/ `CAIRN` opens if a path string misses. AllocateAddress `0x100000`
(512 KiB, then 64/16/4 KiB if firmware refuses), else AnyPages and copy
after ExitBootServices when that slot is conventional/loader and not the
running image. KMAP scratch at `0x8400` after EBS.
`scripts/mkimg.py --cairn-out` writes the blob. A miss is honest — Grove
still paints, the glass says `no ingle`. Limine still has no cairn.

## Laying down (stow, G2b)

`stow` re-inks a cairn leaf with the same measure (`len` must equal
`datalen`; ink is for leaves, sparks refuse). Cairn first (kind 0). The ATA rite:

1. Verify (KMAP scratch → checksum → walk → leaf xor, as `glean` does).
2. Copy the bytes + recompute the leaf xor + recompute the cairn sum, all in
   the RAM copy.
3. ATA-write every touched cairn sector back, re-read each through ATA and
   byte-compare (`-EIO` on any mismatch — never trust an unproven write).
4. Rewrite LBA0 with the new sum (without it the next boot distrusts the
   cairn) and verify that sector too.

When that name is missing (or there is no cairn) and MSC saw a FAT volume,
`stow` takes the volume's **root** by the same Gleam name.
WRITE(10) each sector, wait, READ(10) compare. The volume serial must be KINDLING
`85C7-AA81` (Linux vfat UUID). A different volume is `-EPERM`. DMA dest
stays the MSC page. On FAT32 a missing or empty root file is created
(allocate clusters, LFN + 8.3, both FAT copies), and a present file grows or
shrinks to `len` (extra clusters allocated and zeroed; a shorter file frees
the tail). FAT16 still re-inks same measure only.

Sparks run from a private pool copy, never in place: a spark's buf lives
inside its own record, and running in place lets its writes perturb the image
the next boot verifies — the floor must be its own (contiguity asserted,
`tale FAIL scattered` / `spawn FAIL 4` otherwise). `spawn` (house
call 5) copies the named spark the same way from the cairn first (kind 1);
when that name is missing and a FAT volume is live, it gathers from the
volume's **root** by the same Gleam name (exact size, no truncated spark).
Raw `nasm -f bin` stays 64 KiB and identity-mapped (entry at offset 0).
A Gleam ELF64 little-endian x86_64 `ET_EXEC` (our headers, house calls,
not a Linux ABI, no `PT_INTERP`) may be 256 KiB; Kindling maps its
`PT_LOAD` at the spark's own VA (1.25 GiB, above the EFI 1 GiB well and
below QEMU GOP at 2 GiB, so it never sits on iron GOP at 1 GiB) and enters at `e_entry`. Then it clones the kernel map onto
a new CR3, strips U/S from kernel leaves, and grants user only the spark
and a 64 KiB cup (one guard page not-present). One Light: kindle the spark;
when it smoors, `spawn` returns the last word. Nested spawn is `-EAGAIN`.
FAT is the medium — DMA dest stays the MSC page.

A panic mid-stow leaves the disk half-inked (sector new, KMAP old) — the
next boot then refuses `no-cairn` instead of reading corrupt. That refusal
is the design working.

## Words (house voice, no collisions)

- **cairn** — the store (stone pile for many pages; `fae-names.md` §6).
- **leaf** — a file (the Grove's leaves). Flat names, no paths yet. Also
  the G33 spark: gleans Gleam name `LEAF`, writes the page, reads until `q`.
- **spark** — an executable leaf (small fire that runs at CPL3).
- **glean** — house call 8: gather a leaf's bytes (cairn first, then the
  volume root when FAT is live).
- **stow** — house call 9 (lay bytes down; G2b ATA path, G36 FAT path).
- **spawn** — house call 5 (named spark, private pages, own cup + CR3;
  cairn first, then the volume root when FAT is live).
- **tale** — the first spark: speaks a leaf, then exits.
- **first-leaf** — the first leaf, packed at cast time.
- **wick** — the second spark: tells `second-leaf` from its own cup, then exits.
- **ember** — the live coal (G30): kindles `wick`, writes `stayed`, exits with wick's last word. Packed on the spawn-test image. G31: kindles `splanc` when that spark is packed; spawn refuses, ember exits 1.
- **gleam** — G55 rustc `no_std` spark. House `write`, then G56 `flush` (present + a glass-only line), then `exit 0`. Linked at 1.25 GiB (G58). Packed on the gleam-test image and, from G57, on the paved cairn / ingle-leaf / efi-ingle so ingle kindles it after the fire. Proof lines `the rust is lit` and `the glass is ours`.
- **splanc** — a spark that does not catch (G31). Irish *splanc*. `ud2`; the house names the miss.
- **second-leaf** — `the cup is its own`.
- **ingle** — the greeter spark, the desktop (G34). Writes its name, kindles `keeper` when present, greets an inked `hand` after last word 0, then Enter lights the fire, `r` recasts, `q` smoors. After the fire is lit, kindles `gleam` once (G57; a miss is quiet; a last word other than 0 fails), then `bud` once (G59; a miss is quiet; a last word other than 0 fails), then `pane` once (G60; a miss is quiet; a last word other than 0 fails), then `leaf` once, writes `the light remains` when that spark smoors, waits for `q` (smoor) or `r` (recast, house `cast`). Packed on `make image` with `gleam`, `bud`, `pane`, `leaf`, `LEAF`, `keeper`, wax `hand`, and wax `hands`/`twin`; EFI copies the blob from `EFI/BOOT/CAIRN`. A miss of `gleam`, `bud`, `pane`, `leaf`, or `keeper` is quiet. A last word other than 0 from `keeper` does not light the fire.
- **bud** — G59 grow spark. Gleans FAT `page`; wax (10) or a miss becomes ink (600) and writes `the leaf grew`. Already inked it writes `kept`. A dark volume is quiet (`exit 0`) so ingle still kindles `pane`. Not packed as a cairn *leaf* named `page`.
- **pane** — G60 tiny TUI. Gleam ELF at 1.25 GiB. Writes `the pane is open`, presents flush, then form-feed frames the left glass. `q` writes `the pane is shut` and `exit 0`. `r` recasts. Packed on ingle-leaf, efi-grow, and the paved cairn. G61: `.got` stays in the load segment so a rustc libcall cannot jump through address 8.
- **page** — G59 FAT leaf. Wax `wax waits` (10). Ink `a longer leaf` plus `L` fill (600). Not in the cairn. KINDLING `85C7-AA81` only.
- **keeper** — G37 Setup Assistant, G40 hall. Empty book: name, word, word again, `enlist`. Later: list keepers; digit `choose`s, `n` enlists, `d` dismisses, `r` recasts. Printable name keys echo; the word echoes as stars; backspace edits (G38). The hall answers (G41): `the name is cut`, `the word sleeps in the twin`, `this fire knows you`, `that name is given back`; a confirm miss is `the second word is a stranger`.
- **hand** — G37/G39 chosen name. Measure 32. Wax is 32 zeros. `enlist`/`choose` ink it. A miss there is quiet. `ingle` greets it after a successful last word 0.
- **hands** — G39 book. Exact measure 716. AES-256-GCM under `SHA-256("kindling-hands"\|\|vol_id)`. Roster of 8 slots. Not a spark. FAT root on EFI / KINDLING; cairn on the BIOS hearth.
- **twin** — G39 snapshot of the same plaintext book, different wrapping key. Byte-compare after unseal. Two live seals that disagree is `the book is split`. A lone leaf (one side missing or wax) is an empty book, so a miss that only inks `hands` can enlist again.
- **roll / enlist / choose / dismiss** — house calls 11–14. The kernel owns the keys. Sparks never see wrapping keys or verifiers.
- **LEAF** — G32/G33 proof leaf. Page `the volume speaks`. Cairn on the paved / BIOS leaf image; FAT root on QEMU efi-msc / efi-leaf / the Databar.
- **slate** — G19/G36 proof leaf. Ten bytes, wax `wax waits` then ink `ink holds`. Cairn on the BIOS tale image; FAT root (LFN `slate`) on QEMU efi-stow. Volume serial KINDLING `85C7-AA81`.
