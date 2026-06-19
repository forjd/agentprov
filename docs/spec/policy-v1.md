# Policy v1

## Purpose

A Policy defines which actions an agent may perform, which are denied, and which require approval.

## Required fields

- `schema`: must be `agentprov.dev/policy/v1`
- `policy_id`
- `version`
- `agent_id`

## Rule lists

- `allow`
- `deny`
- `require_approval`

Each rule has:

- `action`
- `resource`
- optional `expires_at`

Expired rules do not match. Invalid `expires_at` values are treated as inactive
rules.

Future versions may add `conditions` and richer resource matching.

## Matching rules

The MVP supports:

- exact matches
- `*` wildcard
- prefix wildcard suffix, for example `discord://guild/123/*`

## Decision order

1. If the policy `agent_id` does not match, deny.
2. Deny rules win.
3. Require-approval rules are distinct from allow.
4. Allow rules allow.
5. Otherwise deny.

## Approval events

When `agentprov policy check --emit-event` evaluates to `require_approval`, the
CLI writes both a `permission.check` event and a `human.approval.request` event
to the run log. Approval grants and denials can be represented with
`human.approval.grant` and `human.approval.deny` events.
