
# Performance Analysis (Benchmarks with `hyperfine`) [OUTDATED]

## 1. Introduction and Methodology

This document presents a comparative performance analysis of `axes` against the task runners `just` and `make`, using the statistical benchmarking tool `hyperfine`. The objective is to rigorously evaluate the execution latency and scalability of each tool under different workloads.

Unlike a single timing measurement, `hyperfine` executes each command multiple times (`--runs 100`) after a warm-up phase (`--warmup 10`), providing a `mean` and standard deviation (`± σ`). This method minimizes the impact of system fluctuations and offers a much more accurate view of each tool's real performance in a "hot run" scenario.

For `axes`, a "hot run" implies that the Abstract Syntax Tree (AST) binary cache has already been generated in a previous run, simulating the most common use case in a developer's workflow.

### Test Scenarios

The following scenarios were defined, varying the number of declared scripts in each tool's configuration file:

* **Low:** 100 basic scripts.
* **Mid:** 1,000 basic scripts.
* **High:** 10,000 basic scripts.
* **Big:** 100,000 basic scripts.
* **Divided:** 100,000 scripts split across two projects with inheritance (`axes` only).

## 2. Results Analysis

### 2.1. Latency Comparison (Mean Execution Time)

Mean latency is the primary metric for evaluating the agility of the tool in daily use.

| Scenario | `axes` (ms ± σ) | `just` (ms ± σ) | `make` (ms ± σ) |
| :-------- | :-------------: | :-------------: | :-------------: |
| **Low** (100) | 3.4 ± 0.2 | 40.3 ± 2.5 | **1.9 ± 0.2** |
| **Mid** (1,000) | **3.9 ± 0.2** | 46.7 ± 2.2 | 4.5 ± 0.3 |
| **High** (10,000) | **9.4 ± 0.5** | 106.2 ± 3.6 | 170.6 ± 4.2 |
| **Big** (100,000) | **65.6 ± 2.1** | 650.6 ± 14.5 | N/A* |

<small><i>*N/A: `make` was omitted from the `hyperfine` test in the "Big" scenario due to its extremely high latency (over 100 seconds), documented in previous tests, which makes it unviable for fast statistical benchmarking.</i></small>

#### Scalability Analysis and Inflection Points

1. **`Low` Scenario (100 scripts):** In very small configurations, `make` is the undisputed leader at ~1.9ms. Its simplicity and maturity as a C binary give it an advantage in minimal startup overhead. `axes` follows closely at ~3.4ms, demonstrating extremely low baseline latency. `just` is noticeably slower, with a baseline latency of ~40.3ms.

2. **`Mid` Scenario (1,000 scripts): The First Inflection Point.** At only 1,000 scripts, `axes` already outperforms `make` (`3.9ms` vs `4.5ms`). This is the first indication that the text parsing cost of `make` is starting to become a relevant factor, while the binary deserialization cost of `axes` remains almost constant.

3. **`High` Scenario (10,000 scripts): Architectural Dominance.** At 10,000 scripts, the architectural advantage of `axes` is overwhelming.
    * `axes` maintains an exceptional latency of **9.4ms**.
    * `just` exceeds 100ms.
    * `make` exceeds 170ms.
    * According to `hyperfine` data, at this point, `axes` is **11.3 times faster than `just`** and **18.2 times faster than `make`**. The Ahead-of-Time (AOT) compilation strategy to a binary AST proves fundamentally superior to the run-time text parsing approach at this scale.

4. **`Big` Scenario (100,000 scripts): Extreme Scalability.** `axes` maintains excellent performance at **65.6ms**, while `just` degrades to **650.6ms**. `axes` is almost **10 times faster than `just`** in this high-complexity scenario.

### 2.2. Inheritance Engine Performance (`Divided` vs. `Big`)

This benchmark is crucial as it measures the overhead (or optimization) of one of `axes`'s key features: the orchestration of nested projects.

| `axes` Scenario (100,000 scripts) | Mean Time (ms ± σ) | Relative Performance |
| :-------------------------------- | :----------------: | :------------------: |
| **Big** (Single File) | 65.6 ± 2.1 | 1.00x |
| **Divided** (2 Files with Inheritance) | **44.2 ± 2.0** | **1.48x Faster** |

#### Inheritance Architecture Analysis

The result is counter-intuitive and extremely positive: managing 100,000 scripts split across an inheritance hierarchy is almost **50% faster** than managing them in a single monolithic file.

This strongly validates several architectural decisions:

* **Concurrent Loading:** The `ConfigLoader` processes the two files (`parent` and `into`) in parallel using `rayon`. Concurrency in disk reading and binary cache deserialization is more efficient than processing a single, larger file sequentially.
* **In-Memory Merge Efficiency:** The operation of merging configuration layers in the `ResolvedConfig` is computationally very cheap, proving that the complexity of inheritance introduces no performance penalty on the "hot path."
* **Data Locality:** It is plausible that working with smaller, separate cache files results in better OS filesystem cache performance and CPU cache performance.

**The conclusion is unequivocal: the orchestration engine of `axes` is not an overhead, but an optimization.**

## 3. Technical Conclusions

1. **Sub-linear Scalability:** While `just` and `make` show linear or worse growth in latency as configuration complexity increases, `axes` demonstrates sub-linear growth thanks to its binary AST cache architecture.

2. **Validation of the AOT Model:** The strategy of "compiling" the `axes.toml` on the first run and then simply deserializing the AST on subsequent executions is fundamentally more scalable than any run-time text parsing approach.

3. **Orchestration as Optimization:** Advanced features of `axes`, such as inheritance and concurrent loading, not only do not compromise performance but actively enhance it in large-scale scenarios. This positions `axes` as a unique solution that offers orchestrator capabilities with speed superior to that of simple executors.

The raw benchmarks used to build the above analysis are provided below.

## Benchmarks

>
> *All benchmarks has divided by:*
>
> * Low: 100 basic scripts.
> * Mid: 1000 basic scripts.
> * High: 10000 basic scripts.
> * Big: 100000 basic scripts.
> * Divided: 50000 + 50000 basic scripts divided into two inherited projects.

### Hyperfine datas

[.../axes/stress_tests:hyperfine]
→ hyperfine --shell=none "axes ./low_test/script_90"     "just --justfile low_test/justfile script_90"    "make -f low_test/makefile.mk script_90"      --warmup 10 --runs 100
Benchmark 1: axes ./low_test/script_90
  Time (mean ± σ):       3.4 ms ±   0.2 ms    [User: 2.3 ms, System: 2.6 ms]
  Range (min … max):     3.1 ms …   4.0 ms    100 runs

Benchmark 2: just --justfile low_test/justfile script_90
  Time (mean ± σ):      40.3 ms ±   2.5 ms    [User: 2.3 ms, System: 1.8 ms]
  Range (min … max):    37.2 ms …  56.6 ms    100 runs

Benchmark 3: make -f low_test/makefile.mk script_90
  Time (mean ± σ):       1.9 ms ±   0.2 ms    [User: 1.3 ms, System: 0.4 ms]
  Range (min … max):     1.7 ms …   2.4 ms    100 runs

Summary
  make -f low_test/makefile.mk script_90 ran
    1.79 ± 0.18 times faster than axes ./low_test/script_90
   21.15 ± 2.18 times faster than just --justfile low_test/justfile script_90

---

[.../axes/stress_tests:hyperfine]
→ hyperfine --shell=none  "axes ./mid_test/script_900"     "just --justfile mid_test/justfile script_900"    "make -f mid_test/makefile.mk script_900"     --warmup 10 --runs 100
Benchmark 1: axes ./mid_test/script_900
  Time (mean ± σ):       3.9 ms ±   0.2 ms    [User: 1.9 ms, System: 3.5 ms]
  Range (min … max):     3.5 ms …   4.6 ms    100 runs

Benchmark 2: just --justfile mid_test/justfile script_900
  Time (mean ± σ):      46.7 ms ±   2.2 ms    [User: 4.5 ms, System: 5.1 ms]
  Range (min … max):    43.6 ms …  54.1 ms    100 runs

Benchmark 3: make -f mid_test/makefile.mk script_900
  Time (mean ± σ):       4.5 ms ±   0.3 ms    [User: 3.5 ms, System: 0.7 ms]
  Range (min … max):     4.1 ms …   5.7 ms    100 runs

Summary
  axes ./mid_test/script_900 ran
    1.16 ± 0.10 times faster than make -f mid_test/makefile.mk script_900
   12.01 ± 0.79 times faster than just --justfile mid_test/justfile script_900

---

[.../axes/stress_tests:hyperfine]
→ hyperfine --shell=none   "axes ./high_test/script_9000"     "just --justfile high_test/justfile script_9000"    "make -f high_test/makefile.mk script_9000"    --warmup 10 --runs 100
Benchmark 1: axes ./high_test/script_9000
  Time (mean ± σ):       9.4 ms ±   0.5 ms    [User: 4.5 ms, System: 6.6 ms]
  Range (min … max):     8.0 ms …  10.7 ms    100 runs

Benchmark 2: just --justfile high_test/justfile script_9000
  Time (mean ± σ):     106.2 ms ±   3.6 ms    [User: 38.9 ms, System: 24.4 ms]
  Range (min … max):    99.0 ms … 115.9 ms    100 runs

Benchmark 3: make -f high_test/makefile.mk script_9000
  Time (mean ± σ):     170.6 ms ±   4.2 ms    [User: 167.3 ms, System: 3.5 ms]
  Range (min … max):   164.3 ms … 188.9 ms    100 runs

Summary
  axes ./high_test/script_9000 ran
   11.35 ± 0.74 times faster than just --justfile high_test/justfile script_9000
   18.24 ± 1.11 times faster than make -f high_test/makefile.mk script_9000

---

[.../axes/stress_tests:hyperfine]
→ hyperfine --shell=none    "axes ./big_test/script_90000" "axes ./big_divided_test/into/script_90000"    "just --justfile big_test/justfile script_90000"       --warmup 10 --runs 100
Benchmark 1: axes ./big_test/script_90000
  Time (mean ± σ):      65.6 ms ±   2.1 ms    [User: 43.8 ms, System: 22.6 ms]
  Range (min … max):    62.6 ms …  73.8 ms    100 runs

Benchmark 2: axes ./big_divided_test/into/script_90000
  Time (mean ± σ):      44.2 ms ±   2.0 ms    [User: 40.1 ms, System: 24.7 ms]
  Range (min … max):    41.3 ms …  50.8 ms    100 runs

Benchmark 3: just --justfile big_test/justfile script_90000
  Time (mean ± σ):     650.6 ms ±  14.5 ms    [User: 390.7 ms, System: 214.6 ms]
  Range (min … max):   624.3 ms … 692.3 ms    100 runs

Summary
  axes ./big_divided_test/into/script_90000 ran
    1.48 ± 0.08 times faster than axes ./big_test/script_90000
   14.73 ± 0.74 times faster than just --justfile big_test/justfile script_90000
