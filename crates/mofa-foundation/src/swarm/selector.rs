//! Automatic coordination-pattern selection from DAG topology.
//!
//! [`PatternSelector`] inspects a [`SubtaskDAG`]'s structure and metadata and
//! returns the most appropriate [`CoordinationPattern`] without any LLM call.
//! The selection is deterministic and runs in O(n) time.

use serde::{Deserialize, Serialize};

use crate::swarm::{CoordinationPattern, RiskLevel, SubtaskDAG};

/// Result of automatic pattern selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternSelection {
    /// The recommended coordination pattern.
    pub pattern: CoordinationPattern,
    /// Confidence in the recommendation, in the range `[0.0, 1.0]`.
    pub confidence: f64,
    /// Human-readable explanation of why this pattern was chosen.
    pub reason: String,
}

/// Result of validating a manually chosen [`CoordinationPattern`] against a [`SubtaskDAG`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationResult {
    Valid,
    Suboptimal { reason: String, suggested: CoordinationPattern },
    Invalid { reason: String },
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid | Self::Suboptimal { .. })
    }
}

/// Inspects a [`SubtaskDAG`]'s topology and task metadata to recommend the
/// best [`CoordinationPattern`].
///
/// Rules are applied in priority order; the first matching rule wins:
///
/// | Priority | Pattern      | Trigger condition                                              |
/// |----------|-------------|----------------------------------------------------------------|
/// | 1        | Routing      | Single source node + ≥1 sink with `required_capabilities` set |
/// | 2        | Supervision  | Any task has `risk_level ≥ High` or `hitl_required`           |
/// | 3        | Debate       | Exactly 2 sources → exactly 1 sink                            |
/// | 4        | Consensus    | ≥3 sources with identical capabilities → exactly 1 sink       |
/// | 5        | MapReduce    | ≥2 heterogeneous sources → exactly 1 sink                     |
/// | 6        | Sequential   | Every node has at most 1 predecessor and 1 successor           |
/// | 7        | Parallel     | Fallback for all other shapes                                  |
pub struct PatternSelector;

impl PatternSelector {
    /// Returns the recommended [`CoordinationPattern`].
    ///
    /// Convenience wrapper around [`select_with_reason`].
    pub fn select(dag: &SubtaskDAG) -> CoordinationPattern {
        Self::select_with_reason(dag).pattern
    }

    /// Returns the recommended pattern together with a confidence score and
    /// a human-readable reason.
    pub fn select_with_reason(dag: &SubtaskDAG) -> PatternSelection {
        if dag.task_count() == 0 {
            return PatternSelection {
                pattern: CoordinationPattern::Sequential,
                confidence: 1.0,
                reason: "empty dag — sequential is the no-op default".into(),
            };
        }

        let all = dag.all_tasks();

        let source_indices: Vec<_> = all
            .iter()
            .filter(|(idx, _)| dag.dependencies_of(*idx).is_empty())
            .map(|(idx, _)| *idx)
            .collect();

        let sink_indices: Vec<_> = all
            .iter()
            .filter(|(idx, _)| dag.dependents_of(*idx).is_empty())
            .map(|(idx, _)| *idx)
            .collect();

        // 1. Routing — single source + at least one sink declares required_capabilities
        if source_indices.len() == 1 {
            let specialist_count = sink_indices
                .iter()
                .filter(|&&idx| {
                    dag.get_task(idx)
                        .map(|t| !t.required_capabilities.is_empty())
                        .unwrap_or(false)
                })
                .count();

            if specialist_count > 0 {
                return PatternSelection {
                    pattern: CoordinationPattern::Routing,
                    confidence: 0.95,
                    reason: format!(
                        "single router source dispatches to {specialist_count} specialist sink(s) with required_capabilities"
                    ),
                };
            }
        }

        // 2. Supervision — any task carries high/critical risk or requires HITL
        let has_oversight_need = all
            .iter()
            .any(|(_, t)| t.hitl_required || t.risk_level >= RiskLevel::High);

        if has_oversight_need {
            return PatternSelection {
                pattern: CoordinationPattern::Supervision,
                confidence: 0.90,
                reason: "one or more tasks carry high/critical risk or require human-in-the-loop \
                         oversight — a supervisor node should always run last"
                    .into(),
            };
        }

        // Rules 3–5 apply only to the many-to-one funnel shape (≥2 sources, 1 sink)
        if source_indices.len() >= 2 && sink_indices.len() == 1 {
            // 3. Debate — exactly 2 sources (binary argument → judge)
            if source_indices.len() == 2 {
                return PatternSelection {
                    pattern: CoordinationPattern::Debate,
                    confidence: 0.85,
                    reason: "exactly 2 source nodes converge on 1 sink — \
                             binary debate with a deciding judge"
                        .into(),
                };
            }

            // 4. Consensus — ≥3 sources that all share the same required_capabilities
            //    (they are equivalent "voters")
            let mut source_caps: Vec<Vec<String>> = source_indices
                .iter()
                .map(|&idx| {
                    let mut caps = dag
                        .get_task(idx)
                        .map(|t| t.required_capabilities.clone())
                        .unwrap_or_default();
                    caps.sort();
                    caps
                })
                .collect();
            source_caps.dedup();

            if source_caps.len() == 1 && !source_caps[0].is_empty() {
                return PatternSelection {
                    pattern: CoordinationPattern::Consensus,
                    confidence: 0.80,
                    reason: format!(
                        "{} equivalent voter sources (identical capabilities) converge on 1 aggregator",
                        source_indices.len()
                    ),
                };
            }

            // 5. MapReduce — ≥2 heterogeneous sources → 1 sink (general fan-in)
            return PatternSelection {
                pattern: CoordinationPattern::MapReduce,
                confidence: 0.75,
                reason: format!(
                    "{} mapper sources fan-in to 1 reducer sink",
                    source_indices.len()
                ),
            };
        }

        // 6. Sequential — strict linear chain: exactly 1 source, exactly 1 sink,
        //    and every node has at most 1 predecessor and 1 successor.
        let is_linear = source_indices.len() == 1
            && sink_indices.len() == 1
            && all.iter().all(|(idx, _)| {
                dag.dependencies_of(*idx).len() <= 1 && dag.dependents_of(*idx).len() <= 1
            });

        if is_linear && dag.task_count() > 1 {
            return PatternSelection {
                pattern: CoordinationPattern::Sequential,
                confidence: 0.90,
                reason: "all nodes form a strict linear chain with at most one \
                         predecessor and one successor each"
                    .into(),
            };
        }

        // 7. Parallel — safe fallback for any other shape
        PatternSelection {
            pattern: CoordinationPattern::Parallel,
            confidence: 0.60,
            reason: "dag topology does not match any specialised pattern; \
                     parallel execution is the safe default"
                .into(),
        }
    }

    /// Checks whether a manually chosen `pattern` is appropriate for `dag`.
    ///
    /// Returns:
    /// - [`ValidationResult::Valid`] — the pattern fits the DAG topology.
    /// - [`ValidationResult::Suboptimal`] — the pattern works but a better fit exists.
    /// - [`ValidationResult::Invalid`] — the pattern's structural requirements are not met.
    pub fn validate(dag: &SubtaskDAG, pattern: CoordinationPattern) -> ValidationResult {
        if dag.task_count() == 0 {
            return ValidationResult::Valid;
        }

        let all = dag.all_tasks();

        let source_count = all
            .iter()
            .filter(|(idx, _)| dag.dependencies_of(*idx).is_empty())
            .count();

        let sink_indices: Vec<_> = all
            .iter()
            .filter(|(idx, _)| dag.dependents_of(*idx).is_empty())
            .map(|(idx, _)| *idx)
            .collect();
        let sink_count = sink_indices.len();

        let is_linear = source_count == 1
            && sink_count == 1
            && all.iter().all(|(idx, _)| {
                dag.dependencies_of(*idx).len() <= 1 && dag.dependents_of(*idx).len() <= 1
            });

        let has_oversight_need = all
            .iter()
            .any(|(_, t)| t.hitl_required || t.risk_level >= RiskLevel::High);

        let specialist_count = sink_indices
            .iter()
            .filter(|&&idx| {
                dag.get_task(idx)
                    .map(|t| !t.required_capabilities.is_empty())
                    .unwrap_or(false)
            })
            .count();

        match pattern {
            CoordinationPattern::Sequential => {
                if !is_linear {
                    return ValidationResult::Invalid {
                        reason: format!(
                            "sequential requires a strict linear chain; dag has \
                             {source_count} source(s) and {sink_count} sink(s)"
                        ),
                    };
                }
                if has_oversight_need {
                    return ValidationResult::Suboptimal {
                        reason: "dag contains high-risk tasks; Supervision ensures a \
                                 supervisor always runs even if earlier steps fail"
                            .into(),
                        suggested: CoordinationPattern::Supervision,
                    };
                }
                ValidationResult::Valid
            }

            CoordinationPattern::Parallel => {
                if is_linear && dag.task_count() > 1 {
                    return ValidationResult::Suboptimal {
                        reason: "dag is a strict linear chain; Sequential preserves \
                                 output ordering between dependent steps"
                            .into(),
                        suggested: CoordinationPattern::Sequential,
                    };
                }
                if has_oversight_need {
                    return ValidationResult::Suboptimal {
                        reason: "dag contains high-risk tasks; Supervision ensures a \
                                 supervisor always runs even if workers fail"
                            .into(),
                        suggested: CoordinationPattern::Supervision,
                    };
                }
                ValidationResult::Valid
            }

            CoordinationPattern::MapReduce => {
                if source_count < 2 {
                    return ValidationResult::Invalid {
                        reason: format!(
                            "mapreduce requires ≥2 source (mapper) nodes; found {source_count}"
                        ),
                    };
                }
                if sink_count != 1 {
                    return ValidationResult::Invalid {
                        reason: format!(
                            "mapreduce requires exactly 1 sink (reducer) node; found {sink_count}"
                        ),
                    };
                }
                ValidationResult::Valid
            }

            CoordinationPattern::Debate => {
                if source_count != 2 {
                    return ValidationResult::Invalid {
                        reason: format!(
                            "debate requires exactly 2 source (debater) nodes; found {source_count}"
                        ),
                    };
                }
                if sink_count != 1 {
                    return ValidationResult::Invalid {
                        reason: format!(
                            "debate requires exactly 1 sink (judge) node; found {sink_count}"
                        ),
                    };
                }
                ValidationResult::Valid
            }

            CoordinationPattern::Consensus => {
                if source_count < 3 {
                    return ValidationResult::Invalid {
                        reason: format!(
                            "consensus requires ≥3 source (voter) nodes for a meaningful \
                             majority; found {source_count}"
                        ),
                    };
                }
                if sink_count != 1 {
                    return ValidationResult::Invalid {
                        reason: format!(
                            "consensus requires exactly 1 sink (aggregator) node; \
                             found {sink_count}"
                        ),
                    };
                }
                ValidationResult::Valid
            }

            CoordinationPattern::Routing => {
                if source_count != 1 {
                    return ValidationResult::Invalid {
                        reason: format!(
                            "routing requires exactly 1 source (router) node; \
                             found {source_count}"
                        ),
                    };
                }
                if specialist_count == 0 {
                    return ValidationResult::Suboptimal {
                        reason: "no sink nodes declare required_capabilities; the router has \
                                 no capability signal to match against — consider MapReduce \
                                 or Parallel"
                            .into(),
                        suggested: Self::select(dag),
                    };
                }
                ValidationResult::Valid
            }

            CoordinationPattern::Supervision => {
                if source_count < 1 || sink_count != 1 {
                    return ValidationResult::Invalid {
                        reason: format!(
                            "supervision requires ≥1 source (worker) and exactly 1 sink \
                             (supervisor); found {source_count} source(s) and {sink_count} sink(s)"
                        ),
                    };
                }
                if !has_oversight_need {
                    return ValidationResult::Suboptimal {
                        reason: "no high-risk tasks detected; a lighter pattern may suffice"
                            .into(),
                        suggested: Self::select(dag),
                    };
                }
                ValidationResult::Valid
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::{RiskLevel, SubtaskDAG, SwarmSubtask};

    fn task(id: &str) -> SwarmSubtask {
        SwarmSubtask::new(id, id)
    }

    #[test]
    fn test_select_routing() {
        let mut dag = SubtaskDAG::new("t");
        let router = dag.add_task(task("router"));
        let mut spec_a = task("billing-agent");
        spec_a.required_capabilities = vec!["billing".into()];
        let mut spec_b = task("tech-agent");
        spec_b.required_capabilities = vec!["technical".into()];
        let a = dag.add_task(spec_a);
        let b = dag.add_task(spec_b);
        dag.add_dependency(router, a).unwrap();
        dag.add_dependency(router, b).unwrap();

        let sel = PatternSelector::select_with_reason(&dag);
        assert_eq!(sel.pattern, CoordinationPattern::Routing);
        assert!(sel.confidence >= 0.90);
    }

    #[test]
    fn test_select_supervision() {
        let mut dag = SubtaskDAG::new("t");
        let mut worker = task("deploy");
        worker.risk_level = RiskLevel::High;
        let w = dag.add_task(worker);
        let supervisor = dag.add_task(task("supervisor"));
        dag.add_dependency(w, supervisor).unwrap();

        assert_eq!(PatternSelector::select(&dag), CoordinationPattern::Supervision);
    }

    #[test]
    fn test_select_debate() {
        let mut dag = SubtaskDAG::new("t");
        let a = dag.add_task(task("advocate-a"));
        let b = dag.add_task(task("advocate-b"));
        let judge = dag.add_task(task("judge"));
        dag.add_dependency(a, judge).unwrap();
        dag.add_dependency(b, judge).unwrap();

        assert_eq!(PatternSelector::select(&dag), CoordinationPattern::Debate);
    }

    #[test]
    fn test_select_consensus() {
        let mut dag = SubtaskDAG::new("t");
        let caps = vec!["classifier".into()];
        let make_voter = |id: &str| {
            let mut t = task(id);
            t.required_capabilities = caps.clone();
            t
        };
        let v1 = dag.add_task(make_voter("voter-1"));
        let v2 = dag.add_task(make_voter("voter-2"));
        let v3 = dag.add_task(make_voter("voter-3"));
        let agg = dag.add_task(task("aggregator"));
        dag.add_dependency(v1, agg).unwrap();
        dag.add_dependency(v2, agg).unwrap();
        dag.add_dependency(v3, agg).unwrap();

        assert_eq!(PatternSelector::select(&dag), CoordinationPattern::Consensus);
    }

    #[test]
    fn test_select_mapreduce() {
        let mut dag = SubtaskDAG::new("t");
        let m1 = dag.add_task(task("mapper-1"));
        let m2 = dag.add_task(task("mapper-2"));
        let m3 = dag.add_task(task("mapper-3"));
        let r = dag.add_task(task("reducer"));
        dag.add_dependency(m1, r).unwrap();
        dag.add_dependency(m2, r).unwrap();
        dag.add_dependency(m3, r).unwrap();

        assert_eq!(PatternSelector::select(&dag), CoordinationPattern::MapReduce);
    }

    #[test]
    fn test_select_sequential() {
        let mut dag = SubtaskDAG::new("t");
        let a = dag.add_task(task("a"));
        let b = dag.add_task(task("b"));
        let c = dag.add_task(task("c"));
        dag.add_dependency(a, b).unwrap();
        dag.add_dependency(b, c).unwrap();

        assert_eq!(PatternSelector::select(&dag), CoordinationPattern::Sequential);
    }

    #[test]
    fn test_select_parallel_fallback() {
        let mut dag = SubtaskDAG::new("t");
        dag.add_task(task("worker-1"));
        dag.add_task(task("worker-2"));
        dag.add_task(task("worker-3"));

        assert_eq!(PatternSelector::select(&dag), CoordinationPattern::Parallel);
    }

    #[test]
    fn test_select_empty_dag_is_sequential() {
        let dag = SubtaskDAG::new("empty");
        let sel = PatternSelector::select_with_reason(&dag);
        assert_eq!(sel.pattern, CoordinationPattern::Sequential);
        assert_eq!(sel.confidence, 1.0);
    }

    #[test]
    fn test_validate_debate_wrong_source_count_is_invalid() {
        let mut dag = SubtaskDAG::new("t");
        let a = dag.add_task(task("a"));
        let b = dag.add_task(task("b"));
        let c = dag.add_task(task("c"));
        let judge = dag.add_task(task("judge"));
        dag.add_dependency(a, judge).unwrap();
        dag.add_dependency(b, judge).unwrap();
        dag.add_dependency(c, judge).unwrap();

        let result = PatternSelector::validate(&dag, CoordinationPattern::Debate);
        assert!(matches!(result, ValidationResult::Invalid { .. }));
    }

    #[test]
    fn test_validate_sequential_on_fan_in_dag_is_invalid() {
        let mut dag = SubtaskDAG::new("t");
        let m1 = dag.add_task(task("mapper-1"));
        let m2 = dag.add_task(task("mapper-2"));
        let r = dag.add_task(task("reducer"));
        dag.add_dependency(m1, r).unwrap();
        dag.add_dependency(m2, r).unwrap();

        let result = PatternSelector::validate(&dag, CoordinationPattern::Sequential);
        assert!(matches!(result, ValidationResult::Invalid { .. }));
    }

    #[test]
    fn test_validate_supervision_without_high_risk_is_suboptimal() {
        let mut dag = SubtaskDAG::new("t");
        let w1 = dag.add_task(task("worker-1"));
        let w2 = dag.add_task(task("worker-2"));
        let sup = dag.add_task(task("supervisor"));
        dag.add_dependency(w1, sup).unwrap();
        dag.add_dependency(w2, sup).unwrap();

        let result = PatternSelector::validate(&dag, CoordinationPattern::Supervision);
        assert!(matches!(result, ValidationResult::Suboptimal { .. }));
        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_returns_valid_for_correct_pattern() {
        let mut dag = SubtaskDAG::new("t");
        let m1 = dag.add_task(task("mapper-1"));
        let m2 = dag.add_task(task("mapper-2"));
        let m3 = dag.add_task(task("mapper-3"));
        let r = dag.add_task(task("reducer"));
        dag.add_dependency(m1, r).unwrap();
        dag.add_dependency(m2, r).unwrap();
        dag.add_dependency(m3, r).unwrap();

        assert_eq!(
            PatternSelector::validate(&dag, CoordinationPattern::MapReduce),
            ValidationResult::Valid
        );
    }
}
