pub mod canonical;
pub mod cli;
pub mod collector;
pub mod event;
pub mod export;
pub mod integrations;
pub mod policy;
pub mod run_log;
pub mod schema;
pub mod signing;

pub use event::{EventInput, build_event, build_event_from_input, event_hash, verify_event_hash};
pub use export::{to_openinference_span, to_otel_span};
pub use policy::policy_decision;
pub use run_log::{
    AppendEventInput, VerifyReport, append_event_to_run, append_jsonl, next_event_for_run,
    read_jsonl, verify_events, verify_run_log, write_jsonl,
};
pub use schema::{SchemaKind, validate_value};
pub use signing::{
    LocalKeyFile, generate_key, inspect_key_view, public_key_view, read_key, sign_value,
    signed_payload_hash, verify_signature,
};
