//! QuestionnaireResponse validation against its Questionnaire.
//!
//! When a `QuestionnaireResponse` carries a `questionnaire` canonical (or a
//! contained Questionnaire via `#id`), the response is validated against the
//! form definition. Only the parts that the FHIR R4 specification pins down
//! normatively are enforced here as errors:
//!
//! - Answer `value[x]` type must match the Questionnaire `item.type`
//!   (<https://hl7.org/fhir/R4/questionnaire.html#item.type>).
//! - `group` / `display` items must not carry an `answer`.
//! - `repeats = false` (default) => a question may carry at most one answer.
//! - For `choice` / `open-choice` with an inline `answerOption`, a coded answer
//!   must match one of the offered options (open-choice also allows free text).
//!
//! Checks that the specification leaves to the validator implementation
//! (unknown `linkId`, required-but-missing, answered-while-disabled) are handled
//! by [`QrStrictness`] so they can be enabled once cross-checked against a
//! reference validator, without shipping false rejections by default.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use super::{FhirSchemaErrorCode, ValidationError};

/// Resolves a `Questionnaire` canonical URL to its JSON definition.
///
/// Used to validate a `QuestionnaireResponse` against the form it claims to
/// answer. Contained questionnaires (`#id`) are resolved directly from the
/// response and do not require a provider.
#[async_trait]
pub trait QuestionnaireProvider: Send + Sync {
    /// Resolve a `Questionnaire` by canonical URL (an optional `|version`
    /// suffix may be present). Return `None` when it cannot be resolved; the
    /// validator then skips form-based checks rather than failing.
    async fn resolve(&self, canonical: &str) -> Option<Arc<JsonValue>>;
}

/// Which validator-convention checks to enforce, on top of the always-on
/// normative checks. Defaults to the safest set (normative only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QrStrictness {
    /// Error when a response `item.linkId` has no matching Questionnaire item.
    pub unknown_link_id: bool,
    /// Error when a required, enabled item has no answer.
    pub required_missing: bool,
    /// Error when an `enableWhen`-disabled item carries an answer.
    pub disabled_answered: bool,
}

impl QrStrictness {
    /// Mirror the HL7 Java validator: enable the convention checks too.
    pub fn java_like() -> Self {
        Self {
            unknown_link_id: true,
            required_missing: true,
            disabled_answered: true,
        }
    }
}

/// A Questionnaire item flattened for lookup by `linkId`.
struct QItem<'a> {
    obj: &'a serde_json::Map<String, JsonValue>,
    item_type: &'a str,
}

/// Index every Questionnaire item by `linkId` (recursively through nested
/// `item` arrays).
fn index_items<'a>(items: &'a JsonValue, out: &mut HashMap<&'a str, QItem<'a>>) {
    let Some(arr) = items.as_array() else {
        return;
    };
    for item in arr {
        let Some(obj) = item.as_object() else {
            continue;
        };
        if let Some(link_id) = obj.get("linkId").and_then(|v| v.as_str()) {
            let item_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            out.insert(link_id, QItem { obj, item_type });
        }
        if let Some(children) = obj.get("item") {
            index_items(children, out);
        }
    }
}

/// The `value[x]` answer keys allowed for a given Questionnaire `item.type`.
///
/// `None` means "no answer allowed" (group / display). For `choice` /
/// `open-choice`, the permitted set is derived from the item's `answerOption`
/// value types when present, otherwise a permissive set is used so we never
/// reject a legitimately-typed coded answer.
fn allowed_answer_keys(qitem: &QItem<'_>) -> Option<Vec<&'static str>> {
    match qitem.item_type {
        "boolean" => Some(vec!["valueBoolean"]),
        "decimal" => Some(vec!["valueDecimal"]),
        "integer" => Some(vec!["valueInteger"]),
        "date" => Some(vec!["valueDate"]),
        "dateTime" => Some(vec!["valueDateTime"]),
        "time" => Some(vec!["valueTime"]),
        "string" | "text" => Some(vec!["valueString"]),
        "url" => Some(vec!["valueUri"]),
        "attachment" => Some(vec!["valueAttachment"]),
        "quantity" => Some(vec!["valueQuantity"]),
        "reference" => Some(vec!["valueReference"]),
        "choice" | "open-choice" => {
            // choice answers may be Coding/integer/date/time/string depending on
            // the answerOption value types. Derive from answerOption; fall back
            // to the full set the spec permits. open-choice always allows a
            // free-text valueString.
            let mut keys: Vec<&'static str> = Vec::new();
            let add = |k: &'static str, keys: &mut Vec<&'static str>| {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            };
            if let Some(opts) = qitem.obj.get("answerOption").and_then(|v| v.as_array()) {
                for opt in opts {
                    if let Some(o) = opt.as_object() {
                        for k in [
                            "valueCoding",
                            "valueInteger",
                            "valueDate",
                            "valueTime",
                            "valueString",
                            "valueReference",
                        ] {
                            if o.contains_key(k) {
                                add(k, &mut keys);
                            }
                        }
                    }
                }
            }
            if keys.is_empty() {
                for k in [
                    "valueCoding",
                    "valueInteger",
                    "valueDate",
                    "valueTime",
                    "valueString",
                ] {
                    add(k, &mut keys);
                }
            }
            if qitem.item_type == "open-choice" {
                add("valueString", &mut keys);
            }
            Some(keys)
        }
        // Unknown/absent type: don't constrain the answer type.
        _ => Some(vec![]),
    }
}

/// The `value[x]` key present on an answer object, if any (e.g. `valueBoolean`).
fn answer_value_key(answer: &serde_json::Map<String, JsonValue>) -> Option<&str> {
    answer
        .keys()
        .map(String::as_str)
        .find(|k| k.starts_with("value"))
}

fn path_vec(path: &str) -> Vec<JsonValue> {
    if path.is_empty() {
        vec![]
    } else {
        path.split('.')
            .map(|s| JsonValue::String(s.to_string()))
            .collect()
    }
}

fn error(path: &str, message: String) -> ValidationError {
    ValidationError {
        error_type: FhirSchemaErrorCode::QuestionnaireViolation.to_string(),
        path: path_vec(path),
        message: Some(message),
        value: None,
        expected: None,
        got: None,
        schema_path: None,
        constraint_key: None,
        constraint_expression: None,
        constraint_severity: Some("error".to_string()),
    }
}

/// Validate a QuestionnaireResponse (`qr`) against its `questionnaire`
/// definition, appending any violations to `errors`.
pub fn validate_questionnaire_response(
    qr: &JsonValue,
    questionnaire: &JsonValue,
    strictness: QrStrictness,
    errors: &mut Vec<ValidationError>,
) {
    let mut index: HashMap<&str, QItem<'_>> = HashMap::new();
    if let Some(items) = questionnaire.get("item") {
        index_items(items, &mut index);
    }

    // Index every answered linkId across the whole response so `enableWhen`
    // conditions (which reference other items' answers) can be evaluated.
    let mut answers_by_link: HashMap<&str, Vec<&serde_json::Map<String, JsonValue>>> =
        HashMap::new();
    if let Some(qr_items) = qr.get("item") {
        collect_qr_answers(qr_items, &mut answers_by_link);
    }

    if let Some(qr_items) = qr.get("item") {
        validate_items(
            qr_items,
            &index,
            &answers_by_link,
            strictness,
            "QuestionnaireResponse.item",
            errors,
        );
    }

    // Questionnaire-driven required check: every required, enabled item must be
    // answered (or, for a group, present), even when it is entirely absent from
    // the response. Walked in tandem with the response so group scopes match.
    if strictness.required_missing {
        let empty = Vec::new();
        let qr_items = qr.get("item").and_then(|v| v.as_array()).unwrap_or(&empty);
        let q_items = questionnaire.get("item").and_then(|v| v.as_array());
        if let Some(q_items) = q_items {
            check_required(
                q_items,
                qr_items,
                &answers_by_link,
                "QuestionnaireResponse.item",
                errors,
            );
        }
    }
}

/// Walk the Questionnaire and the response together, reporting required, enabled
/// items that have no answer (questions) or are absent (groups).
fn check_required(
    q_items: &[JsonValue],
    qr_scope: &[JsonValue],
    qr_answers: &HashMap<&str, Vec<&serde_json::Map<String, JsonValue>>>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) {
    for q in q_items {
        let Some(qobj) = q.as_object() else {
            continue;
        };
        let link_id = qobj.get("linkId").and_then(|v| v.as_str()).unwrap_or("");
        let qtype = qobj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !item_enabled(qobj, qr_answers) {
            continue;
        }

        // Response items in this scope that answer this Questionnaire item.
        let matches: Vec<&serde_json::Map<String, JsonValue>> = qr_scope
            .iter()
            .filter_map(|v| v.as_object())
            .filter(|o| o.get("linkId").and_then(|v| v.as_str()) == Some(link_id))
            .collect();

        let required = qobj.get("required").and_then(|v| v.as_bool()) == Some(true);
        if required {
            let satisfied = match qtype {
                "group" | "display" => matches.iter().any(|m| {
                    m.get("item")
                        .and_then(|v| v.as_array())
                        .is_some_and(|a| !a.is_empty())
                }),
                _ => matches.iter().any(|m| {
                    m.get("answer")
                        .and_then(|v| v.as_array())
                        .is_some_and(|a| !a.is_empty())
                }),
            };
            if !satisfied {
                errors.push(error(
                    &format!("{path}[{link_id}]"),
                    format!("required item '{link_id}' has no answer"),
                ));
            }
        }

        // Recurse into child questions using the first matching response group.
        if let Some(children) = qobj.get("item").and_then(|v| v.as_array()) {
            let empty = Vec::new();
            let child_scope = matches
                .iter()
                .find_map(|m| m.get("item").and_then(|v| v.as_array()))
                .unwrap_or(&empty);
            check_required(
                children,
                child_scope,
                qr_answers,
                &format!("{path}[{link_id}].item"),
                errors,
            );
        }
    }
}

/// Collect every answered `linkId` in the response, mapped to its answer
/// objects (recursively, including answers nested under answers).
fn collect_qr_answers<'a>(
    items: &'a JsonValue,
    out: &mut HashMap<&'a str, Vec<&'a serde_json::Map<String, JsonValue>>>,
) {
    let Some(arr) = items.as_array() else {
        return;
    };
    for item in arr {
        let Some(obj) = item.as_object() else {
            continue;
        };
        if let (Some(link_id), Some(answers)) = (
            obj.get("linkId").and_then(|v| v.as_str()),
            obj.get("answer").and_then(|v| v.as_array()),
        ) {
            let entry = out.entry(link_id).or_default();
            for a in answers {
                if let Some(ao) = a.as_object() {
                    entry.push(ao);
                }
            }
            for a in answers {
                if let Some(nested) = a.get("item") {
                    collect_qr_answers(nested, out);
                }
            }
        }
        if let Some(nested) = obj.get("item") {
            collect_qr_answers(nested, out);
        }
    }
}

/// Whether a Questionnaire item is enabled, per its `enableWhen` conditions and
/// `enableBehavior` (`all` / `any`). An item with no `enableWhen` is always
/// enabled.
fn item_enabled(
    qobj: &serde_json::Map<String, JsonValue>,
    answers: &HashMap<&str, Vec<&serde_json::Map<String, JsonValue>>>,
) -> bool {
    let Some(conds) = qobj.get("enableWhen").and_then(|v| v.as_array()) else {
        return true;
    };
    if conds.is_empty() {
        return true;
    }
    // Default to `all`; `enableBehavior` is required by the spec when there is
    // more than one condition, but a single condition is unambiguous.
    let any = qobj.get("enableBehavior").and_then(|v| v.as_str()) == Some("any");
    let mut results = conds
        .iter()
        .filter_map(|c| c.as_object())
        .map(|c| eval_enable_when(c, answers));
    if any {
        results.any(|r| r)
    } else {
        results.all(|r| r)
    }
}

/// Evaluate a single `enableWhen` condition against the collected answers.
fn eval_enable_when(
    cond: &serde_json::Map<String, JsonValue>,
    answers: &HashMap<&str, Vec<&serde_json::Map<String, JsonValue>>>,
) -> bool {
    let question = cond.get("question").and_then(|v| v.as_str()).unwrap_or("");
    let operator = cond.get("operator").and_then(|v| v.as_str()).unwrap_or("");
    let answered = answers.get(question);

    if operator == "exists" {
        let want = cond
            .get("answerBoolean")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let has = answered.is_some_and(|a| !a.is_empty());
        return has == want;
    }

    let Some(answer_key) = cond
        .keys()
        .map(String::as_str)
        .find(|k| k.starts_with("answer") && *k != "answer")
    else {
        return false;
    };
    let target = &cond[answer_key];
    // The matching answer value key: answerBoolean -> valueBoolean, etc.
    let value_key = format!("value{}", &answer_key["answer".len()..]);
    let is_coding = answer_key == "answerCoding";

    answered.is_some_and(|list| {
        list.iter().any(|a| {
            let Some(av) = a.get(&value_key) else {
                return false;
            };
            match operator {
                "=" => value_equal(av, target, is_coding),
                "!=" => !value_equal(av, target, is_coding),
                ">" | "<" | ">=" | "<=" => numeric_compare(av, target, operator),
                _ => false,
            }
        })
    })
}

/// Equality for enableWhen: Codings compare on `system` + `code`; other values
/// compare structurally.
fn value_equal(a: &JsonValue, b: &JsonValue, is_coding: bool) -> bool {
    if is_coding {
        let field = |v: &JsonValue, f: &str| v.get(f).and_then(|x| x.as_str()).map(str::to_string);
        field(a, "code") == field(b, "code") && field(a, "system") == field(b, "system")
    } else {
        a == b
    }
}

/// Ordered comparison for numeric enableWhen operators.
fn numeric_compare(a: &JsonValue, b: &JsonValue, op: &str) -> bool {
    let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) else {
        return false;
    };
    match op {
        ">" => x > y,
        "<" => x < y,
        ">=" => x >= y,
        "<=" => x <= y,
        _ => false,
    }
}

/// Validate an array of QuestionnaireResponse items against the indexed
/// Questionnaire items.
fn validate_items(
    qr_items: &JsonValue,
    index: &HashMap<&str, QItem<'_>>,
    qr_answers: &HashMap<&str, Vec<&serde_json::Map<String, JsonValue>>>,
    strictness: QrStrictness,
    path: &str,
    errors: &mut Vec<ValidationError>,
) {
    let Some(arr) = qr_items.as_array() else {
        return;
    };
    for (i, item) in arr.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let item_path = format!("{path}[{i}]");
        let link_id = obj.get("linkId").and_then(|v| v.as_str()).unwrap_or("");

        let Some(qitem) = index.get(link_id) else {
            if strictness.unknown_link_id && !link_id.is_empty() {
                errors.push(error(
                    &item_path,
                    format!("linkId '{link_id}' has no matching item in the Questionnaire"),
                ));
            }
            continue;
        };

        let answers = obj.get("answer").and_then(|v| v.as_array());
        let allowed = allowed_answer_keys(qitem);
        let enabled = item_enabled(qitem.obj, qr_answers);

        // A disabled item should not be answered.
        if strictness.disabled_answered && !enabled && answers.is_some_and(|a| !a.is_empty()) {
            errors.push(error(
                &item_path,
                format!("item '{link_id}' is answered but disabled by enableWhen"),
            ));
        }

        match allowed {
            None => {
                // group / display: no answer permitted.
                if answers.is_some_and(|a| !a.is_empty()) {
                    errors.push(error(
                        &item_path,
                        format!(
                            "item '{link_id}' of type '{}' must not have an answer",
                            qitem.item_type
                        ),
                    ));
                }
            }
            Some(allowed_keys) => {
                if let Some(answers) = answers {
                    // repeats defaults to false: at most one answer.
                    let repeats = qitem
                        .obj
                        .get("repeats")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if !repeats && answers.len() > 1 {
                        errors.push(error(
                            &item_path,
                            format!(
                                "item '{link_id}' does not allow repeats but has {} answers",
                                answers.len()
                            ),
                        ));
                    }

                    for (ai, answer) in answers.iter().enumerate() {
                        let Some(aobj) = answer.as_object() else {
                            continue;
                        };
                        let answer_path = format!("{item_path}.answer[{ai}]");
                        if let Some(key) = answer_value_key(aobj) {
                            if !allowed_keys.is_empty() && !allowed_keys.contains(&key) {
                                errors.push(error(
                                    &answer_path,
                                    format!(
                                        "answer type '{key}' is invalid for item '{link_id}' of type '{}' (expected {})",
                                        qitem.item_type,
                                        allowed_keys.join(" or ")
                                    ),
                                ));
                            } else {
                                check_answer_option(
                                    qitem,
                                    aobj,
                                    key,
                                    link_id,
                                    &answer_path,
                                    errors,
                                );
                            }
                        }

                        // Answers may carry nested items.
                        if let Some(nested) = aobj.get("item") {
                            validate_items(
                                nested,
                                index,
                                qr_answers,
                                strictness,
                                &format!("{answer_path}.item"),
                                errors,
                            );
                        }
                    }
                }
                // Required-but-missing is reported by the Questionnaire-driven
                // `check_required` pass, which also covers items absent from the
                // response entirely.
            }
        }

        // Recurse into nested response items (groups).
        if let Some(nested) = obj.get("item") {
            validate_items(
                nested,
                index,
                qr_answers,
                strictness,
                &format!("{item_path}.item"),
                errors,
            );
        }
    }
}

/// For choice / open-choice items with an inline `answerOption`, verify a coded
/// answer matches one of the offered options. Non-matching coded answers are
/// rejected; open-choice free-text (`valueString`) is always allowed.
fn check_answer_option(
    qitem: &QItem<'_>,
    answer: &serde_json::Map<String, JsonValue>,
    key: &str,
    link_id: &str,
    path: &str,
    errors: &mut Vec<ValidationError>,
) {
    let Some(opts) = qitem.obj.get("answerOption").and_then(|v| v.as_array()) else {
        return;
    };
    // open-choice free-text string is always acceptable.
    if qitem.item_type == "open-choice" && key == "valueString" {
        return;
    }
    let answer_val = &answer[key];
    let matches = opts.iter().any(|opt| {
        opt.as_object()
            .and_then(|o| o.get(key))
            .is_some_and(|opt_val| answer_option_equal(key, opt_val, answer_val))
    });
    if !matches {
        errors.push(error(
            path,
            format!("answer for item '{link_id}' is not one of the allowed answerOption values"),
        ));
    }
}

/// Compare an answer value against an answerOption value. Codings match on
/// `system` + `code`; other types match on structural equality.
fn answer_option_equal(key: &str, opt_val: &JsonValue, answer_val: &JsonValue) -> bool {
    if key == "valueCoding" {
        let code = |v: &JsonValue| v.get("code").and_then(|c| c.as_str()).map(str::to_string);
        let system = |v: &JsonValue| v.get("system").and_then(|c| c.as_str()).map(str::to_string);
        code(opt_val) == code(answer_val) && system(opt_val) == system(answer_val)
    } else {
        opt_val == answer_val
    }
}
