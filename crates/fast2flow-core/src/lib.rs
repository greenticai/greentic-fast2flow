use std::sync::Arc;
use std::time::{Duration, Instant};

use fast2flow_contracts::{Candidate, Fast2FlowHookInV1, Fast2FlowHookOutV1, RoutingDirective};
use fast2flow_hooks::{FilterDecision, HookFilter};
use fast2flow_llm::LlmProvider;
use fast2flow_strategy::RoutingStrategy;

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

    pub async fn route(
        &self,
        request: Fast2FlowHookInV1,
        index: &dyn CandidateIndex,
    ) -> Fast2FlowHookOutV1 {
        let started_at = Instant::now();
        let budget = Duration::from_millis(request.time_budget_ms);

        if budget.is_zero() {
            return continue_directive();
        }

        for filter in &self.filters {
            match filter.evaluate(&request) {
                FilterDecision::Proceed => {}
                FilterDecision::Continue => return continue_directive(),
                FilterDecision::Respond(message) => {
                    return Fast2FlowHookOutV1 {
                        directive: RoutingDirective::Respond { message },
                    }
                }
                FilterDecision::Deny(reason) => {
                    return Fast2FlowHookOutV1 {
                        directive: RoutingDirective::Deny { reason },
                    }
                }
            }
            if exceeded_budget(started_at, budget) {
                return continue_directive();
            }
        }

        let text = request.envelope.text.trim();
        if text.is_empty() {
            return continue_directive();
        }

        let candidates = index.search(&request.scope, text, self.config.candidate_limit);
        if let Some(decision) = self.strategy.evaluate(text, &candidates) {
            if decision.confidence >= self.config.min_confidence {
                return Fast2FlowHookOutV1 {
                    directive: RoutingDirective::Dispatch {
                        target: decision.target,
                        confidence: decision.confidence,
                        reason: decision.reason,
                    },
                };
            }
        }

        if let Some(llm) = &self.llm {
            let elapsed = started_at.elapsed();
            if elapsed >= budget {
                return continue_directive();
            }
            let remaining = budget - elapsed;
            let prompt = llm_prompt(&request.scope, text, &candidates);
            if let Ok(answer) = llm.complete(&prompt, remaining).await {
                if answer.confidence >= self.config.llm_min_confidence {
                    return Fast2FlowHookOutV1 {
                        directive: RoutingDirective::Dispatch {
                            target: answer.target,
                            confidence: answer.confidence,
                            reason: answer.reason,
                        },
                    };
                }
            }
        }

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
