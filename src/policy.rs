use chrono::{DateTime, Utc};
use serde_json::{Value, json};

pub fn policy_decision(policy: &Value, agent: &str, action: &str, resource: &str) -> Value {
    if policy
        .get("agent_id")
        .and_then(Value::as_str)
        .is_some_and(|id| id != agent)
    {
        return decision(policy, "deny", "agent did not match policy");
    }
    if matches_rule(policy.get("deny"), action, resource) {
        return decision(policy, "deny", "matched deny rule");
    }
    if matches_rule(policy.get("require_approval"), action, resource) {
        return decision(policy, "require_approval", "matched require_approval rule");
    }
    if matches_rule(policy.get("allow"), action, resource) {
        return decision(policy, "allow", "matched allow rule");
    }
    decision(policy, "deny", "no matching allow rule")
}

fn decision(policy: &Value, decision: &str, reason: &str) -> Value {
    json!({
        "decision": decision,
        "policy_id": policy.get("policy_id").and_then(Value::as_str),
        "policy_version": policy.get("version").and_then(Value::as_str),
        "reason": reason,
    })
}

fn matches_rule(rules: Option<&Value>, action: &str, resource: &str) -> bool {
    rules.and_then(Value::as_array).is_some_and(|rules| {
        rules
            .iter()
            .any(|rule| rule_matches(rule, action, resource))
    })
}

fn rule_matches(rule: &Value, action: &str, resource: &str) -> bool {
    let rule_action = rule.get("action").and_then(Value::as_str).unwrap_or("");
    let rule_resource = rule.get("resource").and_then(Value::as_str).unwrap_or("");
    rule_is_active(rule)
        && pattern_matches(rule_action, action)
        && pattern_matches(rule_resource, resource)
}

pub fn pattern_matches(pattern: &str, value: &str) -> bool {
    pattern == "*"
        || pattern == value
        || pattern
            .strip_suffix('*')
            .is_some_and(|prefix| value.starts_with(prefix))
}

fn rule_is_active(rule: &Value) -> bool {
    rule.get("expires_at")
        .and_then(Value::as_str)
        .map(|expires_at| {
            DateTime::parse_from_rfc3339(expires_at)
                .map(|expires_at| expires_at.with_timezone(&Utc) > Utc::now())
                .unwrap_or(false)
        })
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prefix_wildcard_policy_rule_matches() {
        assert!(pattern_matches(
            "discord://guild/123/*",
            "discord://guild/123/channel/456"
        ));
        assert!(!pattern_matches(
            "discord://guild/123/*",
            "discord://guild/999/channel/456"
        ));
    }

    #[test]
    fn deny_rules_win_over_allow_rules() {
        let policy = json!({
            "policy_id": "policy",
            "version": "v1",
            "agent_id": "agent",
            "allow": [{"action": "*", "resource": "*"}],
            "deny": [{"action": "secret.read", "resource": "prod/*"}]
        });
        let decision = policy_decision(&policy, "agent", "secret.read", "prod/db");
        assert_eq!(decision["decision"], "deny");
        assert_eq!(decision["reason"], "matched deny rule");
    }

    #[test]
    fn require_approval_rules_win_over_allow_rules() {
        let policy = json!({
            "policy_id": "policy",
            "version": "v1",
            "agent_id": "agent",
            "allow": [{"action": "*", "resource": "*"}],
            "require_approval": [{"action": "github.pr.merge", "resource": "*"}]
        });
        let decision = policy_decision(
            &policy,
            "agent",
            "github.pr.merge",
            "repo://owner/name/pull/1",
        );
        assert_eq!(decision["decision"], "require_approval");
        assert_eq!(decision["reason"], "matched require_approval rule");
    }

    #[test]
    fn unmatched_agent_is_denied() {
        let policy = json!({
            "policy_id": "policy",
            "version": "v1",
            "agent_id": "agent",
            "allow": [{"action": "*", "resource": "*"}]
        });
        let decision = policy_decision(&policy, "other-agent", "http.get", "https://example.com");
        assert_eq!(decision["decision"], "deny");
        assert_eq!(decision["reason"], "agent did not match policy");
    }

    #[test]
    fn expired_rules_do_not_match() {
        let policy = json!({
            "policy_id": "policy",
            "version": "v1",
            "agent_id": "agent",
            "allow": [{
                "action": "http.get",
                "resource": "https://example.com",
                "expires_at": "2000-01-01T00:00:00Z"
            }]
        });
        let decision = policy_decision(&policy, "agent", "http.get", "https://example.com");
        assert_eq!(decision["decision"], "deny");
        assert_eq!(decision["reason"], "no matching allow rule");
    }
}
