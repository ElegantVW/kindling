; bud — grow a FAT leaf named page. Quiet if the volume is dark.
; Flat binary, nasm -f bin, entry at offset 0. Position-independent.
; House calls: glean 8, stow 9, write 2, exit 1.
; Wax (10) becomes ink (600) so the chain grows a cluster. Already inked
; it says kept. A miss of the volume exits 0 so ingle still kindles leaf.

        bits    64
        default abs

INK_LEN equ 600
WAX_LEN equ 10

start:
        mov     rax, 8                  ; glean "page"
        lea     rdi, [rel pagename]
        lea     rsi, [rel buf]
        mov     rdx, 1024
        int     0xE0
        test    rax, rax
        js      .create
        cmp     rax, INK_LEN
        je      .maybe_kept
        jmp     .grow

.maybe_kept:
        cld
        lea     rsi, [rel buf]
        lea     rdi, [rel ink]
        mov     ecx, INK_LEN
        repe    cmpsb
        je      .kept

.grow:
        mov     rax, 9                  ; stow ink into page
        lea     rdi, [rel pagename]
        lea     rsi, [rel ink]
        mov     rdx, INK_LEN
        int     0xE0
        test    rax, rax
        js      .quiet
        cmp     rax, INK_LEN
        jne     .quiet
        mov     rax, 8                  ; glean it back
        lea     rdi, [rel pagename]
        lea     rsi, [rel buf]
        mov     rdx, 1024
        int     0xE0
        cmp     rax, INK_LEN
        jne     .bad
        cld
        lea     rsi, [rel buf]
        lea     rdi, [rel ink]
        mov     ecx, INK_LEN
        repe    cmpsb
        jne     .bad
        mov     rax, 2                  ; "the leaf grew"
        mov     rdi, 1
        lea     rsi, [rel m_grew]
        mov     rdx, 14
        int     0xE0
        mov     rax, 1
        xor     edi, edi
        int     0xE0
        ud2

.create:
        mov     rax, 9
        lea     rdi, [rel pagename]
        lea     rsi, [rel ink]
        mov     rdx, INK_LEN
        int     0xE0
        test    rax, rax
        js      .quiet
        cmp     rax, INK_LEN
        jne     .quiet
        mov     rax, 2
        mov     rdi, 1
        lea     rsi, [rel m_grew]
        mov     rdx, 14
        int     0xE0
        mov     rax, 1
        xor     edi, edi
        int     0xE0
        ud2

.kept:
        mov     rax, 2                  ; "kept"
        mov     rdi, 1
        lea     rsi, [rel m_kept]
        mov     rdx, 5
        int     0xE0
        mov     rax, 2                  ; tell the page
        mov     rdi, 1
        lea     rsi, [rel buf]
        mov     rdx, INK_LEN
        int     0xE0
        mov     rax, 1
        xor     edi, edi
        int     0xE0
        ud2

.quiet:
        mov     rax, 1                  ; exit(0) — no KINDLING FAT
        xor     edi, edi
        int     0xE0
        ud2

.bad:
        mov     rax, 1                  ; exit(1) — stow lied
        mov     rdi, 1
        int     0xE0
        ud2

pagename: db    "page", 0
m_grew: db      "the leaf grew", 10
m_kept: db      "kept", 10
ink:    db      "a longer leaf", 10
        times   INK_LEN - 14 db 'L'
buf:    times   1024 db 0
