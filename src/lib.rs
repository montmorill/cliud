#![warn(clippy::restriction)]
#![allow(clippy::absolute_paths, reason = "..")]
#![allow(clippy::arbitrary_source_item_ordering, reason = "..")]
#![allow(clippy::arithmetic_side_effects, reason = "..")]
#![allow(clippy::big_endian_bytes, reason = "..")]
#![allow(clippy::blanket_clippy_restriction_lints, reason = "..")]
#![allow(clippy::default_numeric_fallback, reason = "..")]
#![allow(clippy::error_impl_error, reason = "..")]
#![allow(clippy::exhaustive_enums, reason = "..")]
#![allow(clippy::exhaustive_structs, reason = "..")]
#![allow(clippy::impl_trait_in_params, reason = "..")]
#![allow(clippy::implicit_return, reason = "..")]
#![allow(clippy::integer_division_remainder_used, reason = "..")]
#![allow(clippy::let_underscore_untyped, reason = "..")]
#![allow(clippy::min_ident_chars, reason = "..")]
#![allow(clippy::missing_docs_in_private_items, reason = "..")]
#![allow(clippy::missing_trait_methods, reason = "..")]
#![allow(clippy::module_name_repetitions, reason = "..")]
#![allow(clippy::print_stderr, reason = "..")]
#![allow(clippy::print_stdout, reason = "..")]
#![allow(clippy::question_mark_used, reason = "..")]
#![allow(clippy::separated_literal_suffix, reason = "..")]
#![allow(clippy::shadow_reuse, reason = "..")]
#![allow(clippy::single_call_fn, reason = "..")]
#![allow(clippy::single_char_lifetime_names, reason = "..")]
#![allow(clippy::std_instead_of_alloc, reason = "..")]
#![allow(clippy::std_instead_of_core, reason = "..")]
#![allow(clippy::use_debug, reason = "..")]

pub mod compress;
pub mod http;
pub mod middleware;
pub mod server;
pub mod service;
pub mod websocket;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
