# Decoration API Performance Benchmarks

This document describes the performance benchmark suite for Zed's decoration API, created for Cursorless integration.

## Overview

The benchmark suite (`decoration_performance.rs`) measures the performance characteristics of the decoration system to ensure it meets the requirements for real-time voice coding with Cursorless.

## Benchmark Groups

### 1. Hat Creation Benchmarks

Tests the overhead of creating decoration types:

- **create_single_hat**: Create a single hat decoration (color + shape)
- **create_all_88_hat_types**: Create all 88 hat combinations (8 colors × 11 shapes)
- **create_flash_highlight**: Create a flash highlight decoration

**Target Performance:**
- Single hat creation: < 10μs
- All 88 hats: < 1ms
- Flash highlight: < 5μs

### 2. Decoration Registry Benchmarks

Tests the core registry operations:

- **create_and_set**: Create decoration type and set decorations on editor
- **set_1000_decorations**: Batch set 1,000 decorations at once

**Target Performance:**
- Create and set: < 100μs
- 1,000 decorations: < 5ms

### 3. Hat Tokenization Benchmarks

Tests text tokenization for hat placement:

- **tokenize_100_chars**: Tokenize ~100 character text
- **tokenize_1000_chars**: Tokenize ~1,000 character text
- **tokenize_unicode**: Tokenize Unicode text (emoji, CJK, complex graphemes)

**Target Performance:**
- 100 chars: < 50μs
- 1,000 chars: < 500μs
- Unicode text: < 200μs

### 4. Hat Renderer Benchmarks

Tests the hat rendering system:

- **assign_10_hats**: Assign 10 hats to tokens
- **assign_50_hats**: Assign 50 hats to tokens

**Target Performance:**
- 10 hats: < 200μs
- 50 hats: < 1ms

### 5. Highlight Renderer Benchmarks

Tests the flash highlight system:

- **add_10_highlights**: Add 10 flash highlights
- **add_100_highlights**: Add 100 flash highlights

**Target Performance:**
- 10 highlights: < 200μs
- 100 highlights: < 2ms

### 6. Realistic Scenarios

Tests real-world Cursorless usage:

- **cursorless_scenario**: Assign 20 hats + 5 highlights simultaneously on realistic code

**Target Performance:**
- Realistic scenario: < 2ms (to maintain 60 FPS = ~16ms frame budget)

## Running Benchmarks

### Run all decoration benchmarks:

```bash
cd crates/editor
cargo bench --bench decoration_performance
```

### Run specific benchmark group:

```bash
cargo bench --bench decoration_performance -- "Hat Creation"
cargo bench --bench decoration_performance -- "Decoration Registry"
cargo bench --bench decoration_performance -- "Hat Tokenization"
cargo bench --bench decoration_performance -- "Hat Renderer"
cargo bench --bench decoration_performance -- "Highlight Renderer"
cargo bench --bench decoration_performance -- "Realistic Scenarios"
```

### Run specific benchmark:

```bash
cargo bench --bench decoration_performance -- "create_single_hat"
cargo bench --bench decoration_performance -- "assign_50_hats"
```

## Performance Targets Rationale

### Frame Budget Analysis

For smooth 60 FPS voice coding:
- Frame time: 16.67ms
- Editor rendering budget: ~10ms
- Decoration rendering budget: ~2-3ms
- Leaves: ~3-4ms for other UI

### Cursorless Usage Patterns

Typical Cursorless session:
- **Hat updates**: 10-50 hats visible at once
- **Highlight updates**: 1-5 highlights for target feedback
- **Update frequency**: Every voice command (1-5 per second)
- **Latency requirement**: < 50ms perceived as instantaneous

### Performance Constraints

The benchmarks ensure:
1. **Hat assignment** (50 hats): < 1ms → 16x safety margin for 60 FPS
2. **Highlight assignment** (10 highlights): < 200μs → 80x safety margin
3. **Combined realistic scenario**: < 2ms → 8x safety margin
4. **Registry operations** scale linearly with decoration count

## Interpreting Results

### Good Performance Indicators

✅ All operations complete in < 10ms
✅ Linear scaling (2x decorations = ~2x time)
✅ Unicode handling not significantly slower than ASCII
✅ Realistic scenario < 2ms

### Warning Signs

⚠️ Operations taking > 10ms
⚠️ Quadratic scaling (2x decorations = 4x time)
⚠️ Unicode handling > 3x slower than ASCII
⚠️ Realistic scenario > 5ms

### Critical Issues

🚨 Operations taking > 50ms
🚨 Exponential scaling
🚨 Realistic scenario > 16ms (drops below 60 FPS)

## Optimization Strategies

If benchmarks show performance issues:

1. **Type Caching**: Ensure decoration types are reused, not recreated
2. **Batch Operations**: Group decoration updates to minimize registry locks
3. **Spatial Indexing**: Consider R-tree for large decoration sets
4. **Incremental Updates**: Only retokenize changed regions
5. **Viewport Culling**: Only create decorations for visible text

## Continuous Performance Monitoring

Recommended workflow:

1. **Baseline**: Run benchmarks before changes
2. **Regression Detection**: Compare before/after results
3. **CI Integration**: Fail PR if performance degrades > 20%
4. **Profiling**: Use `cargo flamegraph` for detailed analysis

```bash
# Example: Compare before/after
cargo bench --bench decoration_performance -- --save-baseline before
# ... make changes ...
cargo bench --bench decoration_performance -- --baseline before
```

## Related Documentation

- [Decoration API Contract](decoration-api-contract.md)
- [Decoration Buffer Tracking](decoration-buffer-tracking.md)
- [Cursorless Requirements](cursorless-decoration-requirements.md)
- [Hat Renderer Architecture](decoration-wrapper-api.md)
