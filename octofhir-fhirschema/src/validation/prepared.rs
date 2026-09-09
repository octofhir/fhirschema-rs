//! Per-resource constraint state. No formatted path/expression memo keys.
use super::compiled::{CompiledConstraint, ConstraintSeverity};
use octofhir_fhir_model::{
    ConstraintId, ExecutableExpression, FhirPathEvaluator, JsonVariables, NodeId, PreparedResource,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub(super) type PlanCache = moka::future::Cache<String, Arc<ExecutableExpression>>;
type Key = (NodeId, ConstraintId, u32);

pub(super) struct ConstraintRun {
    resource: PreparedResource,
    native: bool,
    preparation_error: Option<String>,
    results: HashMap<Key, bool>,
    types: HashMap<String, u32>,
    plans: HashMap<String, Result<Arc<ExecutableExpression>, String>>,
}

impl ConstraintRun {
    pub(super) async fn new(
        evaluator: &dyn FhirPathEvaluator,
        source: Arc<Value>,
        variables: &JsonVariables,
    ) -> Self {
        let (resource, native, preparation_error) =
            match evaluator.prepare_resource(source.clone(), variables).await {
                Ok(Some(resource)) => (resource, true, None),
                Ok(None) => (PreparedResource::new(source, Arc::new(())), false, None),
                Err(error) => (
                    PreparedResource::new(source, Arc::new(())),
                    false,
                    Some(error.to_string()),
                ),
            };
        Self {
            resource,
            native,
            preparation_error,
            results: HashMap::new(),
            types: HashMap::new(),
            plans: HashMap::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn evaluate(
        &mut self,
        evaluator: &dyn FhirPathEvaluator,
        cache: &PlanCache,
        data: &Value,
        typ: Option<&str>,
        variables: &JsonVariables,
        constraints: &[CompiledConstraint],
        arc_hint: Option<Arc<Value>>,
    ) -> Vec<Option<Result<bool, String>>> {
        let Some(node) = self.resource.node_id(data) else {
            return constraints
                .iter()
                .map(|c| {
                    (c.severity != ConstraintSeverity::Warning)
                        .then(|| Err("Constraint node is not in the prepared resource".into()))
                })
                .collect();
        };
        let type_id = match typ {
            None => 0,
            Some(name) => {
                let next = self.types.len() as u32 + 1;
                if let Some(id) = self.types.get(name) {
                    *id
                } else {
                    self.types.insert(name.to_string(), next);
                    next
                }
            }
        };
        let mut entries = Vec::with_capacity(constraints.len());
        let mut pending = Vec::new();
        let mut pending_keys = Vec::new();
        let mut seen = HashSet::new();
        for constraint in constraints {
            if constraint.severity == ConstraintSeverity::Warning {
                entries.push(None);
                continue;
            }
            if let Some(error) = &self.preparation_error {
                entries.push(Some(Err(error.clone())));
                continue;
            }
            let expression = constraint.expression.as_str();
            if !self.plans.contains_key(expression) {
                let plan = cache
                    .try_get_with_by_ref(expression, async {
                        evaluator.compile_handle(expression).await.map(Arc::new)
                    })
                    .await
                    .map_err(|error| error.to_string());
                self.plans.insert(expression.to_owned(), plan);
            }
            match &self.plans[expression] {
                Err(error) => entries.push(Some(Err(error.clone()))),
                Ok(plan) => {
                    let key = (node, plan.id(), type_id);
                    entries.push(Some(Ok(key)));
                    if !self.results.contains_key(&key) && seen.insert(key) {
                        pending.push(plan.clone());
                        pending_keys.push(key);
                    }
                }
            }
        }
        let mut errors = HashMap::new();
        if !pending.is_empty() {
            let evaluated = if self.native {
                evaluator
                    .evaluate_prepared_constraints(
                        &self.resource,
                        node,
                        data,
                        typ,
                        variables,
                        &pending,
                    )
                    .await
            } else {
                let expressions: Vec<_> = pending.iter().map(|p| p.expression()).collect();
                evaluator
                    .evaluate_constraints_shared_context_typed(
                        arc_hint.unwrap_or_else(|| Arc::new(data.clone())),
                        typ,
                        variables,
                        &expressions,
                    )
                    .await
            };
            match evaluated {
                Ok(values) if values.len() == pending_keys.len() => {
                    for (key, result) in pending_keys.into_iter().zip(values) {
                        match result {
                            Ok(satisfied) => {
                                self.results.insert(key, satisfied);
                            }
                            Err(error) => {
                                errors.insert(key, error.to_string());
                            }
                        }
                    }
                }
                result => {
                    let message = match result {
                        Err(error) => error.to_string(),
                        Ok(_) => "Evaluator returned an incorrect constraint result count".into(),
                    };
                    for key in pending_keys {
                        errors.insert(key, message.clone());
                    }
                }
            }
        }
        entries
            .into_iter()
            .map(|entry| {
                entry.map(|entry| match entry {
                    Err(error) => Err(error),
                    Ok(key) => self.results.get(&key).copied().ok_or_else(|| {
                        errors
                            .get(&key)
                            .cloned()
                            .unwrap_or_else(|| "Constraint evaluation failed".into())
                    }),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use octofhir_fhir_model::{
        CompiledExpression, EmptyModelProvider, EvaluationResult, FhirPathConstraint, ModelError,
        ModelProvider, Result as ModelResult, ValidationResult,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct LegacyEvaluator {
        compiled: AtomicUsize,
        evaluated: AtomicUsize,
        short_batch: bool,
    }

    #[async_trait]
    impl FhirPathEvaluator for LegacyEvaluator {
        async fn evaluate(&self, _: &str, _: Arc<Value>) -> ModelResult<EvaluationResult> {
            unreachable!("Validation must use the existing batch interface")
        }
        async fn evaluate_with_variables(
            &self,
            _: &str,
            _: Arc<Value>,
            _: &JsonVariables,
        ) -> ModelResult<EvaluationResult> {
            unreachable!("Validation must use the existing batch interface")
        }
        async fn compile(&self, expression: &str) -> ModelResult<CompiledExpression> {
            self.compiled.fetch_add(1, Ordering::Relaxed);
            tokio::task::yield_now().await;
            Ok(if expression == "invalid" {
                CompiledExpression::invalid(expression.into(), "Invalid syntax".into())
            } else {
                CompiledExpression::new(expression.into(), expression.into(), true)
            })
        }
        async fn validate_expression(&self, _: &str) -> ModelResult<ValidationResult> {
            unreachable!("The default handle API uses compile")
        }
        fn model_provider(&self) -> &dyn ModelProvider {
            &EmptyModelProvider
        }
        async fn validate_constraints(
            &self,
            _: Arc<Value>,
            _: &[FhirPathConstraint],
        ) -> ModelResult<ValidationResult> {
            unreachable!("Validation must use the existing batch interface")
        }
        async fn evaluate_constraints_shared_context_typed(
            &self,
            _: Arc<Value>,
            typ: Option<&str>,
            _: &JsonVariables,
            expressions: &[&str],
        ) -> ModelResult<Vec<ModelResult<bool>>> {
            self.evaluated.fetch_add(1, Ordering::Relaxed);
            Ok(if self.short_batch {
                Vec::new()
            } else {
                expressions
                    .iter()
                    .map(|expression| match *expression {
                        "error" => Err(ModelError::evaluation_error("Evaluation failed")),
                        "false" => Ok(false),
                        _ => Ok(typ != Some("date")),
                    })
                    .collect()
            })
        }
    }

    fn constraint(expression: &str) -> CompiledConstraint {
        CompiledConstraint {
            key: expression.into(),
            expression: expression.into(),
            human: expression.into(),
            severity: ConstraintSeverity::Error,
        }
    }

    #[tokio::test]
    async fn legacy_batch_keeps_duplicate_slots_and_memoizes_by_declared_type() {
        let evaluator = LegacyEvaluator::default();
        let cache = PlanCache::new(16);
        let source = Arc::new(serde_json::json!("2000-01-02"));
        let vars = JsonVariables::new();
        let mut run = ConstraintRun::new(&evaluator, source.clone(), &vars).await;
        let mut constraints: Vec<_> = [
            "ok", "ok", "false", "error", "error", "invalid", "invalid", "warning",
        ]
        .into_iter()
        .map(constraint)
        .collect();
        constraints.last_mut().unwrap().severity = ConstraintSeverity::Warning;
        for _ in 0..2 {
            let result = run
                .evaluate(
                    &evaluator,
                    &cache,
                    &source,
                    None,
                    &vars,
                    &constraints,
                    Some(source.clone()),
                )
                .await;
            assert_eq!(result.len(), 8);
            assert_eq!(result[0], Some(Ok(true)));
            assert_eq!(result[1], Some(Ok(true)));
            assert_eq!(result[2], Some(Ok(false)));
            for entry in &result[3..7] {
                assert!(matches!(entry, Some(Err(_))));
            }
            assert_eq!(result[3], result[4]);
            assert_eq!(result[5], result[6]);
            assert_eq!(result[7], None);
        }
        assert_eq!(evaluator.compiled.load(Ordering::Relaxed), 4);
        // Evaluation errors are retried, successful booleans are memoized.
        assert_eq!(evaluator.evaluated.load(Ordering::Relaxed), 2);
        let result = run
            .evaluate(
                &evaluator,
                &cache,
                &source,
                Some("date"),
                &vars,
                &[constraint("ok")],
                None,
            )
            .await;
        assert_eq!(result, vec![Some(Ok(false))]);
        assert_eq!(evaluator.evaluated.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn malformed_legacy_batch_is_reported_for_every_duplicate() {
        let evaluator = LegacyEvaluator {
            short_batch: true,
            ..Default::default()
        };
        let source = Arc::new(serde_json::json!({}));
        let vars = JsonVariables::new();
        let mut run = ConstraintRun::new(&evaluator, source.clone(), &vars).await;
        let result = run
            .evaluate(
                &evaluator,
                &PlanCache::new(16),
                &source,
                None,
                &vars,
                &[constraint("ok"), constraint("ok")],
                None,
            )
            .await;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], result[1]);
        assert!(matches!(&result[0], Some(Err(error)) if error.contains("result count")));
    }

    #[tokio::test]
    async fn concurrent_validations_share_one_handle_compilation() {
        let evaluator = Arc::new(LegacyEvaluator::default());
        let cache = PlanCache::new(16);
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let evaluator = evaluator.clone();
            let cache = cache.clone();
            tasks.push(tokio::spawn(async move {
                let source = Arc::new(serde_json::json!({}));
                let vars = JsonVariables::new();
                let mut run = ConstraintRun::new(evaluator.as_ref(), source.clone(), &vars).await;
                let result = run
                    .evaluate(
                        evaluator.as_ref(),
                        &cache,
                        &source,
                        None,
                        &vars,
                        &[constraint("ok")],
                        None,
                    )
                    .await;
                assert_eq!(result, vec![Some(Ok(true))]);
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(evaluator.compiled.load(Ordering::Relaxed), 1);
        assert_eq!(evaluator.evaluated.load(Ordering::Relaxed), 8);
    }
}
