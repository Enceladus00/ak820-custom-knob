@ Hook stubs. Each 6-byte hook replaces "movs rX,#6; cmp.w rX, rCMD, lsr #4"
@ (the group-6 test) with "b.w stub; nop". Non-0x3X commands re-run those two
@ instructions and jump back to the original bne, so stock behaviour is kept.
    .syntax unified
    .cpu cortex-m3
    .thumb

    .section .hook24, "ax"          @ 2.4 GHz dispatcher, at 0xED36
    b.w     stub24
    nop

    .section .hookw, "ax"           @ wired dispatcher, at 0xFC54
    b.w     stubw
    nop

    .section .hookframe, "ax"       @ frame loop, at 0x1054E: was "bl 0x14B90" (LED push)
    bl      ak_frame

    .section .text.stubs, "ax"
    .thumb_func
stub24:                             @ r0 = command, r5/r9 must survive
    lsrs    r1, r0, #4
    cmp     r1, #3
    bne     1f
    ldr     r1, =0x200052CC         @ received frame
    sub     r2, r1, #0x40           @ reply frame
    bl      ak_cmd3x
    b.w     exit24
1:  movs    r1, #6
    cmp.w   r1, r0, lsr #4
    b.w     cont24

    .thumb_func
stubw:                              @ r1 = command, r2 must survive
    lsrs    r3, r1, #4
    cmp     r3, #3
    bne     2f
    mov     r0, r1
    ldr     r1, =0x20003CDE         @ received frame copy
    ldr     r2, =0x20003CBC         @ reply frame
    bl      ak_cmd3x
    b.w     exitw
2:  movs    r3, #6
    cmp.w   r3, r1, lsr #4
    b.w     contw
    .ltorg

    .thumb_set exit24, 0xEF92       @ strb r9,[r5,#3] (reply ready); pop
    .thumb_set cont24, 0xED3C       @ bne 0xED70
    .thumb_set exitw,  0xFAE0       @ pop
    .thumb_set contw,  0xFC5A       @ bne 0xFC84
