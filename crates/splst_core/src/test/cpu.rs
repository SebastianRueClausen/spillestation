use splst_asm::Register;
use super::run_code;

#[test]
fn zero_reg() {
    let cpu = run_code(r#"
        main:
            li $zero, 1
            break 0
    "#);
    assert_eq!(cpu.read_reg(Register::ZERO), 0);
}

#[test]
fn data() {
    let cpu = run_code(r#"
        main:
            la      $t1, num
            lw      $t2, 0($t1)
            nop

            break 0

        .data
            num: .word 42
    "#);
    assert_eq!(cpu.read_reg(Register::T2), 42);
}

#[test]
fn branch_delay() {
    let cpu = run_code(r#"
        main:
            li      $v0, 0
            j       l1  
            addiu   $v0, $v0, 1
        l1:
            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::V0), 1);
}

#[test]
fn branch_delay_1() {
    let cpu = run_code(r#"
        main:
            beq     $0, $0, part1
            beq     $0, $0, part2
            addi    $3, $0, 1
        part1:
            addi    $1, $0, 1
            beq     $0, $0, end
            nop
        part2:
            addi    $2, $0, 1
        end:
            nop
            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::from(1_u8)), 1);
    assert_eq!(cpu.read_reg(Register::from(2_u8)), 0);
    assert_eq!(cpu.read_reg(Register::from(3_u8)), 0);
}

#[test]
fn load_cancel() {
    let cpu = run_code(r#"
        main:
            li      $t1, 1
            nop

            sw      $t1, 0($0)

            li      $1, 2
            nop

            mfc0    $1, 12
            lw      $1, 0($0)
            mfc0    $1, 15
            lw      $1, 0($0)
            lw      $1, 0($0)
            addiu   $2, $1, 0
            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::from(1_u8)), 1);
    assert_eq!(cpu.read_reg(Register::from(2_u8)), 2);
}

#[test]
fn load_delay() {
    let cpu = run_code(r#"
        main:
            li      $v0, 42
            li      $s1, 43
            la      $v1, 0x0
            sw      $v0, 0($v1)
            lw      $s1, 0($v1)
            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::S1), 43);
}

#[test]
fn simple_loop() {
    let cpu = run_code(r#"
        main:
            li      $v0, 1
        l2:
            sll     $v0, $v0, 1
            slti    $v1, $v0, 1024
            bne     $v1, $zero, l2
            nop

            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::V0), 1024);
}

#[test]
fn sign_extension() {
    let cpu = run_code(r#"
        main:
            li      $t3, 0x8080
            sw      $t3, 0($0)

            lh      $1, 0($0)
            lhu     $2, 0($0)
            lb      $3, 0($0)
            lbu     $4, 0($0)
            nop

            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::from(1_u32)), 0xffff_8080);
    assert_eq!(cpu.read_reg(Register::from(2_u32)), 0x0000_8080);
    assert_eq!(cpu.read_reg(Register::from(3_u32)), 0xffff_ff80);
    assert_eq!(cpu.read_reg(Register::from(4_u32)), 0x0000_0080);
}

#[test]
fn sll() {
    let cpu = run_code(r#"
        main:
            li      $v0, 8
            sll     $v0, $v0, 2
            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::V0), 8 << 2);
}

#[test]
fn srl() {
    let cpu = run_code(r#"
        main:
            li      $v0, 8
            srl     $v0, $v0, 2
            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::V0), 8 >> 2);
}

#[test]
fn sra() {
    let cpu = run_code(r#"
        main:
            li      $v0, -8
            sra     $v0, $v0, 2
            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::V0), (-8_i32 >> 2) as u32);
}

#[test]
fn sllv() {
    let cpu = run_code(r#"
        main:
            li      $v0, 8
            li      $v1, 2
            sllv    $v0, $v0, $v1
            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::V0), 8 << 2);
}

#[test]
fn srlv() {
    let cpu = run_code(r#"
        main:
            li      $v0, 8
            li      $v1, 2
            srlv    $v0, $v0, $v1
            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::V0), 8 >> 2);
}

#[test]
fn jalr() {
    let cpu = run_code(r#"
        main:
            la      $v0, l1
            li      $ra, 0
            li      $a0, 0
            li      $a1, 0

            jalr    $ra, $v0

            li      $a0, 3
            li      $a1, 4

        l1:
            break   0
    "#);
    assert_ne!(cpu.read_reg(Register::RA), 0);
    assert_eq!(cpu.read_reg(Register::A0), 3);
    assert_ne!(cpu.read_reg(Register::A1), 4);
}

#[test]
fn bltzal() {
    let cpu = run_code(r#"
        main:
            li      $t0, -1
            bltzal  $t0, l1 
            nop
            li      $t0, 1
        l1:
            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::T0), (-1_i32) as u32);
    assert_ne!(cpu.read_reg(Register::RA), 0);
}

#[test]
fn bgezal() {
    let cpu = run_code(r#"
        main:
            li      $5, -1
            move    $1, $0
            move    $31, $0
            bltzal  $0, nottaken0
            nop
            li      $1, 1
        nottaken0:
            sltu    $2, $0, $31
            li      $3, -1
            move    $31, $0
            bgezal  $3, nottaken1
            nop
            li      $3, 1
        nottaken1:
            sltu    $4, $0, $31
            li      $5, -1
            move    $31, $0
            bltzal  $5, taken0
            nop
            li      $5, 1
        taken0:
            sltu    $6, $0, $31
            move    $7, $0
            move    $31, $0
            bgezal  $0, taken1
            nop
            li      $7, 1
        taken1:
            sltu    $8, $0, $31

            break   0
    "#);
    assert_eq!(cpu.read_reg(Register::from(1_u32)), 1);
    assert_eq!(cpu.read_reg(Register::from(2_u32)), 1);
    assert_eq!(cpu.read_reg(Register::from(3_u32)), 1);
    assert_eq!(cpu.read_reg(Register::from(4_u32)), 1);
    assert_eq!(cpu.read_reg(Register::from(5_u32)), (-1_i32) as u32);
    assert_eq!(cpu.read_reg(Register::from(6_u32)), 1);
    assert_eq!(cpu.read_reg(Register::from(7_u32)), 0);
    assert_eq!(cpu.read_reg(Register::from(8_u32)), 1);
}

#[test]
fn addiu() {
    let cpu = run_code(r#"
        main:
            li      $v0, 0
            addiu   $v0, $v0, -1

            li      $v1, -1
            addiu   $v1, $v1, 1

            break 0
    "#);
    assert_eq!(cpu.read_reg(Register::V0), (-1_i32) as u32);
    assert_eq!(cpu.read_reg(Register::V1), 0);
}

#[test]
fn lwl_lwr() {
    let cpu = run_code(r#"
        main:
            li      $t1, 0x76543210
            sw      $t1, 0($0)

            li      $t1, 0xfedcba98
            sw      $t1, 4($0)

            lwr     $1, 0($0)
            lwl     $1, 3($0)
            lwr     $2, 1($0)
            lwl     $2, 4($0)
            lwr     $3, 2($0)
            lwl     $3, 5($0)
            lwr     $4, 3($0)
            lwl     $4, 6($0)
            lwr     $5, 4($0)
            lwl     $5, 7($0)
            lwl     $6, 3($0)
            lwr     $6, 0($0)
            lwl     $7, 4($0)
            lwr     $7, 1($0)
            lwl     $8, 5($0)
            lwr     $8, 2($0)
            lwl     $9, 6($0)
            lwr     $9, 3($0)
            lwl     $10, 7($0)
            lwr     $10, 4($0)
            addiu   $11, $0, -1
            lwl     $11, 0($0)
            addiu   $12, $0, -1
            lwr     $12, 0($0)
            addiu   $13, $0, -1
            lwl     $13, 1($0)
            addiu   $14, $0, -1
            lwr     $14, 1($0)
            addiu   $15, $0, -1
            lwl     $15, 2($0)
            addiu   $16, $0, -1
            lwr     $16, 2($0)
            addiu   $17, $0, -1
            lwl     $17, 3($0)
            addiu   $18, $0, -1
            lwr     $18, 3($0)
            nop
            break   0
    "#);

    let values: [u32; 18] = [
        0x76543210, 0x98765432, 0xba987654, 0xdcba9876, 0xfedcba98, 0x76543210, 0x98765432,
        0xba987654, 0xdcba9876, 0xfedcba98, 0x10ffffff, 0x76543210, 0x3210ffff, 0xff765432,
        0x543210ff, 0xffff7654, 0x76543210, 0xffffff76,
    ];

    for (i, val) in values.iter().enumerate() {
        assert_eq!(cpu.read_reg(Register::from(i as u32 + 1)), *val);
    }
}

#[test]
fn lwl_lwr_1() {
    let cpu = run_code(r#"
        main:
            li      $t1, 0x76543210
            sw      $t1, 0($0)

            li      $t1, 0xfedcba98
            sw      $t1, 4($0)

            addiu       $1, $0, -1
            lwr         $1, 2($0)
            lwl         $1, 5($0)
            move        $2, $1
            addiu       $3, $0, -1
            lwr         $3, 2($0)
            nop
            lwl         $3, 5($0)
            move        $4, $3
            addiu       $5, $0, -1
            lwl         $5, 5($0)
            nop
            lwr         $5, 2($0)
            move        $6, $5
            addiu       $7, $0, -1
            lw          $7, 4($0)
            lwl         $7, 2($0)
            move        $8, $7
            addiu       $9, $0, -1
            lw          $9, 4($0)
            nop
            lwl         $9, 2($0)
            move        $10, $9
            addiu       $11, $0, -1
            lw          $11, 4($0)
            lwr         $11, 2($0)
            move        $12, $11
            addiu       $13, $0, -1
            lw          $13, 4($0)
            nop
            lwr         $13, 2($0)
            move        $14, $13
            lui         $15, 0x67e
            ori         $15, $15, 0x67e
            mtc2        $15, 25
            addiu       $15, $0, -1
            mfc2        $15, 25
            lwl         $15, 1($0)
            move        $16, $15
            addiu       $17, $0, -1
            mfc2        $17, 25
            nop
            lwr         $17, 1($0)
            move        $18, $17
            nop 

            break       0
    "#);

    let values: [u32; 18] = [
        0xba987654, 0xffffffff, 0xba987654, 0xffff7654, 0xba987654, 0xba98ffff, 0x54321098,
        0xffffffff, 0x54321098, 0xfedcba98, 0xfedc7654, 0xffffffff, 0xfedc7654, 0xfedcba98,
        0x3210067e, 0xffffffff, 0x06765432, 0x067e067e,
    ];

    for (i, val) in values.iter().enumerate() {
        assert_eq!(cpu.read_reg(Register::from(i as u32 + 1)), *val);
    }
}

#[test]
fn swl_swr() {
    let cpu = run_code(r#"
        main:
            li      $1, 0
            li      $2, 0x76543210
            li      $3, 0xfedcba98

            sw      $2, 0($1)
            swl     $3, 0($1)
            addiu   $1, $1, 4
            sw      $2, 0($1)
            swl     $3, 1($1)	
            addiu   $1, $1, 4
            sw      $2, 0($1)
            swl     $3, 2($1)	
            addiu   $1, $1, 4
            sw      $2, 0($1)
            swl     $3, 3($1)
            addiu   $1, $1, 4
            sw      $2, 0($1)
            swr     $3, 0($1)
            addiu   $1, $1, 4
            sw      $2, 0($1)
            swr     $3, 1($1)	
            addiu   $1, $1, 4
            sw      $2, 0($1)
            swr     $3, 2($1)	
            addiu   $1, $1, 4
            sw      $2, 0($1)
            swr     $3, 3($1)

            break   0
    "#);

    assert_eq!(cpu.bus.peek::<u32>(0), Some(0x765432fe));
    assert_eq!(cpu.bus.peek::<u32>(4), Some(0x7654fedc));
    assert_eq!(cpu.bus.peek::<u32>(8), Some(0x76fedcba));
    assert_eq!(cpu.bus.peek::<u32>(12), Some(0xfedcba98));
    assert_eq!(cpu.bus.peek::<u32>(16), Some(0xfedcba98));
    assert_eq!(cpu.bus.peek::<u32>(20), Some(0xdcba9810));
    assert_eq!(cpu.bus.peek::<u32>(24), Some(0xba983210));
    assert_eq!(cpu.bus.peek::<u32>(28), Some(0x98543210));
}
