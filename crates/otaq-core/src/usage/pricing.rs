//! The price table of the optional dollar estimate (USG-02; d0020). The one place prices live.
//!
//! Anthropic's first-party API prices per million tokens, from the Claude API reference bundled with Claude Code
//! 2.1.296 (models table cached 2026-10-06; read 2026-10-10). Cache writes cost 1.25× the input price for the
//! 5-minute cache and 2× for the 1-hour cache; cache reads have their own price per model. Claude Haiku 5.5 is
//! priced at its rate for prompts up to 100K tokens. A model that isn't in the table (an older one, or Claude
//! Code's own `<synthetic>` messages) adds nothing: the figure is an estimate and says so.

use super::Tokens;

/// Dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
}

/// Model id prefixes and their prices; the longest matching prefix wins (`claude-opus-5-5` before
/// `claude-opus-5`), so dated ids like `claude-haiku-4-5-20251001` match too.
pub const PRICES: &[(&str, Price)] = &[
    ("claude-fable-5-1", price(10.0, 50.0, 0.25)),
    ("claude-mythos-5-1", price(10.0, 50.0, 0.25)),
    ("claude-fable-5", price(10.0, 50.0, 1.0)),
    ("claude-mythos-5", price(10.0, 50.0, 1.0)),
    ("claude-opus-5-5", price(4.0, 20.0, 0.20)),
    ("claude-opus-5", price(5.0, 25.0, 0.50)),
    ("claude-opus-4-8", price(5.0, 25.0, 0.50)),
    ("claude-opus-4-7", price(5.0, 25.0, 0.50)),
    ("claude-opus-4-6", price(5.0, 25.0, 0.50)),
    ("claude-sonnet-5-5", price(2.0, 10.0, 0.20)),
    ("claude-sonnet-5", price(2.0, 10.0, 0.20)),
    ("claude-sonnet-4-6", price(3.0, 15.0, 0.30)),
    ("claude-haiku-5-5", price(0.10, 0.50, 0.01)),
    ("claude-haiku-4-5", price(1.0, 5.0, 0.10)),
];

/// Cache writes for the 5-minute cache, as a multiple of the input price.
const WRITE_5M: f64 = 1.25;
/// Cache writes for the 1-hour cache, as a multiple of the input price.
const WRITE_1H: f64 = 2.0;

const fn price(input: f64, output: f64, cache_read: f64) -> Price {
    Price {
        input,
        output,
        cache_read,
    }
}

/// The price of a model id, if the table has it.
pub fn price_of(model: &str) -> Option<Price> {
    PRICES
        .iter()
        .filter(|(prefix, _)| {
            model
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'))
        })
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, p)| *p)
}

/// What one response cost, in millionths of a dollar. `cache_write_1h` is the part of the cache writes that
/// went to the 1-hour cache. `None` for a model the table doesn't know.
pub fn cost_micro_usd(model: Option<&str>, tokens: &Tokens, cache_write_1h: u64) -> Option<u64> {
    let p = price_of(model?)?;
    let n = |v: u64| v as f64;
    let write_1h = cache_write_1h.min(tokens.cache_write);
    let write_5m = tokens.cache_write - write_1h;
    // Dollars per million tokens × tokens = millionths of a dollar.
    let micro = n(tokens.input) * p.input
        + n(tokens.output) * p.output
        + n(write_5m) * p.input * WRITE_5M
        + n(write_1h) * p.input * WRITE_1H
        + n(tokens.cache_read) * p.cache_read;
    Some(micro.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_longest_prefix_wins_and_dated_ids_match() {
        assert_eq!(price_of("claude-opus-5-5").unwrap().input, 4.0);
        assert_eq!(price_of("claude-opus-5").unwrap().input, 5.0);
        assert_eq!(price_of("claude-haiku-4-5-20251001").unwrap().input, 1.0);
        assert_eq!(price_of("claude-fable-5-1").unwrap().cache_read, 0.25);
        assert_eq!(price_of("claude-fable-5").unwrap().cache_read, 1.0);
        assert!(price_of("<synthetic>").is_none());
        assert!(price_of("claude-opus-4-1").is_none());
    }

    #[test]
    fn a_response_costs_each_count_at_its_rate() {
        // Opus 5.5: 2 in, 226 out, 11 524 written to the 1-hour cache, 28 134 read.
        let tokens = Tokens {
            input: 2,
            output: 226,
            cache_write: 11_524,
            cache_read: 28_134,
        };
        // 2×4 + 226×20 + 11 524×8 + 28 134×0.2 = 8 + 4 520 + 92 192 + 5 626.8
        assert_eq!(
            cost_micro_usd(Some("claude-opus-5-5"), &tokens, 11_524),
            Some(102_347)
        );
        // The same writes to the 5-minute cache: 11 524 × 5 = 57 620.
        assert_eq!(
            cost_micro_usd(Some("claude-opus-5-5"), &tokens, 0),
            Some(67_775)
        );
        assert_eq!(cost_micro_usd(None, &tokens, 0), None);
    }
}
