This document outlines the academic foundations, formal methods, and microarchitectural research utilized in the design, optimization, and verification pipeline of the **Briev** compiler.

---

## 1. Axiomatic Semantics & Formal Program Verification
*Relates to: `src/assertion_verify.rs`, `src/symbolic.rs`, `src/proof_engine.rs`*

Briev implements a compile-time program prover to verify that operations with `sig -> true` guarantees are mathematically incapable of failing or returning false [1].

*   **Hoare, C. A. R.** (1969). *An axiomatic basis for computer programming*. *Communications of the ACM*, 12(10), 576–580.
    *   *System Correlation:* Establishes the mathematical foundation for **preconditions and postconditions** (Hoare Triples: $P \{Q\} R$) used in Briev's contract-first transaction design [1].
*   **Filliâtre, J. C., & Paskevich, A.** (2013). *Why3 — Where program verification meets systems*. In *International Conference on Verification, Model Checking, and Abstract Interpretation* (pp. 125-128). Springer, Berlin, Heidelberg.
    *   *System Correlation:* Guides the architecture of Briev’s intermediate symbolic executor, showing how program paths can be converted into verification conditions (VCs) for SMT-solving frameworks [1, 1].
*   **Barnes, J.** (2012). *SPARK: The Proven Path to High Integrity Software*. Altran Praxis.
    *   *System Correlation:* Establishes the precedent for **Absence of Run-Time Errors (AoRTE)** proving, used by Briev to safely strip out physical hardware safety/boundary checks once spatial safety is statically proven [1].

---

## 2. Concolic Testing & Automated Path Generation
*Relates to: `src/fuzzing/concolic.rs`, `src/fuzzing/ast_generator.rs`*

Briev combines symbolic execution with concrete execution (concolic testing) to automatically generate inputs that exercise different code coverage paths [1].

*   **Godefroid, P., Klarlund, N., & Sen, K.** (2005). *DART: Directed Automated Random Testing*. In *Proceedings of the 2005 ACM SIGPLAN Conference on Programming Language Design and Implementation* (pp. 213-223).
    *   *System Correlation:* The seminal paper introducing **Concolic Testing** (Concrete + Symbolic). Brievly implements this path-constraint collection model in `concolic.rs` to generate targeted fuzzer inputs [1].
*   **Sen, K., Marinov, D., & Agha, G.** (2005). *CUTE: A Concolic Unit Testing Engine for C*. In *Proceedings of the 10th European Software Engineering Conference Held Jointly with 13th ACM SIGSOFT International Symposium on Foundations of Software Engineering* (pp. 263-272).
    *   *System Correlation:* Guides the constraint-solving pipeline, mapping logical negation on guard branches to target inputs.

---

## 3. Compiler Representation, SSA Form, & Stack Promotion
*Relates to: `src/analysis/cross_reference.rs`, `src/target_spec/mod.rs`*

To prevent manual variable allocation errors, the Briev compiler relies on standard stack variables promoted to registers via LLVM's `SROA` and `mem2reg` [1].

*   **Cytron, R., Ferrante, J., Rosen, B. K., Wegman, M. N., & Zadeck, F. K.** (1991). *Efficiently computing static single assignment form and the control dependence graph*. *ACM Transactions on Programming Languages and Systems (TOPLAS)*, 13(4), 451–490.
    *   *System Correlation:* The landmark work defining the **Dominance Frontier** algorithm. It details how memory `alloca` structures are mathematically translated into register-resident `phi` nodes [1, 1], which is the foundational optimization used when Briev transitions to stack-promoted execution state.

---

## 4. Superword Level Parallelism & Vectorization Hazards
*Relates to: `src/analysis/mod.rs` (SLP Hazard Estimator)*

Briev evaluates register pressure and microarchitectural bottlenecks to prevent vectorization overhead from degrading scalar execution.

*   **Larsen, S., & Amarasinghe, S.** (2000). *Exploiting superword level parallelism with multimedia instruction sets*. In *Proceedings of the ACM SIGPLAN 2000 Conference on Programming Language Design and Implementation* (pp. 145-156).
    *   *System Correlation:* Defines **SLP Vectorization**. It outlines the exact instruction-packing and register-shuffling constraints that Briev's hazard estimator evaluates to avoid microarchitectural pipeline stalls [1].
*   **Mendis, C., & Amarasinghe, S.** (2018). *goSLP: globally optimized superword level parallelism framework*. *Proceedings of the ACM on Programming Languages*, 2(OOPSLA), 1–28.
    *   *System Correlation:* Demonstrates the performance penalty of local vector packing. It provides the microarchitectural basis for Briev's **Arithmetic-to-Shuffle Ratio (ASR)** calculation.

---

## 5. Equality Saturation & Algebraic Cost Extraction
*Relates to: `src/analysis/region.rs`, `src/analysis/transition_graph.rs`*

Briev detects linear transaction chains, composing their bodies to fold state transitions into closed-form representations [1, 1].

*   **Willsey, M., Nandi, C., Wang, Y. R., Flatt, O., Tatlock, Z., & Panchekha, P.** (2021). *egg: Fast and extensible equality saturation*. *Proceedings of the ACM on Programming Languages*, 5(POPL), 1–29.
    *   *System Correlation:* Establishes the mathematics of **E-Graphs** and **Equality Saturation**. It serves as the model for optimizing and rewriting composed transaction equations [1].
*   **VanHattum, A., Nigam, R., Lee, V. T., Bornholt, J., & Sampson, A.** (2021). *Vectorization for digital signal processors via equality saturation*. In *Proceedings of the 26th ACM International Conference on Architectural Support for Programming Languages and Operating Systems* (pp. 874-886).
    *   *System Correlation:* Demonstrates how structural equality saturation can resolve the boundary between optimal vector pipelines and scalar fallbacks.

---

## 6. Zero-Copy, Lock-Free Inter-Process Communication
*Relates to: `src/ffi/metropolitan.rs`, `src/ffi/metro_cli.rs`*

The Metropolitan FFI acts as a high-performance shared-memory pipeline, bypassing traditional IPC overhead through a specialized 32-byte header protocol [1, 1].

*   **Herlihy, M.** (1991). *Wait-free synchronization*. *ACM Transactions on Programming Languages and Systems (TOPLAS)*, 13(1), 124-149.
    *   *System Correlation:* Establishes the foundational theory of lock-free, wait-free data structures using atomic **Compare-And-Swap (CAS)** operations [1]. This drives the concurrency control inside the 32-byte `Metropolitan` shared memory header [1].
*   **Thompson, M., Farley, D., Barker, M., Gee, P., & Stewart, A.** (2011). *Disruptor: High performance alternative to bounded queues for exchanging data between concurrent threads*. *Technical Paper, LMAX Exchange*.
    *   *System Correlation:* Demonstrates the extreme cost of cache-line invalidation and lock contention in traditional queues. This informs the design of Briev's zero-copy, memory-mapped shared regions [1].

---

## 7. Synchronous Reactive Programming & Adaptive Scheduling
*Relates to: `src/scheduler.rs`, `src/reactor.rs`*

Briev utilizes a highly specialized, multi-rate polling scheduler to manage heterogeneous execution frequencies with minimal overhead [1].

*   **Halbwachs, N., Caspi, P., Raymond, P., & Pilaud, D.** (1991). *The synchronous dataflow programming language LUSTRE*. *Proceedings of the IEEE*, 79(9), 1305-1320.
    *   *System Correlation:* Establishes the formal clock-calculus and temporal semantics for multi-rate variables and multi-frequency reactive event loops [1].
*   **Berry, G., & Gonthier, G.** (1992). *The Esterel synchronous programming language: Design, semantics, implementation*. *Science of Computer Programming*, 19(2), 87-152.
    *   *System Correlation:* Establishes the reactive compilation model [1], proving how asynchronous, trigger-driven program states can be compressed statically into deterministic, high-efficiency, flat sequential code [1].