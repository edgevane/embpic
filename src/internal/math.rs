//! Tiny float helpers shared by backends (no libm, no_std-safe).

/// e^-x for x >= 0 (Schraudolph approx, clamped to [0, 1]).
pub(crate) fn exp_neg(x: f32) -> f32 {
    if x <= 0.0 {
        return 1.0;
    }
    if x > 25.0 {
        return 0.0;
    }
    let v = 12102203.0 * (-x) + 1065353216.0;
    f32::from_bits(v as u32).clamp(0.0, 1.0)
}
