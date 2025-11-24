//! Host helper primitives invoked by ISA semantics.
//!
//! The ISA semantics DSL can emit calls like `$host::add(...)`.  This module
//! sketches the runtime surface those helpers will target so emulator
//! integrations can provide concrete implementations while unit tests can rely
//! on a lightweight software fallback.

/// Result of an arithmetic operation performed by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostArithResult {
    /// Truncated result after applying the requested bit-width mask.
    pub value: u64,
    /// Carry or borrow flag depending on the operation context.
    pub carry: bool,
    /// Signed overflow flag computed using the requested bit width.
    pub overflow: bool,
}

impl HostArithResult {
    pub fn new(value: u64, carry: bool, overflow: bool) -> Self {
        Self {
            value,
            carry,
            overflow,
        }
    }
}

/// Result of a wide multiply operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostMulResult {
    /// Low portion of the product, truncated to the requested width.
    pub low: u64,
    /// High bits that spill past the requested width.
    pub high: u64,
}

impl HostMulResult {
    pub fn new(low: u64, high: u64) -> Self {
        Self { low, high }
    }
}

/// Trait describing the primitive helpers surfaced to the semantics DSL.
pub trait HostServices {
    /// Adds two unsigned values using the provided bit width.
    fn add(&mut self, lhs: u64, rhs: u64, carry_in: bool, width: u32) -> HostArithResult;

    /// Subtracts `rhs` from `lhs` within the provided bit width.
    fn sub(&mut self, lhs: u64, rhs: u64, borrow_in: bool, width: u32) -> HostArithResult;

    /// Multiplies two values and returns the full-width product split into low/high pieces.
    fn mul(&mut self, lhs: u64, rhs: u64, width: u32) -> HostMulResult;
}

/// Minimal software fallback so semantics can run inside tests without a host.
#[derive(Debug, Default, Clone)]
pub struct SoftwareHost;

impl HostServices for SoftwareHost {
    fn add(&mut self, lhs: u64, rhs: u64, carry_in: bool, width: u32) -> HostArithResult {
        add_core(lhs, rhs, width, carry_in)
    }

    fn sub(&mut self, lhs: u64, rhs: u64, borrow_in: bool, width: u32) -> HostArithResult {
        let (mask, sign_bit) = mask_and_sign(width);
        let base = add_core(lhs, (!rhs) & mask, width, !borrow_in);
        let overflow = compute_overflow_sub(lhs, rhs, base.value, sign_bit);
        HostArithResult::new(base.value, !base.carry, overflow)
    }

    fn mul(&mut self, lhs: u64, rhs: u64, width: u32) -> HostMulResult {
        let (mask, _) = mask_and_sign(width);
        let product = (lhs as u128).wrapping_mul(rhs as u128);
        let low = (product & mask as u128) as u64;
        let high = (product >> width) as u64;
        HostMulResult::new(low, high)
    }
}

fn add_core(lhs: u64, rhs: u64, width: u32, carry_in: bool) -> HostArithResult {
    let (mask, sign_bit) = mask_and_sign(width);
    let lhs = lhs & mask;
    let rhs = rhs & mask;

    let (sum0, carry0) = lhs.overflowing_add(rhs);
    let (sum, carry1) = sum0.overflowing_add(carry_in as u64);
    let carry_from_width = (sum & !mask) != 0;
    let carry_out = carry0 || carry1 || carry_from_width;
    let value = sum & mask;

    let overflow = if sign_bit == 0 {
        false
    } else {
        let sign_mask = 1u64 << (sign_bit - 1);
        let lhs_sign = lhs & sign_mask != 0;
        let rhs_sign = rhs & sign_mask != 0;
        let res_sign = value & sign_mask != 0;
        (lhs_sign == rhs_sign) && (lhs_sign != res_sign)
    };
    HostArithResult::new(value, carry_out, overflow)
}

fn mask_and_sign(width: u32) -> (u64, u32) {
    match width {
        0 => (0, 0),
        1..=63 => ((1u64 << width) - 1, width),
        _ => (u64::MAX, 64),
    }
}

fn compute_overflow_sub(lhs: u64, rhs: u64, value: u64, sign_bit: u32) -> bool {
    if sign_bit == 0 {
        return false;
    }
    let sign_mask = 1u64 << (sign_bit - 1);
    let lhs_sign = lhs & sign_mask != 0;
    let rhs_sign = rhs & sign_mask != 0;
    let res_sign = value & sign_mask != 0;
    (lhs_sign != rhs_sign) && (lhs_sign != res_sign)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_reports_carry_and_overflow() {
        let mut host = SoftwareHost::default();
        let res = host.add(0x7FFF_FFFF, 0x7FFF_FFFF, false, 32);
        assert_eq!(res.value, 0xFFFF_FFFE);
        assert!(res.overflow);
        assert!(!res.carry);

        let res = host.add(0xFFFF_FFFF, 1, false, 32);
        assert_eq!(res.value, 0);
        assert!(res.carry);
        assert!(!res.overflow);
    }

    #[test]
    fn sub_reports_borrow_and_overflow() {
        let mut host = SoftwareHost::default();
        let res = host.sub(0, 1, false, 32);
        assert_eq!(res.value, 0xFFFF_FFFF);
        assert!(res.carry); // borrow flag
        assert!(!res.overflow);

        let res = host.sub(0x8000_0000, 1, false, 32);
        assert_eq!(res.value, 0x7FFF_FFFF);
        assert!(res.overflow);
    }

    #[test]
    fn add_accepts_carry_in() {
        let mut host = SoftwareHost::default();
        let res = host.add(0xFFFF_FFFF, 0, true, 32);
        assert_eq!(res.value, 0);
        assert!(res.carry);
    }

    #[test]
    fn mul_returns_high_bits() {
        let mut host = SoftwareHost::default();
        let res = host.mul(0x1_0000_0000, 2, 64);
        assert_eq!(res.low, 0x2_0000_0000);
        assert_eq!(res.high, 0);

        let res = host.mul(0xFFFF_FFFF, 0xFFFF_FFFF, 32);
        assert_eq!(res.low, 1);
        assert_eq!(res.high, 0xFFFF_FFFE);
    }
}
