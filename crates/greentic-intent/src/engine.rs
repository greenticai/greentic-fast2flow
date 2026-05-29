//! The top-level `IntentEngine` + builder. Drives the pipeline:
//! tokenize → extract (per registered extractor) → resolve overlaps →
//! render markers.

use std::sync::Arc;
use std::time::Instant;

use crate::context::IntentContext;
use crate::entity::Entity;
use crate::extractors::{EntityCandidate, EntityExtractor};
use crate::language::ResolvedLanguage;
use crate::locale::ResolvedLocale;
use crate::marker::{render_markers, IntentLatency, MarkResult};
use crate::resources::IntentResources;
use crate::tokenizer::{Tokenizer, WhitespaceTokenizer};

/// Builder for [`IntentEngine`].
#[derive(Default)]
pub struct IntentEngineBuilder {
    tokenizer: Option<Arc<dyn Tokenizer>>,
    extractors: Vec<Arc<dyn EntityExtractor>>,
    resources: IntentResources,
}

impl std::fmt::Debug for IntentEngineBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntentEngineBuilder")
            .field("extractor_count", &self.extractors.len())
            .field("has_custom_tokenizer", &self.tokenizer.is_some())
            .field("resource_locales", &self.resources.locales.len())
            .finish()
    }
}

impl IntentEngineBuilder {
    /// Use a specific tokenizer. Defaults to [`WhitespaceTokenizer`].
    pub fn with_tokenizer<T: Tokenizer + 'static>(mut self, tokenizer: T) -> Self {
        self.tokenizer = Some(Arc::new(tokenizer));
        self
    }

    /// Register an extractor. Order matters when overlap resolution falls
    /// through on confidence + span ties — earlier extractors win.
    pub fn with_extractor<E: EntityExtractor + 'static>(mut self, extractor: E) -> Self {
        self.extractors.push(Arc::new(extractor));
        self
    }

    /// Replace the entire resource bag (gazetteer, locale bundles).
    pub fn with_resources(mut self, resources: IntentResources) -> Self {
        self.resources = resources;
        self
    }

    /// Build the engine.
    pub fn build(self) -> IntentEngine {
        IntentEngine {
            tokenizer: self
                .tokenizer
                .unwrap_or_else(|| Arc::new(WhitespaceTokenizer)),
            extractors: self.extractors,
            resources: Arc::new(self.resources),
        }
    }
}

/// Deterministic multilingual marker engine. Cheap to clone — internals
/// are wrapped in `Arc`.
#[derive(Clone)]
pub struct IntentEngine {
    tokenizer: Arc<dyn Tokenizer>,
    extractors: Vec<Arc<dyn EntityExtractor>>,
    resources: Arc<IntentResources>,
}

impl std::fmt::Debug for IntentEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntentEngine")
            .field("extractor_count", &self.extractors.len())
            .field("resource_locales", &self.resources.locales.len())
            .finish()
    }
}

impl IntentEngine {
    /// New builder.
    pub fn builder() -> IntentEngineBuilder {
        IntentEngineBuilder::default()
    }

    /// Run the pipeline on `text` under `ctx`. Phase A pipes everything
    /// through — but with no extractors registered (or with stub
    /// extractors), returns the original text marked as-is with no
    /// entities. Real extraction lands per-extractor milestone.
    pub fn mark(&self, text: &str, ctx: &IntentContext) -> MarkResult {
        let total_started = Instant::now();

        let tok_started = Instant::now();
        let tokens = self.tokenizer.tokenize(text);
        let tokenize_ms = elapsed_ms(tok_started);

        // Language/locale resolution: Phase A always falls back to en-GB.
        // The language module fills this in with the real chain later.
        let language = pick_language(ctx);
        let locale = ResolvedLocale {
            locale: language.locale.clone(),
            language: language.language.clone(),
            script: language.script.clone(),
        };

        let extract_started = Instant::now();
        let mut all_candidates: Vec<EntityCandidate> = Vec::new();
        for extractor in &self.extractors {
            let candidates =
                extractor.extract(&tokens, text, ctx, &locale, self.resources.as_ref());
            all_candidates.extend(candidates);
        }
        let extract_ms = elapsed_ms(extract_started);

        let resolve_started = Instant::now();
        let kept = crate::resolver::resolve(all_candidates);
        let resolve_ms = elapsed_ms(resolve_started);

        let entities: Vec<Entity> = kept
            .into_iter()
            .enumerate()
            .map(|(idx, c)| Entity {
                id: format!("e{}", idx + 1),
                kind: c.kind,
                raw: c.raw,
                normalized: c.normalized,
                canonical: c.canonical,
                start: c.start,
                end: c.end,
                role: c.role,
                confidence: c.confidence,
                locale: c.locale,
                evidence: c.evidence,
            })
            .collect();

        let render_started = Instant::now();
        let mut result = render_markers(text, &entities);
        let render_ms = elapsed_ms(render_started);

        result.language = language;
        result.latency = IntentLatency {
            total_ms: elapsed_ms(total_started),
            tokenize_ms,
            extract_ms,
            resolve_ms,
            render_ms,
        };
        result
    }
}

fn pick_language(ctx: &IntentContext) -> ResolvedLanguage {
    if let Some(locale) = ctx
        .preferred_locale
        .as_deref()
        .or(ctx.user_locale.as_deref())
        .or(ctx.tenant_locale.as_deref())
    {
        let language = locale.split('-').next().unwrap_or("en").to_string();
        let script = match language.as_str() {
            "ar" | "fa" | "he" => "Arabic".to_string(),
            "zh" | "ja" => "Han".to_string(),
            _ => "Latin".to_string(),
        };
        let source = if ctx.preferred_locale.as_deref() == Some(locale) {
            crate::language::LanguageSource::Context
        } else if ctx.user_locale.as_deref() == Some(locale) {
            crate::language::LanguageSource::User
        } else {
            crate::language::LanguageSource::Tenant
        };
        ResolvedLanguage {
            language,
            locale: locale.to_string(),
            script,
            source,
            confidence: 1.0,
        }
    } else {
        ResolvedLanguage::fallback_en_gb()
    }
}

fn elapsed_ms(started: Instant) -> f32 {
    let dur = started.elapsed();
    (dur.as_secs_f64() * 1000.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_returns_unchanged_text_when_no_extractors_registered() {
        let engine = IntentEngine::builder().build();
        let ctx = IntentContext::now_utc("Europe/London");
        let result = engine.mark("what is the weather in London tomorrow?", &ctx);
        assert_eq!(
            result.marked_text,
            "what is the weather in London tomorrow?"
        );
        assert!(result.entities.is_empty());
        assert_eq!(result.language.locale, "en-GB");
    }

    #[test]
    fn preferred_locale_wins() {
        let engine = IntentEngine::builder().build();
        let mut ctx = IntentContext::now_utc("Europe/Paris");
        ctx.preferred_locale = Some("fr-FR".into());
        ctx.tenant_locale = Some("en-GB".into());
        let result = engine.mark("hello", &ctx);
        assert_eq!(result.language.locale, "fr-FR");
        assert_eq!(result.language.language, "fr");
    }
}
