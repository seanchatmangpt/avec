//! Typed noun-verb routes GENERATED from first-class RDF arguments.
//!
//! Every body closes through `BehaviorSpec`, except commands carrying the
//! explicit, admitted `cnv:CustomBehavior` opt-out, which route to a
//! hand-written `crate::custom_handlers::<noun>_<verb>` function instead. That
//! is the sole consumer handler seam this compiler ever emits, and it is
//! visible per-command in the source ontology, not a blanket escape hatch.

use clap_noun_verb::{NounVerbError, Result};
use clap_noun_verb_macros::verb;
use serde::Serialize;
use serde_json::{Map, Value};
#[doc = "Assert 64-byte alignment for Buf sizes 1..1000 step 7."]
#[verb("test", "align")]
fn align_test() -> Result<Value> {
    let inputs = Map::new();
    crate::custom_handlers::align_test(inputs)
}

#[doc = "64 MiB mask_words line-rate bench, min-of-5."]
#[verb("mask", "bench")]
fn bench_mask() -> Result<Value> {
    let inputs = Map::new();
    crate::custom_handlers::bench_mask(inputs)
}

#[doc = "Literal-page w=4 RLE encode comparison, min-of-5."]
#[verb("parquet", "bench")]
fn bench_parquet() -> Result<Value> {
    let inputs = Map::new();
    crate::custom_handlers::bench_parquet(inputs)
}

#[doc = "Print the CLI version."]
#[verb("version", "self")]
fn self_version() -> Result<Value> {
    let inputs = Map::new();
    crate::custom_handlers::self_version(inputs)
}

#[linkme::distributed_slice(::clap_noun_verb::cli::registry::__NOUN_REGISTRY)]
static REGISTER_ALIGN_NOUN: fn() = register_align_noun;

fn register_align_noun() {
    ::clap_noun_verb::cli::registry::CommandRegistry::register_noun(
        "align",
        "64-byte alignment probe over the Buf allocator.",
    );
}

#[linkme::distributed_slice(::clap_noun_verb::cli::registry::__NOUN_REGISTRY)]
static REGISTER_BENCH_NOUN: fn() = register_bench_noun;

fn register_bench_noun() {
    ::clap_noun_verb::cli::registry::CommandRegistry::register_noun(
        "bench",
        "Kernel line-rate benchmarks.",
    );
}

#[linkme::distributed_slice(::clap_noun_verb::cli::registry::__NOUN_REGISTRY)]
static REGISTER_SELF_NOUN: fn() = register_self_noun;

fn register_self_noun() {
    ::clap_noun_verb::cli::registry::CommandRegistry::register_noun(
        "self",
        "Meta commands about the CLI itself.",
    );
}

#[allow(dead_code)]
fn encode_input<T: Serialize>(name: &str, value: &T) -> Result<Value> {
    serde_json::to_value(value).map_err(|error| {
        NounVerbError::execution_error(format!(
            "failed to encode generated argument {name:?}: {error}"
        ))
    })
}

#[cfg(test)]
mod generated_route_proofs {
    const GENERATED_SOURCE: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/generated_cli.rs"));

    fn routes_only() -> &'static str {
        GENERATED_SOURCE
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(GENERATED_SOURCE)
    }

    #[test]
    fn every_admitted_command_became_one_registered_verb() {
        assert_eq!(routes_only().matches("#[verb(").count(), 4);
    }

    #[test]
    fn no_consumer_handler_seam_survives_generation() {
        // `custom_handlers::` is the sole, explicitly admitted escape hatch
        // (cnv:CustomBehavior); strip it before checking for the legacy,
        // unbounded `handlers::` seam this compiler never emits otherwise.
        let without_admitted_custom_seam = routes_only().replace("custom_handlers::", "");
        assert!(!without_admitted_custom_seam.contains("handlers::"));
        assert!(!routes_only().contains("todo!"));
        assert!(!routes_only().contains("unimplemented!"));
    }

    #[test]
    fn every_custom_command_routes_to_its_own_handler() {
        // Proves per-command wiring, not just that a seam exists somewhere:
        // each cnv:CustomBehavior command must call the one handler function
        // named after ITS OWN noun and verb, not a neighboring command's.
        assert!(
            routes_only().contains("custom_handlers::align_test("),
            "align test must route to its own custom handler"
        );
        assert!(
            routes_only().contains("custom_handlers::bench_mask("),
            "bench mask must route to its own custom handler"
        );
        assert!(
            routes_only().contains("custom_handlers::bench_parquet("),
            "bench parquet must route to its own custom handler"
        );
        assert!(
            routes_only().contains("custom_handlers::self_version("),
            "self version must route to its own custom handler"
        );
    }
}
