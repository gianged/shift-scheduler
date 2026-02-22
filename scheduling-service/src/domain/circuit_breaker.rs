use std::time::{Duration, Instant};

use serde::Deserialize;

/// States of the circuit breaker state machine.
///
/// Transitions: `Closed` -> `Open` (on failure threshold) -> `HalfOpen` (after cooldown) -> `Closed` (on success).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Configuration for the circuit breaker, typically deserialized from TOML config.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub cooldown_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            cooldown_secs: 30,
        }
    }
}

impl CircuitBreakerConfig {
    pub fn cooldown_duration(&self) -> Duration {
        Duration::from_secs(self.cooldown_secs)
    }
}

/// Tracks consecutive failures against a downstream service and prevents calls
/// when the failure threshold is exceeded, allowing the service time to recover.
pub struct CircuitBreaker {
    state: CircuitState,
    consecutive_failures: u32,
    last_failure_time: Option<Instant>,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker in `Closed` state with zero failures.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            last_failure_time: None,
            config,
        }
    }

    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Returns `true` if a call should be attempted.
    ///
    /// When `Open`, transitions to `HalfOpen` if the cooldown has elapsed.
    pub fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_failure) = self.last_failure_time
                    && last_failure.elapsed() >= self.config.cooldown_duration()
                {
                    tracing::info!("Circuit breaker transitioning from Open to HalfOpen");
                    self.state = CircuitState::HalfOpen;
                    return true;
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Records a successful call, resetting the breaker to `Closed`.
    pub fn record_success(&mut self) {
        if self.state != CircuitState::Closed {
            tracing::info!(
                previous_state = ?self.state,
                "Circuit breaker closing after successful call"
            );
        }
        self.consecutive_failures = 0;
        self.last_failure_time = None;
        self.state = CircuitState::Closed;
    }

    /// Records a failed call. Opens the breaker when consecutive failures reach the threshold.
    /// If already `HalfOpen`, immediately re-opens.
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                if self.consecutive_failures >= self.config.failure_threshold {
                    let failures = self.consecutive_failures;
                    tracing::warn!(
                        failures,
                        "Circuit breaker opening after {failures} consecutive failures",
                    );
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                tracing::warn!("Circuit breaker reopening after failure in HalfOpen state");
                self.state = CircuitState::Open;
            }
            CircuitState::Open => {}
        }
    }

    /// Forcibly resets the breaker to `Closed`, used by the health check when the
    /// downstream service is confirmed healthy.
    pub fn force_close(&mut self) {
        if self.state != CircuitState::Closed {
            tracing::info!(
                previous_state = ?self.state,
                "Circuit breaker force-closed by health check"
            );
        }
        self.consecutive_failures = 0;
        self.last_failure_time = None;
        self.state = CircuitState::Closed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_breaker(threshold: u32, cooldown: Duration) -> CircuitBreaker {
        CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: threshold,
            cooldown_secs: cooldown.as_secs(),
        })
    }

    #[test]
    fn starts_closed() {
        let breaker = make_breaker(3, Duration::from_secs(10));
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn stays_closed_below_threshold() {
        let mut breaker = make_breaker(3, Duration::from_secs(10));
        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.can_execute());
    }

    #[test]
    fn opens_at_threshold() {
        let mut breaker = make_breaker(3, Duration::from_secs(10));
        for _ in 0..3 {
            breaker.record_failure();
        }
        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(!breaker.can_execute());
    }

    #[test]
    fn transitions_to_half_open_after_cooldown() {
        let mut breaker = make_breaker(2, Duration::from_millis(0));
        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);

        // Cooldown is 0ms, so it should immediately transition
        assert!(breaker.can_execute());
        assert_eq!(breaker.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn success_in_half_open_closes() {
        let mut breaker = make_breaker(2, Duration::from_millis(0));
        breaker.record_failure();
        breaker.record_failure();
        assert!(breaker.can_execute()); // Transitions to HalfOpen
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        breaker.record_success();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert_eq!(breaker.consecutive_failures, 0);
    }

    #[test]
    fn failure_in_half_open_reopens() {
        let mut breaker = make_breaker(2, Duration::from_millis(0));
        breaker.record_failure();
        breaker.record_failure();
        assert!(breaker.can_execute()); // Transitions to HalfOpen

        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
    }

    #[test]
    fn success_resets_failure_count() {
        let mut breaker = make_breaker(3, Duration::from_secs(10));
        breaker.record_failure();
        breaker.record_failure();
        breaker.record_success();
        assert_eq!(breaker.state(), CircuitState::Closed);

        // Should need 3 more failures to open, not 1
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn force_close_resets_everything() {
        let mut breaker = make_breaker(2, Duration::from_secs(100));
        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);

        breaker.force_close();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert_eq!(breaker.consecutive_failures, 0);
        assert!(breaker.last_failure_time.is_none());
        assert!(breaker.can_execute());
    }

    #[test]
    fn does_not_allow_execution_when_open_and_cooldown_not_elapsed() {
        let mut breaker = make_breaker(2, Duration::from_secs(100));
        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(!breaker.can_execute());
    }
}
