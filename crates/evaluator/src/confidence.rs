//! The statistical confidence filter.
//!
//! Sub-microsecond micro-opportunities are often ghosts: they exist for one
//! WebSocket frame and vanish before an order can reach Binance. Three gates cut
//! them:
//!
//! * Persistence: the cycle has been profitable for at least `N` consecutive
//!   book updates.
//! * Depth confidence: the profitable size at the top of book is at least `K`
//!   times the configured minimum notional, so a small competing order does not
//!   erase it before we arrive.
//! * Latency-adjusted expected value: raw profit is multiplied by an estimated
//!   probability that the opportunity survives the round trip, drawn from the
//!   empirical distribution of opportunity lifetimes measured during operation.
//!
//! Only opportunities passing all gates proceed to execution.

/// Tunable parameters for the confidence filter.
#[derive(Debug, Clone, Copy)]
pub struct ConfidenceParams {
    /// `N`: consecutive profitable updates required.
    pub persistence_required: u32,
    /// `K`: profitable size must be at least this multiple of the minimum
    /// notional.
    pub depth_k: f64,
    /// Baseline survival probability before latency decay.
    pub base_survival: f64,
    /// Half-life (ms) of opportunity survival: the round-trip latency at which
    /// survival probability halves relative to baseline.
    pub survival_half_life_ms: f64,
}

impl Default for ConfidenceParams {
    fn default() -> ConfidenceParams {
        ConfidenceParams {
            persistence_required: 2,
            depth_k: 1.5,
            base_survival: 0.9,
            survival_half_life_ms: 20.0,
        }
    }
}

/// The observed signal for one opportunity at decision time.
#[derive(Debug, Clone, Copy)]
pub struct ConfidenceSignal {
    /// Consecutive profitable book updates observed for this cycle.
    pub persistence_count: u32,
    /// The size (in the starting asset) profitable at the top of book.
    pub profitable_size: f64,
    /// The configured minimum notional (same units).
    pub min_notional: f64,
    /// Estimated round-trip latency (ms) for reaching Binance.
    pub round_trip_latency_ms: f64,
}

/// Whether the persistence and depth-confidence gates pass.
#[must_use]
pub fn passes_filters(params: &ConfidenceParams, signal: &ConfidenceSignal) -> bool {
    signal.persistence_count >= params.persistence_required
        && signal.profitable_size >= params.depth_k * signal.min_notional
}

/// Estimated probability that an opportunity survives a round trip of
/// `latency_ms`, decaying exponentially from `base` with the given half-life.
#[must_use]
pub fn survival_probability(base: f64, latency_ms: f64, half_life_ms: f64) -> f64 {
    if half_life_ms <= 0.0 {
        return base.clamp(0.0, 1.0);
    }
    let decay = 0.5_f64.powf(latency_ms.max(0.0) / half_life_ms);
    (base * decay).clamp(0.0, 1.0)
}

/// Latency-adjusted expected value: raw profit scaled by survival probability.
#[must_use]
pub fn latency_adjusted_ev(raw_profit: f64, survival: f64) -> f64 {
    raw_profit * survival.clamp(0.0, 1.0)
}

/// Convenience: full evaluation returning the latency-adjusted EV if all gates
/// pass, else `None`.
#[must_use]
pub fn evaluate(
    params: &ConfidenceParams,
    signal: &ConfidenceSignal,
    raw_profit: f64,
) -> Option<f64> {
    if !passes_filters(params, signal) {
        return None;
    }
    let survival = survival_probability(
        params.base_survival,
        signal.round_trip_latency_ms,
        params.survival_half_life_ms,
    );
    let ev = latency_adjusted_ev(raw_profit, survival);
    if ev > 0.0 {
        Some(ev)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(persistence: u32, size: f64, latency: f64) -> ConfidenceSignal {
        ConfidenceSignal {
            persistence_count: persistence,
            profitable_size: size,
            min_notional: 100.0,
            round_trip_latency_ms: latency,
        }
    }

    #[test]
    fn persistence_gate() {
        let p = ConfidenceParams::default(); // requires 2
        assert!(!passes_filters(&p, &signal(1, 1000.0, 5.0)));
        assert!(passes_filters(&p, &signal(2, 1000.0, 5.0)));
    }

    #[test]
    fn depth_gate() {
        let p = ConfidenceParams::default(); // K = 1.5, min 100 => need >= 150
        assert!(!passes_filters(&p, &signal(5, 149.0, 5.0)));
        assert!(passes_filters(&p, &signal(5, 150.0, 5.0)));
    }

    #[test]
    fn survival_decays_with_latency() {
        // At one half-life, survival is halved.
        let s0 = survival_probability(0.9, 0.0, 20.0);
        let s1 = survival_probability(0.9, 20.0, 20.0);
        assert!((s0 - 0.9).abs() < 1e-12);
        assert!((s1 - 0.45).abs() < 1e-12);
        assert!(s1 < s0);
    }

    #[test]
    fn evaluate_gates_and_scales() {
        let p = ConfidenceParams::default();
        // Passes gates; EV = profit * survival.
        let ev = evaluate(&p, &signal(3, 1000.0, 0.0), 10.0).unwrap();
        assert!((ev - 9.0).abs() < 1e-9); // 10 * 0.9
                                          // Fails persistence.
        assert!(evaluate(&p, &signal(1, 1000.0, 0.0), 10.0).is_none());
        // Non-positive profit rejected.
        assert!(evaluate(&p, &signal(3, 1000.0, 0.0), 0.0).is_none());
    }
}
