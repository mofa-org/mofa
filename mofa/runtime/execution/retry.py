"""Retry and backoff policy primitives for execution planning."""

from __future__ import annotations

from dataclasses import dataclass
import random
from typing import Any, Mapping, Optional, Tuple


@dataclass(frozen=True)
class RetryPolicy:
    max_attempts: int = 1
    initial_delay_seconds: float = 0.0
    backoff_multiplier: float = 2.0
    max_delay_seconds: float = 30.0
    jitter_ratio: float = 0.0


DEFAULT_RETRY_POLICY = RetryPolicy()


class RetryPolicyError(ValueError):
    """Raised when retry policy values are invalid."""


def validate_retry_policy(policy: RetryPolicy) -> RetryPolicy:
    if policy.max_attempts < 1:
        raise RetryPolicyError("max_attempts must be >= 1")
    if policy.initial_delay_seconds < 0:
        raise RetryPolicyError("initial_delay_seconds must be >= 0")
    if policy.backoff_multiplier < 1:
        raise RetryPolicyError("backoff_multiplier must be >= 1")
    if policy.max_delay_seconds < 0:
        raise RetryPolicyError("max_delay_seconds must be >= 0")
    if not 0 <= policy.jitter_ratio <= 1:
        raise RetryPolicyError("jitter_ratio must be in [0, 1]")
    return policy


def retry_policy_from_mapping(raw: Optional[Mapping[str, Any]]) -> RetryPolicy:
    if raw is None:
        return DEFAULT_RETRY_POLICY
    if not isinstance(raw, Mapping):
        raise RetryPolicyError("retry policy must be a mapping")

    policy = RetryPolicy(
        max_attempts=int(raw.get("max_attempts", DEFAULT_RETRY_POLICY.max_attempts)),
        initial_delay_seconds=float(raw.get("initial_delay_seconds", DEFAULT_RETRY_POLICY.initial_delay_seconds)),
        backoff_multiplier=float(raw.get("backoff_multiplier", DEFAULT_RETRY_POLICY.backoff_multiplier)),
        max_delay_seconds=float(raw.get("max_delay_seconds", DEFAULT_RETRY_POLICY.max_delay_seconds)),
        jitter_ratio=float(raw.get("jitter_ratio", DEFAULT_RETRY_POLICY.jitter_ratio)),
    )
    return validate_retry_policy(policy)


def compute_backoff_delay(policy: RetryPolicy, attempt_index: int, rand: Optional[float] = None) -> float:
    """Compute delay for retry attempt index (0-based retry index)."""
    validate_retry_policy(policy)
    if attempt_index < 0:
        raise RetryPolicyError("attempt_index must be >= 0")

    base_delay = policy.initial_delay_seconds * (policy.backoff_multiplier ** attempt_index)
    capped = min(base_delay, policy.max_delay_seconds)
    if capped <= 0:
        return 0.0

    if policy.jitter_ratio <= 0:
        return capped

    noise_seed = random.random() if rand is None else rand
    noise_seed = max(0.0, min(1.0, noise_seed))
    jitter_span = capped * policy.jitter_ratio
    jitter = (noise_seed * 2.0 - 1.0) * jitter_span
    return max(0.0, capped + jitter)


def build_retry_schedule(policy: RetryPolicy) -> Tuple[float, ...]:
    validate_retry_policy(policy)
    retries = max(0, policy.max_attempts - 1)
    return tuple(compute_backoff_delay(policy, index, rand=0.5) for index in range(retries))
