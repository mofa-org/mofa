//! Integration tests for MockClock timeout and deadline helpers.

use std::time::Duration;
use mofa_testing::clock::{Clock, MockClock};

#[test]
fn test_peek_millis_does_not_advance_when_auto_advance_enabled() {
    let clock = MockClock::new();
    clock.set_auto_advance(Duration::from_millis(50));

    // peek_millis() should NOT trigger auto-advance
    assert_eq!(clock.peek_millis(), 0);
    assert_eq!(clock.peek_millis(), 0);
    assert_eq!(clock.peek(), Duration::from_millis(0));
    assert_eq!(clock.peek_millis(), 0);

    // now_millis() should return current time and advance
    assert_eq!(clock.now_millis(), 0);
    // After now_millis(), time should have moved forward by 50ms
    assert_eq!(clock.peek_millis(), 50);
    assert_eq!(clock.peek(), Duration::from_millis(50));

    // Another peek_millis() still doesn't advance
    assert_eq!(clock.peek_millis(), 50);

    // Calling now() advances again
    assert_eq!(clock.now(), Duration::from_millis(50));
    assert_eq!(clock.peek_millis(), 100);
}

#[test]
fn test_zero_floor_remaining_duration() {
    let clock = MockClock::starting_at(Duration::from_millis(100));
    let deadline = Duration::from_millis(250);

    // Before deadline: positive remaining duration
    assert_eq!(clock.remaining_duration(deadline), Duration::from_millis(150));

    // Advance right to the deadline
    clock.advance(Duration::from_millis(150));
    assert_eq!(clock.remaining_duration(deadline), Duration::ZERO);
    assert!(clock.is_deadline_passed(deadline));

    // Advance far past the deadline: zero-floor guarantee (must saturate at ZERO, no panic or underflow)
    clock.advance(Duration::from_millis(500));
    assert_eq!(clock.remaining_duration(deadline), Duration::ZERO);
    assert!(clock.is_deadline_passed(deadline));
}

#[test]
fn test_deadline_calculation_and_checks() {
    let clock = MockClock::starting_at(Duration::from_secs(10));
    let timeout = Duration::from_secs(5);

    let deadline = clock.deadline_from_now(timeout);
    assert_eq!(deadline, Duration::from_secs(15));
    assert!(!clock.is_deadline_passed(deadline));
    assert_eq!(clock.remaining_duration(deadline), Duration::from_secs(5));

    // Advance halfway
    clock.advance(Duration::from_millis(2500));
    assert!(!clock.is_deadline_passed(deadline));
    assert_eq!(clock.remaining_duration(deadline), Duration::from_millis(2500));

    // Advance exactly to deadline
    clock.advance(Duration::from_millis(2500));
    assert!(clock.is_deadline_passed(deadline));
    assert_eq!(clock.remaining_duration(deadline), Duration::ZERO);
}

#[test]
fn test_advance_to_and_advance_until_deadline() {
    let clock = MockClock::new();

    // Advance to a future time
    clock.advance_to(Duration::from_millis(300));
    assert_eq!(clock.peek_millis(), 300);

    // Attempting to advance to a past time does not regress time
    clock.advance_to(Duration::from_millis(150));
    assert_eq!(clock.peek_millis(), 300);

    // advance_until_deadline advances forward
    let deadline = Duration::from_millis(750);
    clock.advance_until_deadline(deadline);
    assert_eq!(clock.peek(), deadline);
    assert!(clock.is_deadline_passed(deadline));
}

#[test]
fn test_clock_trait_peek_millis_default() {
    let clock = MockClock::new();
    clock.set_auto_advance(Duration::from_millis(25));

    // Test through the Clock trait
    let trait_clock: &dyn Clock = &clock;
    assert_eq!(trait_clock.peek_millis(), 0);
    assert_eq!(trait_clock.peek_millis(), 0);
    assert_eq!(trait_clock.now_millis(), 0);
    assert_eq!(trait_clock.peek_millis(), 25);
}
