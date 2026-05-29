use std::sync::Arc;
use std::time::{Duration, Instant};

use fast2flow_contracts::{Candidate, Fast2FlowHookInV1, Fast2FlowHookOutV1, RoutingDirective};
use fast2flow_hooks::{FilterDecision, HookFilter};
use fast2flow_llm::LlmProvider;
use fast2flow_strategy::RoutingStrategy;
use tracing::{debug, info, instrument, warn};

pub trait CandidateIndex: Send + Sync {
    fn search(&self, scope: &str, text: &str, limit: usize) -> Vec<Candidate>;
}

#[derive(Debug, Clone)]
pub struct RouterConfig {
    pub min_confidence: f32,
    pub llm_min_confidence: f32,
    pub candidate_limit: usize,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.5,
            llm_min_confidence: 0.5,
            candidate_limit: 20,
        }
    }
}

pub struct CoreRouter {
    strategy: Arc<dyn RoutingStrategy>,
    filters: Vec<Arc<dyn HookFilter>>,
    llm: Option<Arc<dyn LlmProvider>>,
    config: RouterConfig,
}

impl CoreRouter {
    pub fn new(
        strategy: Arc<dyn RoutingStrategy>,
        filters: Vec<Arc<dyn HookFilter>>,
        llm: Option<Arc<dyn LlmProvider>>,
        config: RouterConfig,
    ) -> Self {
        Self {
            strategy,
            filters,
            llm,
            config,
        }
    }

    #[instrument(
        name = "fast2flow.route",
        level = "info",
        skip_all,
        fields(
            scope = %request.scope,
            channel = request.envelope.channel.as_deref().unwrap_or(""),
            provider = request.envelope.provider.as_deref().unwrap_or(""),
            text_len = request.envelope.text.trim().len(),
            time_budget_ms = request.time_budget_ms,
            llm_enabled = self.llm.is_some(),
        )
    )]
    pub async fn route(
        &self,
        request: Fast2FlowHookInV1,
        index: &dyn CandidateIndex,
    ) -> Fast2FlowHookOutV1 {
        let started_at = Instant::now();
        let budget = Duration::from_millis(request.time_budget_ms);
        // Available at DEBUG only — this is the inbound user message.
        debug!(text = %request.envelope.text.trim(), "routing message");

        if budget.is_zero() {
            warn!(
                reason = "zero_time_budget",
                "routing → continue (caller passed a zero time budget)"
            );
            return continue_directive();
        }

        for (filter_index, filter) in self.filters.iter().enumerate() {
            match filter.evaluate(&request) {
                FilterDecision::Proceed => {}
                FilterDecision::Continue => {
                    debug!(filter_index, "hook filter → continue");
                    return continue_directive();
                }
                FilterDecision::Respond(message) => {
                    info!(filter_index, "routing → respond (hook filter rule)");
                    return Fast2FlowHookOutV1 {
                        directive: RoutingDirective::Respond { message },
                    };
                }
                FilterDecision::Deny(reason) => {
                    info!(filter_index, %reason, "routing → deny (hook filter rule)");
                    return Fast2FlowHookOutV1 {
                        directive: RoutingDirective::Deny { reason },
                    };
                }
            }
            if exceeded_budget(started_at, budget) {
                warn!(
                    reason = "budget_exceeded_in_filters",
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    budget_ms = budget.as_millis() as u64,
                    "routing → continue (time budget exhausted while running hook filters)"
                );
                return continue_directive();
            }
        }

        let text = request.envelope.text.trim();
        if text.is_empty() {
            debug!(reason = "empty_text", "routing → continue");
            return continue_directive();
        }

        let candidates = index.search(&request.scope, text, self.config.candidate_limit);
        debug!(candidate_count = candidates.len(), "index search complete");
        if let Some(decision) = self.strategy.evaluate(text, &candidates) {
            if decision.confidence >= self.config.min_confidence {
                info!(
                    target = %decision.target,
                    confidence = decision.confidence,
                    reason = %decision.reason,
                    source = "deterministic",
                    "routing → dispatch"
                );
                return Fast2FlowHookOutV1 {
                    directive: RoutingDirective::Dispatch {
                        target: decision.target,
                        confidence: decision.confidence,
                        reason: decision.reason,
                        entities: Vec::new(),
                    },
                };
            }
            info!(
                target = %decision.target,
                confidence = decision.confidence,
                min_confidence = self.config.min_confidence,
                source = "deterministic",
                "best match is below the confidence threshold"
            );
        } else {
            debug!(
                candidate_count = candidates.len(),
                "strategy produced no candidate"
            );
        }

        if let Some(llm) = &self.llm {
            let elapsed = started_at.elapsed();
            if elapsed >= budget {
                warn!(
                    reason = "budget_exhausted_before_llm",
                    elapsed_ms = elapsed.as_millis() as u64,
                    budget_ms = budget.as_millis() as u64,
                    "routing → continue (time budget exhausted before LLM fallback)"
                );
                return continue_directive();
            }
            let remaining = budget - elapsed;
            let prompt = llm_prompt(&request.scope, text, &candidates);
            debug!(
                remaining_ms = remaining.as_millis() as u64,
                "invoking LLM fallback"
            );
            match llm.complete(&prompt, remaining).await {
                Ok(answer) => {
                    if answer.confidence >= self.config.llm_min_confidence {
                        info!(
                            target = %answer.target,
                            confidence = answer.confidence,
                            reason = %answer.reason,
                            source = "llm",
                            "routing → dispatch"
                        );
                        return Fast2FlowHookOutV1 {
                            directive: RoutingDirective::Dispatch {
                                target: answer.target,
                                confidence: answer.confidence,
                                reason: answer.reason,
                                entities: Vec::new(),
                            },
                        };
                    }
                    info!(
                        target = %answer.target,
                        confidence = answer.confidence,
                        llm_min_confidence = self.config.llm_min_confidence,
                        source = "llm",
                        "LLM match is below the confidence threshold"
                    );
                }
                Err(err) => {
                    warn!(error = %err, "LLM fallback failed");
                }
            }
        }

        info!(
            reason = "no_match",
            candidate_count = candidates.len(),
            "routing → continue (no flow matched the message)"
        );
        continue_directive()
    }
}

fn llm_prompt(scope: &str, text: &str, candidates: &[Candidate]) -> String {
    let shortlist = candidates
        .iter()
        .take(5)
        .map(|candidate| format!("{}:{}", candidate.target, candidate.title))
        .collect::<Vec<String>>()
        .join("; ");

    format!(
        "scope={scope}; input={text}; candidates={shortlist}; return json {{target, confidence, reason}}"
    )
}

fn exceeded_budget(started_at: Instant, budget: Duration) -> bool {
    started_at.elapsed() >= budget
}

fn continue_directive() -> Fast2FlowHookOutV1 {
    Fast2FlowHookOutV1 {
        directive: RoutingDirective::Continue,
    }
}
