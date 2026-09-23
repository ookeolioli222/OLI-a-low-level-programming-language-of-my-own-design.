# oli.compute — GPU-first tensor / AI / HPC stack (planned)

Condensed roadmap of the full brief. Goal: one library covering tensors,
autograd, NN, graph compilation, custom GPU kernels and multi-GPU — built on
Oli--'s advantages (no Python in the hot path, no GC, explicit memory,
explicit CPU↔GPU copies and syncs, zero-copy views, observable cost) rather
than cloning NumPy / PyTorch / JAX / Triton.

## Non-negotiable principles
- **Device is part of the type/object.** No implicit large CPU→GPU transfer;
  `x.to(gpu)` is explicit and the profiler reports the real transfer.
- **Cost is observable.** `olic train.oli --compute-cost` reports allocations,
  temporary buffers, host/device copies, kernel launches, kernels after fusion,
  sync points, peak VRAM, tensor-core vs SIMD kernels.
- **Zero-copy views** describe (base, shape, stride, layout, device); any op
  that must materialize a contiguous buffer says so.
- **GPU is not "later".** Types, memory, tensors, graphs and IR are designed
  device-agnostic from day one; the first backend may be a small correct CPU one.
- **No mandatory Python.** `oli build/run/test/bench` is the whole workflow.
- **Own kernels are first-class**, written in Oli-- (`gpu kernel …`), with
  first-class access to thread/block/warp ids, shared memory, barriers,
  shuffle/ballot, atomics, matrix/tensor ops, vector loads, async copies — and
  the programmer can always drop below a high-level op to a hand-written kernel.

## Architecture (subsystems)
core (dtype, shape, layout, stride, device, memory) · cpu (scalar, simd,
threads, kernels) · gpu (core, memory, stream, event, kernel, graph, nvidia,
amd, vulkan) · tensor · matrix · linalg · kernels · fusion · graph · compiler ·
autograd · nn · functional · optim · distributed · quant · sparse · signal ·
stats · random · data · serialization (`.olimodel`, ONNX/SafeTensors import) ·
profiler · debug.

## Compiler / IR
Extend OIR with a compute execution model: `OIR → Compute IR → GPU IR → target
lowering`. Backends are Device-API implementations (NVIDIA→PTX, AMD, Vulkan→
SPIR-V), never the definition of the model; driver JIT to card ISA is treated
as a hardware layer, not a second user language. Graph compiler: fusion
(vertical/horizontal), constant folding, CSE, dead-op elimination, layout
propagation, buffer reuse, memory planning, copy/sync elimination.

## Phases
0 architecture (Device/CPU/GPU/Memory/Tensor/Streams/Kernel/Compute-IR
together) · 1 minimal CPU+GPU foundation (`GPU vector_add`) · 2 tensor runtime
(views, strides, broadcasting) · 3 GPU kernel language (threads, shared mem,
barriers, atomics, GPU IR) · 4 fast math (reductions, softmax, norm, matmul,
conv, autotuning, matrix accelerators) · 5 graph compiler (capture, fusion,
memory planner, command graph) · 6 autograd + NN · 7 transformer stack
(attention, flash-style, RoPE, RMSNorm, KV cache) · 8 multi-GPU (DeviceMesh,
sharded tensor, collectives, data/tensor parallel) · 9 advanced training
(distributed optimizer state, checkpointing, mixed precision, FP8, quant) ·
10 more vendors (AMD, Vulkan).

## Performance discipline
No "faster than PyTorch/JAX/Triton" claim without benchmarks. Every major
optimization needs a correctness test + before/after benchmark + memory
comparison. Metrics beyond time: throughput, tokens/s, TFLOPS utilization,
bandwidth, VRAM, launches, host overhead, compile time, binary size.

## Design-review gate (before implementing any op)
CPU and GPU? where is the memory? copy? temporary? sync? fusable? async?
multi-GPU behavior? SIMD/matrix accelerators? benchmark plan? uses Oli--'s
strengths instead of copying a Python API? can the programmer replace the kernel?
