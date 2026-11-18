use super::*;

#[test]
fn vector_empty_accepts_signed_zero_and_rejects_every_nonzero_or_nan_field() {
    let zero = FlakeVector {
        timing_cv: -0.0,
        ..Default::default()
    };
    assert!(zero.is_empty());
    for value in [1.0, -1.0, f64::MIN_POSITIVE, f64::INFINITY, f64::NAN] {
        let vectors = [
            FlakeVector {
                timing_cv: value,
                ..Default::default()
            },
            FlakeVector {
                locator_churn: value,
                ..Default::default()
            },
            FlakeVector {
                race_score: value,
                ..Default::default()
            },
            FlakeVector {
                network_jitter_ms: value,
                ..Default::default()
            },
            FlakeVector {
                animation_score: value,
                ..Default::default()
            },
        ];
        assert!(vectors.iter().all(|vector| !vector.is_empty()));
    }
}
