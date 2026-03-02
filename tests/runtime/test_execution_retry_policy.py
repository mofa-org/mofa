import unittest

from mofa.runtime.execution import (
    DEFAULT_RETRY_POLICY,
    RetryPolicy,
    RetryPolicyError,
    build_retry_schedule,
    compute_backoff_delay,
    plan_execution,
    retry_policy_from_mapping,
    validate_retry_policy,
)
from mofa.schema import parse_flow_dict


class RetryPolicyTests(unittest.TestCase):
    def test_validate_retry_policy_accepts_default(self):
        self.assertEqual(validate_retry_policy(DEFAULT_RETRY_POLICY), DEFAULT_RETRY_POLICY)

    def test_validate_retry_policy_rejects_zero_attempts(self):
        with self.assertRaises(RetryPolicyError):
            validate_retry_policy(RetryPolicy(max_attempts=0))

    def test_validate_retry_policy_rejects_negative_delay(self):
        with self.assertRaises(RetryPolicyError):
            validate_retry_policy(RetryPolicy(initial_delay_seconds=-1))

    def test_validate_retry_policy_rejects_backoff_lt_one(self):
        with self.assertRaises(RetryPolicyError):
            validate_retry_policy(RetryPolicy(backoff_multiplier=0.5))

    def test_validate_retry_policy_rejects_negative_max_delay(self):
        with self.assertRaises(RetryPolicyError):
            validate_retry_policy(RetryPolicy(max_delay_seconds=-1))

    def test_validate_retry_policy_rejects_bad_jitter(self):
        with self.assertRaises(RetryPolicyError):
            validate_retry_policy(RetryPolicy(jitter_ratio=2))

    def test_retry_policy_from_mapping_defaults(self):
        policy = retry_policy_from_mapping(None)
        self.assertEqual(policy, DEFAULT_RETRY_POLICY)

    def test_retry_policy_from_mapping_applies_values(self):
        policy = retry_policy_from_mapping({"max_attempts": 3, "initial_delay_seconds": 0.5})
        self.assertEqual(policy.max_attempts, 3)
        self.assertEqual(policy.initial_delay_seconds, 0.5)

    def test_retry_policy_from_mapping_rejects_non_mapping(self):
        with self.assertRaises(RetryPolicyError):
            retry_policy_from_mapping("bad")

    def test_compute_backoff_delay_without_jitter(self):
        policy = RetryPolicy(max_attempts=3, initial_delay_seconds=1, backoff_multiplier=2, max_delay_seconds=10)
        self.assertEqual(compute_backoff_delay(policy, 0), 1)
        self.assertEqual(compute_backoff_delay(policy, 1), 2)

    def test_compute_backoff_delay_respects_cap(self):
        policy = RetryPolicy(max_attempts=5, initial_delay_seconds=5, backoff_multiplier=3, max_delay_seconds=7)
        self.assertEqual(compute_backoff_delay(policy, 2), 7)

    def test_compute_backoff_delay_applies_jitter(self):
        policy = RetryPolicy(max_attempts=3, initial_delay_seconds=10, jitter_ratio=0.1)
        delay = compute_backoff_delay(policy, 0, rand=1.0)
        self.assertAlmostEqual(delay, 11.0)

    def test_compute_backoff_delay_rejects_negative_attempt_index(self):
        with self.assertRaises(RetryPolicyError):
            compute_backoff_delay(DEFAULT_RETRY_POLICY, -1)

    def test_build_retry_schedule_uses_attempts_minus_one(self):
        policy = RetryPolicy(max_attempts=4, initial_delay_seconds=1, backoff_multiplier=2)
        schedule = build_retry_schedule(policy)
        self.assertEqual(schedule, (1, 2, 4))

    def test_plan_execution_metadata_uses_default_retry(self):
        flow = parse_flow_dict(
            {
                "schema_version": 2,
                "nodes": [
                    {"id": "source", "type": "source", "outputs": ["out"]},
                    {
                        "id": "sink",
                        "type": "sink",
                        "inputs": {"q": "source/out"},
                    },
                ],
            }
        )
        plan = plan_execution(flow)
        sink_step = [step for step in plan.steps if step.node_id == "sink"][0]
        self.assertEqual(sink_step.metadata.retry_policy.max_attempts, 1)

    def test_plan_execution_metadata_uses_node_retry(self):
        flow = parse_flow_dict(
            {
                "schema_version": 2,
                "nodes": [
                    {"id": "source", "type": "source", "outputs": ["out"]},
                    {
                        "id": "worker",
                        "type": "agent",
                        "build": "pip install -e ./worker",
                        "path": "dynamic",
                        "outputs": ["done"],
                        "inputs": {"q": "source/out"},
                        "retry": {"max_attempts": 3, "initial_delay_seconds": 0.2},
                    },
                ],
            }
        )
        plan = plan_execution(flow)
        worker_step = [step for step in plan.steps if step.node_id == "worker"][0]
        self.assertEqual(worker_step.metadata.retry_policy.max_attempts, 3)
        self.assertEqual(worker_step.metadata.retry_schedule_seconds, (0.2, 0.4))

    def test_plan_execution_metadata_hooks_from_extras(self):
        flow = parse_flow_dict(
            {
                "schema_version": 2,
                "nodes": [
                    {"id": "source", "type": "source", "outputs": ["out"]},
                    {
                        "id": "worker",
                        "type": "agent",
                        "build": "pip install -e ./worker",
                        "path": "dynamic",
                        "outputs": ["done"],
                        "inputs": {"q": "source/out"},
                        "planning": {"lane": "gpu"},
                    },
                ],
            }
        )
        plan = plan_execution(flow)
        worker_step = [step for step in plan.steps if step.node_id == "worker"][0]
        self.assertEqual(worker_step.metadata.hooks["lane"], "gpu")


if __name__ == "__main__":
    unittest.main()
