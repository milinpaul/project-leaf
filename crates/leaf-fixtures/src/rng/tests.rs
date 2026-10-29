//! Tests for the seeded generator.

use super::Rng;

#[test]
fn the_same_seed_gives_the_same_numbers() {
    let a: Vec<u64> = (0..5).scan(Rng::new(7), |rng, _| Some(rng.next_u64())).collect();
    let b: Vec<u64> = (0..5).scan(Rng::new(7), |rng, _| Some(rng.next_u64())).collect();
    let c: Vec<u64> = (0..5).scan(Rng::new(8), |rng, _| Some(rng.next_u64())).collect();
    assert_eq!(a, b);
    assert_ne!(a, c);
    // Fixed values, so a change to the generator (which would change every fixture) is noticed.
    assert_eq!(Rng::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
}

#[test]
fn ranges_are_respected() {
    let mut rng = Rng::new(1);
    for _ in 0..1_000 {
        assert!(rng.below(10) < 10);
        let n = rng.between(3, 6);
        assert!((3..=6).contains(&n));
    }
    assert_eq!(rng.below(0), 0);
    assert_eq!(rng.between(5, 2), 5);
    assert!(!rng.chance(0));
    assert!(rng.chance(100));
    assert_eq!(rng.pick::<u8>(&[]), None);
    assert_eq!(rng.pick(&[9]), Some(&9));
}
