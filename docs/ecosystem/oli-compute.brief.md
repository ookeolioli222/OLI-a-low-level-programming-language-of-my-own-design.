# Oli-- Library 1: `oli.compute`
## GPU-first low-level AI / HPC stack

## Cel projektu

`oli.compute` ma być główną biblioteką obliczeniową Oli-- do:

- AI / machine learning
- treningu i inferencji modeli
- tensorów i macierzy
- GPU computing
- multi-GPU
- HPC
- DSP
- symulacji
- grafów obliczeniowych
- custom kernels
- quantization
- distributed training

GPU NIE jest dodatkiem planowanym „na później”.

Architektura biblioteki od pierwszej wersji musi zakładać równorzędne urządzenia:

```text
CPU
GPU
Multi-GPU
```

Pierwsza implementacja może uruchomić najpierw mały poprawny backend CPU, ale wszystkie typy, pamięć, tensory, grafy i API muszą być od początku zaprojektowane tak, aby GPU nie wymagało później przebudowy całej biblioteki.

---

# Ambicja

Nie kopiujemy NumPy, TensorFlow, PyTorch, JAX ani Tritona.

Celem jest zbudowanie systemu, który łączy najlepsze właściwości nowoczesnych frameworków, ale wykorzystuje przewagę języka systemowego Oli--:

```text
brak Pythona w hot path
brak obowiązkowego GC
jawna pamięć
jawne kopie CPU <-> GPU
jawne synchronizacje
zero-copy tam, gdzie sprzęt na to pozwala
first-class GPU kernels
compile-time specialization
autograd
graph compilation
kernel fusion
multi-GPU
distributed tensors
SIMD
Tensor Cores / matrix accelerators
mixed precision
quantization
profiling kosztów
```

Nie wolno pisać w dokumentacji, że `oli.compute` jest „szybsze od PyTorch/JAX/Triton”, dopóki benchmarki tego nie udowodnią.

Zamiast tego ustal konkretne cele i benchmark gates.

---

# Najważniejsza różnica

W typowych frameworkach użytkownik często operuje na wysokopoziomowym tensorze, a szczegóły:

```text
allocation
temporary buffer
copy
device transfer
synchronization
kernel launch
layout conversion
```

mogą być trudne do zauważenia.

W Oli-- te koszty mają być obserwowalne.

Przykład:

```bash
olic train.oli --compute-cost
```

może zwracać:

```text
graph: transformer_block

GPU allocations:          3
temporary buffers:        1
host -> device copies:    0
device -> host copies:    0
kernel launches:          7
kernels after fusion:     3
synchronization points:   1
peak VRAM:                812 MiB

Tensor Core kernels:      2
SIMD CPU kernels:         0
```

---

# Architektura

```text
oli.compute
│
├── core
│   ├── dtype
│   ├── shape
│   ├── layout
│   ├── stride
│   ├── device
│   └── memory
│
├── cpu
│   ├── scalar
│   ├── simd
│   ├── threads
│   └── kernels
│
├── gpu
│   ├── core
│   ├── memory
│   ├── stream
│   ├── event
│   ├── kernel
│   ├── graph
│   ├── nvidia
│   ├── amd
│   └── vulkan
│
├── tensor
├── matrix
├── linalg
├── kernels
├── fusion
├── graph
├── compiler
├── autograd
├── nn
├── functional
├── optim
├── distributed
├── quant
├── sparse
├── signal
├── stats
├── random
├── data
├── serialization
├── profiler
└── debug
```

---

# 1. DEVICE JEST CZĘŚCIĄ TYPU/OBIEKTU

Podstawowe urządzenia:

```text
cpu
gpu[0]
gpu[1]
```

Przykład:

```oli
let cpu = device.cpu();
let gpu = device.gpu(0);

let x = Tensor[f32].zeros([4096, 4096], on: gpu);
```

Nie może istnieć niejawne przenoszenie wielkich tensorów CPU -> GPU bez możliwości wykrycia tego przez programistę.

Jawna operacja:

```oli
let gx = x.to(gpu);
```

lub:

```oli
gpu.copy(gx, x);
```

Compiler/profiler powinien raportować rzeczywisty transfer.

---

# 2. GPU MEMORY MODEL

Pierwszoklasowe rodzaje pamięci:

```text
HostMemory
PinnedHostMemory
DeviceMemory
UnifiedMemory
SharedMemory
LocalMemory
ConstantMemory
```

Typy:

```text
DeviceBuffer[T]
PinnedBuffer[T]
UnifiedBuffer[T]
GpuView[T]
```

Przykład:

```oli
let weights = gpu.alloc[f16](256.mb);
defer gpu.free(weights);
```

Albo przez allocator:

```oli
let arena = GpuArena.new(gpu, 2.gb);

let a = Tensor[f16].zeros(
    [8192, 8192],
    allocator: arena
);
```

Wymagania:

- pooling VRAM
- suballocation
- aligned allocations
- reusable workspaces
- memory planning
- peak-memory analysis
- asynchronous allocation
- optional unified memory

---

# 3. ZERO-COPY I VIEW

TensorView/GpuView nie kopiuje danych.

```oli
let matrix = Tensor[f32].zeros([4096, 4096], on: gpu);

let row = matrix.view([100, :]);
```

`row` musi być jedynie opisem:

```text
base address
shape
stride
layout
device
```

Jeżeli dana operacja wymaga utworzenia contiguous buffer, API/compiler ma to ujawnić.

---

# 4. GPU STREAMS / QUEUES

Asynchroniczność jest elementem podstawowym.

```oli
let compute = gpu.stream();
let transfer = gpu.stream();

transfer.copy_async(device_data, host_data);

compute.launch(kernel, args);
```

Typy:

```text
GpuStream
GpuEvent
GpuFence
GpuQueue
```

Operacje:

```text
record
wait
synchronize
depends_on
```

Nie dodawaj globalnej synchronizacji bez potrzeby.

---

# 5. CUSTOM GPU KERNELS W OLI--

To jest jedna z najważniejszych funkcji całego ekosystemu.

Użytkownik nie powinien musieć przechodzić do:
- CUDA C++
- Python/Triton
- osobnego DSL

aby napisać kernel.

Koncepcja składni:

```oli
gpu kernel vector_add(
    out: gpu mutview f32,
    a: gpu view f32,
    b: gpu view f32
) {
    let i = gpu.global_id.x;

    if i < out.len {
        out[i] = a[i] + b[i];
    }
}
```

Bardziej zaawansowany kernel:

```oli
gpu kernel matmul_tile(
    c: gpu mutview f16,
    a: gpu view f16,
    b: gpu view f16
) uses gpu.shared, gpu.matrix {
    ...
}
```

Kernele muszą mieć first-class dostęp do:

```text
thread id
block/workgroup id
warp/wave id
shared memory
barriers
shuffle
ballot
atomics
matrix/tensor operations
vector loads
async copies
```

---

# 6. GPU COMPILER

Rozszerz OIR o GPU execution model.

Pipeline:

```text
Oli-- source
    ↓
OIR
    ↓
Compute IR
    ↓
GPU IR
    ↓
target lowering
```

Backendy:

```text
NVIDIA -> PTX / odpowiedni backend drivera
AMD    -> AMD GPU target
Vulkan -> SPIR-V compute
```

Nie uzależniaj semantyki `oli.compute` od CUDA.

Backend CUDA/NVIDIA jest implementacją Device API, a nie definicją modelu Oli--.

Tam, gdzie finalny sterownik GPU wykonuje obowiązkową kompilację/JIT do ISA konkretnej karty, traktuj to jako warstwę sprzętowo-sterownikową, a nie jako drugi język użytkownika.

---

# 7. GPU INTRINSICS

Przewidzieć:

```text
gpu.thread.x
gpu.thread.y
gpu.thread.z

gpu.block.x
gpu.block.y
gpu.block.z

gpu.warp_id
gpu.lane_id

gpu.barrier()
gpu.memory_fence()

gpu.shuffle()
gpu.ballot()

gpu.atomic.add()
gpu.atomic.cas()
```

---

# 8. SHARED MEMORY

Jawna pamięć współdzielona:

```oli
gpu kernel reduce(input: gpu view f32, output: gpu mutview f32) {
    shared let tile: [f32; 256];

    ...
}
```

Compiler zna:

```text
shared-memory footprint
bank alignment
threads/workgroup
register pressure
```

---

# 9. MATRIX / TENSOR ACCELERATORS

Abstrakcja nad sprzętowymi jednostkami macierzowymi.

Nie wiąż publicznego API tylko z nazwą NVIDIA Tensor Core.

Przykład:

```text
gpu.matrix.load
gpu.matrix.mma
gpu.matrix.store
```

Compiler mapuje je na właściwe mechanizmy targetu.

Obsługiwane typy docelowo:

```text
f32
tf32
f16
bf16
fp8
int8
int4
```

---

# 10. TENSOR CORE

Podstawowe typy:

```text
Tensor[T]
TensorView[T]
MutableTensorView[T]
TensorStorage
Shape
Stride
Layout
Device
Placement
ShardSpec
```

Tensor przechowuje/wiąże:

```text
storage
shape
strides
dtype
device
layout
ownership
allocator
alignment
```

---

# 11. OPERACJE TENSOROWE

```text
zeros
ones
full
arange
rand
randn

reshape
flatten
transpose
permute
slice
narrow
broadcast
expand

add
sub
mul
div
pow
sqrt
exp
log
sin
cos

sum
mean
min
max
argmin
argmax

dot
matmul
bmm
einsum

softmax
log_softmax

conv1d
conv2d
conv3d
```

---

# 12. EAGER + COMPILED

Dwa równorzędne tryby.

### Eager

```oli
let y = relu(x.matmul(w) + bias);
```

natychmiast wykonuje operacje.

### Compiled

```oli
let graph = compute.compile {
    let y = relu(x.matmul(w) + bias);
    yield y;
};
```

Compiler posiada wtedy cały graph i może przeprowadzić optymalizacje.

---

# 13. GRAPH COMPILER

Optymalizacje:

```text
operator fusion
kernel fusion
vertical fusion
horizontal fusion
constant folding
common subexpression elimination
dead op elimination
layout propagation
buffer reuse
memory planning
in-place conversion
copy elimination
sync elimination
kernel launch reduction
```

Graph compiler ma rozumieć CPU i GPU.

---

# 14. KERNEL FUSION

Przykład:

```oli
let y = relu(x * scale + bias);
```

naiwnie:

```text
mul kernel
add kernel
relu kernel
```

optimizer powinien mieć możliwość utworzenia:

```text
fused_mul_add_relu kernel
```

bez tworzenia dwóch pośrednich tensorów.

Compiler ma pozwalać sprawdzić rezultat:

```bash
oli compute explain model.oli
```

---

# 15. AUTOTUNER

Biblioteka ma posiadać:

```text
oli.compute.autotune
```

Dla krytycznych kernelów testować warianty:

```text
tile size
workgroup size
warp count
vector width
shared memory usage
pipeline stages
layout
```

i cache'ować najlepszy wariant dla:

```text
GPU model
dtype
shape family
kernel
driver/backend
```

Autotuning ma być opcjonalny i reprodukowalny.

---

# 16. AUTOGRAD

```text
requires_grad
grad
backward
detach
no_grad
```

Ale projektuj jednocześnie:

```text
forward-mode AD
reverse-mode AD
VJP
JVP
custom gradient
gradient checkpointing
```

Przykład:

```oli
let prediction = model.forward(input);
let loss = cross_entropy(prediction, labels);

loss.backward();
optimizer.step();
```

---

# 17. COMPILED AUTOGRAD

Compiler powinien móc zobaczyć jednocześnie:

```text
forward graph
backward graph
optimizer step
```

Dzięki temu może:

```text
fuse backward kernels
reuse buffers
avoid saved intermediates
checkpoint selected values
reduce VRAM
```

---

# 18. NEURAL NETWORK

Moduł:

```text
oli.compute.nn
```

Warstwy:

```text
Linear
Embedding

Conv1D
Conv2D
Conv3D

LayerNorm
RMSNorm
BatchNorm

Dropout

ReLU
GELU
SiLU
SwiGLU

Softmax
Attention
MultiHeadAttention

RNN
GRU
LSTM
```

AI-specific:

```text
RotaryEmbedding
KVCache
GroupedQueryAttention
MixtureOfExperts
```

---

# 19. TRANSFORMER / LLM PRIMITIVES

Dodać zoptymalizowane primitive'y:

```text
attention
flash-style attention
RMSNorm
RoPE
KV cache
paged KV cache
sampling
top-k
top-p
beam helpers
```

Nie kopiować implementacji zewnętrznych 1:1.

Budować własne implementacje na podstawie specyfikacji algorytmów i poprawnych testów.

---

# 20. MIXED PRECISION

Typy:

```text
f32
tf32
f16
bf16
fp8
```

API:

```text
AutoCast
GradScaler
PrecisionPolicy
```

Przykład:

```oli
with compute.precision(bf16) {
    let y = model(x);
}
```

---

# 21. QUANTIZATION

Moduł:

```text
oli.compute.quant
```

Obsługa:

```text
int8
int4
fp8

per-tensor
per-channel
per-group

symmetric
asymmetric
```

Dla LLM:

```text
weight-only quantization
activation quantization
KV-cache quantization
```

---

# 22. SPARSE

```text
COO
CSR
CSC
BSR
```

Operacje:

```text
sparse matmul
sparse-dense
embedding lookup
```

---

# 23. MULTI-GPU JEST CZĘŚCIĄ RDZENIA

Typ:

```text
DeviceMesh
```

Przykład:

```oli
let mesh = DeviceMesh[
    gpu(0),
    gpu(1),
    gpu(2),
    gpu(3)
];
```

---

# 24. DISTRIBUTED TENSOR

Tensor może posiadać jawny sharding:

```oli
let weights = Tensor[f16]
    .shape([32768, 32768])
    .shard(mesh, axis: 0);
```

Typy:

```text
ShardSpec
Replicated
Sharded
Partial
Mesh
Placement
```

Compiler musi znać rozmieszczenie danych.

---

# 25. COLLECTIVES

```text
all_reduce
all_gather
reduce_scatter
broadcast
scatter
gather
all_to_all
```

Backendy:
- NCCL-style backend dla NVIDIA
- RCCL-style backend dla AMD
- później własne/alternatywne transporty

---

# 26. DISTRIBUTED TRAINING

Zaplanować:

```text
Data Parallel
Distributed Data Parallel
Tensor Parallel
Pipeline Parallel
Sequence Parallel
Expert Parallel
FSDP-like sharding
ZeRO-like optimizer/state sharding
```

Nie kopiować publicznych API 1:1.

Stworzyć spójny model oparty o `DeviceMesh + ShardSpec`.

---

# 27. GPU P2P

Obsłużyć:

```text
GPU -> GPU copy
peer memory
NVLink-aware transfer where available
PCIe topology
```

Runtime powinien potrafić zobaczyć topologię:

```text
CPU socket
NUMA
PCIe
GPU
GPU interconnect
```

i wykorzystać ją podczas planowania.

---

# 28. ASYNCHRONOUS DISPATCH

Każda operacja GPU nie powinna domyślnie blokować CPU.

Zaprojektuj przyszły model:

```oli
let future = gpu.submit {
    y = model(x);
};

cpu.do_other_work();

let result = future.await();
```

Nie wymaga to ciężkiego async runtime.

---

# 29. COMMAND GRAPHS

Dodać:

```text
GpuGraph
CompiledGpuGraph
```

Powtarzalny ciąg operacji może zostać przechwycony i uruchamiany z minimalnym overheadem hosta.

---

# 30. MEMORY PLANNER

Dla compiled graph:

```text
lifetime analysis
buffer alias analysis
buffer reuse
temporary workspace reuse
in-place planning
VRAM peak minimization
```

Compiler powinien przed uruchomieniem móc oszacować peak VRAM.

---

# 31. DATA PIPELINE

Moduł:

```text
oli.compute.data
```

```text
Dataset
DataLoader
Sampler
Batch
Prefetch
Transform
```

Wymagania:
- pinned memory
- asynchronous prefetch
- CPU worker threads
- direct staging do GPU
- minimal copies

---

# 32. SERIALIZATION

Format modelu Oli--:

```text
.olimodel
```

Powinien przechowywać:
- tensors
- dtype
- layout
- metadata
- graph opcjonalnie
- version
- checksums

Importery/eksportery planowane dla:
- ONNX
- SafeTensors
- powszechnych formatów wag

---

# 33. PROFILER

Moduł:

```text
oli.compute.profiler
```

Mierzyć:

```text
CPU time
GPU time
kernel duration
kernel launch overhead
memory copy
memory bandwidth
VRAM usage
allocation
synchronization
occupancy
register usage
shared memory
```

Timeline:

```text
CPU             |---prepare---|----next batch----|
GPU compute          |MATMUL|ATTN|FFN|
GPU transfer      |H2D|              |H2D|
```

---

# 34. KERNEL INSPECTOR

```bash
oli compute kernel-info attention
```

Przykładowy wynik:

```text
target: RTX ...
threads/block: 256
registers/thread: 64
shared memory: 48 KiB
occupancy: 75%
vectorized loads: yes
matrix accelerator: yes
global loads: ...
```

---

# 35. DEBUG MODE

Dodać:

```text
bounds checking
NaN/Inf detection
race diagnostics where possible
device assertion
invalid memory access diagnostics
shape checking
```

Release może usuwać część kosztownych checków tam, gdzie jest to bezpieczne.

---

# 36. CPU BACKEND NADAL JEST WAŻNY

CPU:
- scalar
- SIMD
- multithreading
- NUMA awareness
- work stealing
- cache-aware kernels

Backend GPU nie może oznaczać zaniedbania CPU.

---

# 37. AI COMPILER

Docelowo:

```text
Oli-- model
   ↓
OIR
   ↓
Compute Graph IR
   ↓
Autograd transform
   ↓
Fusion
   ↓
Layout optimizer
   ↓
Memory planner
   ↓
Device partitioner
   ↓
Kernel compiler
   ↓
CPU/GPU executable
```

---

# 38. DYNAMIC SHAPES

Nie ograniczać biblioteki wyłącznie do statycznych wymiarów.

Rozróżnić:

```text
StaticShape
DynamicShape
BoundedDynamicShape
```

Compiler może specjalizować typowe shape'y i zachować fallback dla dynamicznych.

---

# 39. SHAPE SYSTEM

Rozważ shape information w systemie typów:

```oli
Tensor[f32, 128 x 768]
```

oraz dynamiczne:

```oli
Tensor[f32, ? x 768]
```

Nie wymuszaj compile-time shape wszędzie.

---

# 40. CUSTOM OPS

Użytkownik musi móc dodać własną operację:

```text
forward kernel
backward kernel
shape rule
dtype rule
device rule
```

bez patchowania frameworka.

---

# 41. NO PYTHON REQUIREMENT

Cały workflow musi działać bez Python runtime:

```bash
oli build
oli run
oli test
oli bench
```

Opcjonalny Python binding może istnieć później dla interoperacyjności, ale nie może być fundamentem `oli.compute`.

---

# 42. INTEROP

Przewidzieć FFI/import z:
- CUDA driver API
- vendor GPU libraries
- BLAS
- cuBLAS-like libraries
- cuDNN-like libraries
- ROCm equivalents
- Vulkan

Jednocześnie własne kernely Oli-- mają być pełnoprawną alternatywą.

---

# 43. WŁASNE KERNELE VS VENDOR LIBRARIES

Dwa tryby:

```text
native Oli kernel
vendor optimized backend
```

Planner/autotuner może wybrać szybszy wariant.

Użytkownik może wymusić:

```text
backend = oli
```

dla całkowicie własnego stacka.

---

# 44. OBSERWOWALNOŚĆ

Każdy Tensor powinien pozwalać sprawdzić:

```text
shape
stride
layout
dtype
device
ownership
allocator
sharding
contiguous
memory bytes
```

Żadnej „magii”, której nie można zobaczyć.

---

# 45. PERFORMANCE CONTRACT

Dla krytycznych API dokumentuj:

```text
allocation behavior
copy behavior
synchronization behavior
device behavior
complexity
```

Przykład:

```text
Tensor.view
ALLOC: no
COPY: no
SYNC: no
DEVICE TRANSFER: no
```

---

# 46. BENCHMARK SUITE

Porównywać co najmniej z:

```text
PyTorch
JAX
Triton kernels
NumPy CPU
vendor BLAS
```

Kategorie:

```text
elementwise
reduction
softmax
normalization
matmul
batched matmul
convolution
attention
transformer block
optimizer step
training step
LLM token generation
```

---

# 47. METRYKI

Nie tylko czas.

Mierzyć:

```text
latency
throughput
tokens/sec
samples/sec
TFLOPS utilization
memory bandwidth
VRAM
peak allocations
kernel launches
host overhead
compile time
binary size
energy where measurable
```

---

# 48. PERFORMANCE GATES

Każda duża optymalizacja musi posiadać:

```text
correctness test
benchmark before
benchmark after
memory comparison
```

Nie merge'uj optymalizacji, która przyspiesza jeden benchmark kosztem poważnej regresji w innych bez udokumentowania kompromisu.

---

# 49. CORRECTNESS

Numerical testing:
- tolerances według dtype
- CPU/GPU differential tests
- reference implementations
- randomized tests
- gradient checks
- determinism tests

---

# 50. ETAPY IMPLEMENTACJI

## Phase 0 — Architecture

Zaprojektuj jednocześnie:
- Device
- CPU
- GPU
- Memory
- Tensor
- Streams
- Kernel abstraction
- Compute IR

GPU musi być obecne w typach i IR już tutaj.

## Phase 1 — Minimal CPU + GPU foundation

CPU:
- scalar tensors

GPU:
- enumerate device
- allocate/free VRAM
- host/device transfer
- stream/event
- uruchomienie minimalnego kernela

Pierwszy GPU milestone:

```text
GPU vector_add
```

## Phase 2 — Tensor runtime

- Tensor
- TensorView
- layouts
- strides
- broadcasting
- CPU kernels
- GPU kernels

## Phase 3 — GPU kernel language

- threads
- workgroups
- shared memory
- barriers
- atomics
- vector loads
- GPU IR

## Phase 4 — Fast math

- reductions
- softmax
- normalization
- matmul
- convolution
- autotuning
- matrix accelerators

## Phase 5 — Graph compiler

- graph capture
- fusion
- memory planner
- command graph
- compiled execution

## Phase 6 — Autograd + NN

- reverse AD
- compiled backward
- layers
- optimizers

## Phase 7 — Transformer stack

- attention
- fused attention
- RoPE
- RMSNorm
- KV cache
- transformer block

## Phase 8 — Multi-GPU

- DeviceMesh
- sharded tensor
- collectives
- data parallel
- tensor parallel

## Phase 9 — Advanced training

- distributed optimizer states
- checkpointing
- mixed precision
- FP8
- quantization

## Phase 10 — Additional GPU vendors

- AMD backend
- Vulkan compute backend
- architecture-specific optimization

---

# 51. CLAUDE CODE — OBOWIĄZKOWY DESIGN REVIEW

Przed każdą implementacją odpowiedz w dokumentacji:

1. Czy operacja działa na CPU i GPU?
2. Gdzie fizycznie znajduje się pamięć?
3. Czy powstaje kopia?
4. Czy powstaje temporary buffer?
5. Czy operacja synchronizuje CPU/GPU?
6. Czy można ją sfuzować?
7. Czy działa asynchronicznie?
8. Jak zachowuje się w multi-GPU?
9. Jak można użyć SIMD/matrix accelerators?
10. Jak będzie benchmarkowana względem istniejących frameworków?
11. Czy API wykorzystuje przewagi Oli-- zamiast kopiować API Pythona?
12. Czy programista może zejść poziom niżej i zastąpić kernel własnym?

---

# 52. GŁÓWNY CEL ARCHITEKTONICZNY

`oli.compute` ma pozwalać jednej osobie przejść bez zmiany języka od:

```oli
let y = model(x);
```

do:

```oli
gpu kernel custom_attention(...) {
    // ręcznie kontrolowany kernel GPU
}
```

i jeszcze niżej do architekturowych intrinsiców GPU.

To jest kluczowa przewaga, do której powinien dążyć projekt.

Nie budujemy tylko wysokopoziomowego frameworka AI.

Budujemy:

```text
Tensor library
+
AI framework
+
autograd engine
+
graph compiler
+
GPU programming model
+
kernel compiler
+
multi-GPU runtime
+
profiler
```

jako jeden spójny ekosystem Oli--.

---

# 53. ZASADA KOŃCOWA DLA CLAUDE CODE

Przy każdej decyzji pytaj:

> Czy można zaprojektować tę część tak, aby użytkownik Oli-- miał wygodę frameworka AI, ale jednocześnie zachował kontrolę programisty systemowego nad pamięcią, urządzeniem, synchronizacją i wygenerowanym kernelem?

Jeżeli odpowiedź brzmi „nie”, przeanalizuj projekt ponownie przed implementacją.
