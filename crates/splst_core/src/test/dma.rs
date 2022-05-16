use super::run_code;

#[test]
fn otc_clear() {
    let cpu = run_code(r#"
        main:
            la  $t1, 0x1f8010e0

            # Set channel base address.
            li  $t0, 16
            sw  $t0, 0($t1)

            # Set channel block control register. Set the block size to 5 words.
            li  $t0, 5
            sw  $t0, 4($t1)

            # Set channel control register. Sets the enabled and start flag.
            li  $t0, 0x11000002
            sw  $t0, 8($t1)

            break 0
    "#);

    assert_eq!(cpu.bus.peek::<u32>(16), Some(12));
    assert_eq!(cpu.bus.peek::<u32>(12), Some(8));
    assert_eq!(cpu.bus.peek::<u32>(8), Some(4));
    assert_eq!(cpu.bus.peek::<u32>(4), Some(0));
}
