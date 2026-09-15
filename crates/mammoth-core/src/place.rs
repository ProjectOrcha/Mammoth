//! Rendezvous (Highest Random Weight) placement.
//!
//! Pure function: no state, no I/O, no lock. Given a block ID and a topology,
//! every process in the cluster derives the same replica set independently.

/// One candidate machine. `seed` is stable for the life of the node.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Stable worker ID.
    pub id: String,
    /// Failure domain.
    pub rack: String,
    /// Stable hashing seed.
    pub seed: u64,
    /// Higher = more of the cluster's data. 1.0 is the baseline.
    pub weight: f64,
}

/// A 64-bit mix. Deterministic across platforms and Rust versions, which
/// `DefaultHasher` explicitly is not — and placement must never move because
/// someone upgraded a compiler.
///
/// Swap this for `xxhash_rust::xxh3::xxh3_64` when you take the dependency;
/// the property that matters is avalanche, not the specific function.
fn mix64(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

/// The weight this node bids for this block. Highest bid wins.
fn score(block: u64, c: &Candidate) -> f64 {
    let h = mix64(block ^ mix64(c.seed));
    // Map to (0, 1), then apply the standard weighted-rendezvous transform.
    // A node with twice the weight wins twice as often, on average.
    let u = (h >> 11) as f64 / (1u64 << 53) as f64;
    if c.weight <= 0.0 {
        return f64::NEG_INFINITY;
    }
    -c.weight / u.max(f64::MIN_POSITIVE).ln()
}

/// The replica set for `block`, best first, spread across racks.
///
/// Rack rule: take the highest-scoring node from each new rack until we run out
/// of racks, then fall back to the ranking. So replica 2 is always in a
/// different failure domain than replica 1, by construction rather than by
/// policy.
pub fn place(block: u64, candidates: &[Candidate], n: usize) -> Vec<&Candidate> {
    let mut ranked: Vec<_> = candidates
        .iter()
        .filter(|c| c.weight.is_finite() && c.weight > 0.0)
        .map(|c| (score(block, c), c))
        .collect();
    ranked.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));

    let mut out = Vec::with_capacity(n.min(ranked.len()));
    let mut used: std::collections::HashSet<&str> = std::collections::HashSet::new();

    // Pass 1: one per rack, highest scorer first.
    for (_, c) in &ranked {
        if out.len() == n {
            break;
        }
        if !out.iter().any(|o: &&Candidate| o.id == c.id) && used.insert(c.rack.as_str()) {
            out.push(*c);
        }
    }
    // Pass 2: if we need more replicas than there are racks, keep going down
    // the ranking. Fewer racks than replicas is a real cluster shape, not an
    // error — it just means you survive fewer simultaneous rack failures.
    if out.len() < n {
        for (_, c) in &ranked {
            if out.len() == n {
                break;
            }
            if !out.iter().any(|o| o.id == c.id) {
                out.push(*c);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidates() -> Vec<Candidate> {
        (0..12)
            .map(|n| Candidate {
                id: format!("w{n}"),
                rack: format!("r{}", n % 3),
                seed: (n as u64).wrapping_mul(0x9e3779b97f4a7c15),
                weight: 1.0,
            })
            .collect()
    }
    #[test]
    fn placement_is_order_independent_rack_aware_and_stable_on_removal() {
        let full = candidates();
        let reversed: Vec<_> = full.iter().rev().cloned().collect();
        let reduced: Vec<_> = full.iter().filter(|c| c.id != "w7").cloned().collect();
        for block in 0..1000 {
            let chosen = place(block, &full, 3);
            assert_eq!(chosen, place(block, &reversed, 3));
            assert_eq!(
                chosen.iter().map(|c| &c.rack).collect::<std::collections::HashSet<_>>().len(),
                3
            );
            if !chosen.iter().any(|c| c.id == "w7") {
                assert_eq!(chosen, place(block, &reduced, 3));
            }
        }
    }
    #[test]
    fn disabled_workers_do_not_prevent_filling_from_healthy_racks() {
        let mut c = candidates();
        for node in &mut c {
            if node.rack != "r0" {
                node.weight = 0.0;
            }
        }
        assert_eq!(place(1, &c, 3).len(), 3);
        assert_eq!(place(1, &c, 100).len(), 4);
        assert!(place(1, &c, 0).is_empty());
        c[0].weight = f64::NAN;
        assert_eq!(place(1, &c, 100).len(), 3);
    }
}
