use super::run_code;
use crate::bus::Word;

#[test]
fn otc_clear() {
    let mut cpu = run_code(r#"
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

    assert_eq!(cpu.bus.peek::<Word>(16), Some(12));
    assert_eq!(cpu.bus.peek::<Word>(12), Some(8));
    assert_eq!(cpu.bus.peek::<Word>(8), Some(4));
    assert_eq!(cpu.bus.peek::<Word>(4), Some(0));
}
