# Oli-- Security Research Platform
## `oli.sec` — master architecture and Claude Code development plan
### Version: Security Architecture V2 / September 2026

---

# 0. MISSION

`oli.sec` is not supposed to be a small cryptography helper library and it is not supposed to be a clone of OpenSSL, Scapy, libpcap, Burp extensions, Frida, Ghidra, AFL++, or any single existing security project.

The goal is to build a native security-research ecosystem for Oli-- that combines:

- packet capture and packet analysis
- high-performance networking
- protocol parsing
- cryptography
- modern authentication testing
- web application security testing
- API security testing
- bug bounty workflows
- fuzzing
- reverse engineering
- dynamic instrumentation
- binary analysis
- emulation
- crash triage
- malware-analysis primitives
- mobile application analysis
- browser-security analysis
- cloud/container/Kubernetes configuration analysis
- software supply-chain analysis
- firmware and embedded analysis
- AI/LLM/agent security testing
- security telemetry
- reproducible evidence generation

The platform must be useful for authorized security research, defensive testing, CTF/lab environments, software verification, bug bounty programs, and security engineering.

It must also exploit the fundamental advantages of Oli--:

```text
native code
no mandatory VM
no mandatory GC
explicit memory ownership
zero-copy views
explicit allocators
native SIMD
native GPU access through oli.compute
direct syscalls when appropriate
first-class low-level memory
first-class protocol layouts
first-class binary layouts
own compiler IR
own machine backend
```

The design objective is not:

> "Copy every security tool into one package."

The objective is:

> "Create a common low-level substrate that makes previously separate security workflows composable."

For example:

```text
capture packet
      ↓
parse protocol
      ↓
link request to process
      ↓
link process to executed basic blocks
      ↓
link basic blocks to binary functions
      ↓
link network input to dataflow
      ↓
fuzz the same parser
      ↓
produce reproducible evidence
```

That cross-layer correlation should become one of the defining features of `oli.sec`.

---

# 1. NON-NEGOTIABLE DESIGN RULES

Claude Code must continuously evaluate every subsystem using the following questions.

## 1.1 Originality review

Before implementing a major component ask:

1. Are we merely cloning an existing tool's API?
2. Can Oli-- expose the underlying machine/security state more clearly?
3. Can the component work zero-copy?
4. Can the component preserve provenance of every byte?
5. Can we correlate this data with another security subsystem?
6. Can we make the result deterministic and replayable?
7. Can the library expose why it reached a security conclusion?
8. Can the programmer drop down to raw bytes/memory/frames/instructions?
9. Can this work without Python?
10. Can this work without a mandatory heavy runtime?

## 1.2 Evidence-first rule

A security finding is not simply:

```text
"possible vulnerability"
```

A finding should contain structured evidence:

```text
Finding
├── class
├── confidence
├── scope
├── request/input
├── response/output
├── state before
├── state after
├── timing
├── affected object
├── parser interpretation
├── trace references
├── minimized reproducer
└── mitigation notes
```

## 1.3 Reproducibility rule

Every active test should optionally produce a replay bundle.

Example:

```text
finding.olisec/
├── finding.json
├── traffic.pcapng
├── requests.har
├── replay.oli
├── trace.otrace
├── binary-metadata.json
├── screenshots/
└── checksums.txt
```

## 1.4 No magic

If a subsystem:

- copies data
- allocates memory
- opens a socket
- creates threads
- attaches a probe
- pauses a process
- performs a network request
- synchronizes
- launches GPU work

the operation must be observable.

---

# 2. SAFETY AND AUTHORIZED-TESTING MODEL

Make authorization and scope a first-class concept.

This is useful in professional pentesting and bug bounty because accidental out-of-scope testing is a real operational problem.

Create:

```text
oli.sec.scope
```

Core types:

```text
Scope
ScopeTarget
ScopeRule
ScopeToken
TestBudget
RatePolicy
EvidencePolicy
```

Concept:

```oli
let scope = Scope {
    hosts: ["lab.example"],
    ports: [443],
    protocols: [https],
    requests_per_second: 5,
    destructive_tests: false
};

let session = sec.test.open(scope);
```

Every active networking component receives the scope object.

```oli
web.test(session, target);
api.test(session, target);
net.probe(session, target);
```

An operation leaving scope should fail before a packet is transmitted.

Provide modes:

```text
observe
safe-active
lab
explicit-destructive
```

The default must be:

```text
safe-active
```

Do not make the library useless for low-level research, but make risky behavior explicit.

---

# 3. TOP-LEVEL ARCHITECTURE

```text
oli.sec
│
├── scope
├── bytes
├── binary
├── codec
├── crypto
├── net
├── capture
├── packet
├── protocol
├── trace
├── instrument
├── reverse
├── disasm
├── lift
├── decompile
├── emulate
├── fuzz
├── crash
├── taint
├── symbolic
├── static
├── web
├── browser
├── api
├── auth
├── mobile
├── cloud
├── container
├── supply
├── firmware
├── hardware
├── ai
├── policy
├── evidence
├── profiler
└── lab
```

Do not implement these as one monolithic binary.

Each major subsystem must be usable independently.

---

# 4. COMMON SECURITY DATA MODEL

One problem with security tooling is that every tool has its own representation.

Oli-- should create a common model.

## 4.1 Universal `Artifact`

```text
Artifact
```

An artifact can represent:

```text
packet
stream
HTTP request
HTTP response
file
binary
memory range
process
thread
function
instruction
certificate
token
API object
mobile package
container image
firmware
AI prompt
AI tool call
crash
```

Each artifact has:

```text
id
type
origin
timestamp
hash
parent
provenance
metadata
security labels
```

## 4.2 Provenance graph

Create:

```text
ArtifactGraph
```

Example:

```text
Ethernet frame
     ↓
IPv4 packet
     ↓
TCP segment
     ↓
TCP stream bytes
     ↓
TLS record
     ↓
HTTP/2 stream
     ↓
GraphQL request
     ↓
server process
     ↓
parser function
```

The same bytes should remain traceable through transformations.

This is a central innovation.

---

# 5. BYTE PROVENANCE

Create:

```text
oli.sec.bytes
```

Core types:

```text
ByteView
MutableByteView
BitView
ByteOrigin
ByteSpan
ByteMap
ByteTransform
```

A parser should optionally retain:

```text
source offset
decoded offset
transformation
```

Example:

```text
URL encoded input
%2e%2e%2f

↓ percent decode

../

↓ path normalization

parent-directory component
```

The framework should be able to explain that transformation.

This becomes useful for:

- parser differentials
- Unicode normalization
- URL parsing
- archive parsing
- binary formats
- request transformations
- reverse engineering

---

# 6. `PACKET REALITY` — MULTI-VANTAGE NETWORK CAPTURE

This should become one of the most distinctive components of Oli--.

Normal packet capture often shows only one view of networking.

Modern systems transform packets through:

```text
NIC
hardware offloads
XDP
driver
GRO
network stack
firewall
TCP
TLS
socket
application
```

`oli.sec.capture` should support multiple simultaneous observation points.

Architecture:

```text
WIRE
 ↓
NIC
 ↓
XDP
 ↓
TCX ingress
 ↓
network stack
 ↓
socket
 ↓
syscall
 ↓
TLS library
 ↓
application
```

Call this feature:

# Packet Reality

API concept:

```oli
let capture = PacketReality.start {
    interface: "eth0",

    planes: [
        xdp,
        tc_ingress,
        socket,
        tls_plaintext
    ]
};
```

---

# 7. PACKET REALITY: CAPTURE BACKENDS

Support:

```text
libpcap
AF_PACKET
AF_XDP
eBPF/XDP
TCX/eBPF
socket tracing
io_uring ZC Rx
external TAP import
```

Optional high-performance backend later:

```text
DPDK
```

Do not require DPDK for normal use.

---

# 8. AF_XDP BACKEND

Use AF_XDP for high-throughput Linux packet processing.

Plan support for:

```text
XDP_SKB
XDP_DRV
XDP_ZEROCOPY
shared UMEM
RX/TX rings
fill/completion rings
multi-buffer packets
XDP metadata
```

Expose drop accounting.

```oli
capture.stats();
```

Result:

```text
received
dropped
invalid descriptors
ring overflow
user processing backlog
zero-copy active
```

Do not silently claim a complete capture when packets were dropped.

---

# 9. XDP METADATA

When hardware/driver supports it, collect:

```text
hardware receive timestamp
RSS hash
VLAN tag
queue id
```

Represent this separately from packet bytes.

Example:

```text
PacketMeta
├── rx_hw_timestamp
├── rss_hash
├── vlan
├── rx_queue
└── capture_plane
```

---

# 10. OFFLOAD-AWARE CAPTURE

Add explicit awareness of:

```text
checksum offload
TSO
GSO
GRO
LRO
VLAN offload
RSS
RPS
XPS
```

The capture layer must record enabled NIC features.

Command concept:

```bash
oli sec capture explain eth0
```

Example result:

```text
RX checksum offload: ON
TX checksum offload: ON
GRO: ON
GSO: ON
TSO: ON

Possible capture effects:
- receive frames may appear coalesced
- locally transmitted packets may contain partial checksums
- observed segment sizes may differ from wire representation
```

Do not automatically disable host features.

Offer an explicit controlled lab profile.

---

# 11. PACKET EXPANSION / RECONSTRUCTION

A distinctive feature:

```text
capture.reconstruct_wire_view()
```

When a host capture contains a GSO/TSO "superpacket", the library should attempt to construct an explanatory representation of the likely segmented wire packets.

Important:

- mark reconstructed packets as synthetic
- never present them as captured ground truth
- retain original packet
- retain offload metadata

Represent:

```text
CapturedHostPacket
SyntheticWireSegments
```

This directly addresses the common problem where capture tools appear to "show fewer/bigger packets".

---

# 12. MULTI-PLANE CORRELATION

Correlate observations from:

```text
XDP
TCX
socket
TLS plaintext
application trace
```

Use:

```text
timestamps
5-tuple
TCP sequence ranges
payload hashes
process/socket association
```

Result:

```text
FlowTimeline
```

Example:

```text
12:00:00.001 XDP          1514 B
12:00:00.002 TCP stack    1460 B payload
12:00:00.003 TLS record   1412 B
12:00:00.004 HTTP parser  GET /api/user
```

This allows researchers to see where transformations occur.

---

# 13. CAPTURE LOSS MAP

Instead of reporting only "dropped packets", maintain:

```text
CaptureLossMap
```

Per plane:

```text
NIC
XDP
TC
AF_XDP
userspace queue
parser
storage
```

Expose uncertainty.

Example:

```text
XDP observed: 2,103,991
AF_XDP delivered: 2,103,612
userspace parsed: 2,103,612

loss between XDP and userspace: 379
```

---

# 14. HARDWARE TIMESTAMPING

When supported, record hardware timestamps.

Useful for:

```text
race-condition research
latency analysis
packet-order analysis
side-channel experiments
distributed capture correlation
```

Do not rely only on userspace wall-clock timestamps.

---

# 15. PCAPNG++

Support normal:

```text
PCAP
PCAPNG
```

Add custom Oli-- metadata blocks while remaining compatible where possible.

Store:

```text
capture plane
offload state
process id
socket id
hardware timestamp
parser provenance
test id
scope id
```

TLS session secrets must never be embedded by default.

If the user explicitly elects to attach them, warn that the capture becomes sensitive.

---

# 16. TLS OBSERVABILITY

For applications under authorized testing, support several safe mechanisms:

```text
TLS key-log import
TLS key-log export integration
application-side plaintext tracing
library-level tracing
```

Do not require breaking encryption.

If session secrets are available legitimately, associate:

```text
encrypted record
↔
plaintext message
```

without discarding the original encrypted traffic.

---

# 17. eBPF OBSERVABILITY

Module:

```text
oli.sec.trace.ebpf
```

Support:

```text
XDP
TCX
socket filter
tracepoint
raw tracepoint
fentry/fexit
kprobe/kretprobe
uprobe/uretprobe
USDT
LSM observation hooks
```

Initial implementation may use libbpf interoperability.

Long-term Oli-- can generate compatible BPF programs directly.

---

# 18. CROSS-LAYER PROCESS CORRELATION

Create:

```text
SocketProcessMap
```

Map flows to:

```text
PID
TID
process name
binary
container/cgroup
socket fd
```

Then:

```text
network request
↔
process
↔
binary image
↔
function trace
```

This is particularly valuable when analyzing local services and test environments.

---

# 19. HARDWARE EXECUTION TRACING

Create abstraction:

```text
oli.sec.trace.hw
```

Backends:

```text
Intel Processor Trace
platform performance counters
ARM trace mechanisms later
```

Use hardware traces for:

- low-overhead execution coverage
- crash reconstruction
- black-box fuzz feedback
- reverse engineering
- performance/security correlation

---

# 20. DYNAMIC INSTRUMENTATION

Module:

```text
oli.sec.instrument
```

Long-term goal: Frida-like primitives in native Oli--.

Capabilities:

```text
attach process
enumerate modules
resolve symbols
hook function entry/return
observe arguments
observe return value
trace basic blocks
trace calls
trace instructions
scan memory
watch memory access
```

Keep active mutation APIs separate from observation APIs.

---

# 21. INSTRUMENTATION MODES

```text
observe
rewrite
sandbox
```

`observe`:

```text
read-only tracing
```

`rewrite`:

```text
explicitly modify execution
```

`sandbox`:

```text
run the target in a controlled laboratory environment
```

Default:

```text
observe
```

---

# 22. TRACE IR

Define:

```text
TraceIR
```

Events:

```text
Call
Return
Branch
Syscall
MemoryRead
MemoryWrite
Exception
ModuleLoad
ModuleUnload
ThreadStart
ThreadStop
```

Use compact binary encoding.

Allow streaming processing without storing every event.

---

# 23. REVERSE ENGINEERING CORE

Create:

```text
oli.sec.reverse
```

Architecture:

```text
loader
 ↓
decoder
 ↓
instruction model
 ↓
lifting
 ↓
Security IR
 ↓
CFG
 ↓
dataflow
 ↓
type recovery
 ↓
high-level analysis
```

---

# 24. BINARY FORMATS

First class:

```text
ELF
PE/COFF
Mach-O
WASM
```

Later:

```text
DEX
ART/OAT
UEFI PE images
firmware containers
```

Parse:

```text
headers
sections
segments
imports
exports
relocations
symbols
TLS
resources
debug metadata
load configuration
security metadata
```

---

# 25. DEBUG INFORMATION

Support:

```text
DWARF
PDB
ELF symbols
PE symbols
source maps where available
```

Use debug info to improve:

```text
function recovery
type recovery
crash reports
source mapping
```

---

# 26. DISASSEMBLER

Module:

```text
oli.sec.disasm
```

Architectures:

```text
x86
x86-64
AArch64
ARM
RISC-V later
```

Must return structured instructions, not strings only.

```text
Instruction
├── address
├── bytes
├── opcode
├── operands
├── implicit reads
├── implicit writes
├── flags read
├── flags written
└── control-flow effect
```

---

# 27. SECURITY IR

Create a dedicated analysis representation:

# SIR — Security Intermediate Representation

Do not simply reuse compiler OIR without considering reverse-engineering requirements.

SIR needs:

```text
unknown values
partial types
indirect branches
memory aliases
opaque calls
undefined behavior in foreign binaries
architecture flags
side effects
```

Core concepts:

```text
Load
Store
Call
Branch
Phi
Syscall
Intrinsic
Unknown
```

---

# 28. BINARY LIFTING

Lift machine code into SIR.

Advantages:

- architecture-independent security analysis
- taint analysis
- symbolic execution
- decompiler
- function similarity
- patch diffing

Each lifted instruction must retain:

```text
machine address
original bytes
source instruction
```

---

# 29. CONTROL-FLOW GRAPH

Produce:

```text
BasicBlock
FunctionCFG
CallGraph
InterproceduralGraph
```

Support uncertain edges.

Do not pretend indirect control flow is fully resolved when it is not.

Represent confidence.

---

# 30. TYPE RECOVERY

Infer:

```text
integers
pointers
arrays
structures
function signatures
calling conventions
```

Use:

```text
dataflow
debug information
API signatures
dynamic traces
```

Allow merging static and dynamic evidence.

---

# 31. DECOMPILER

Long-term module:

```text
oli.sec.decompile
```

Output should prioritize:

```text
semantic clarity
security dataflow
memory access
```

not merely pretty C-like source.

Allow alternate views:

```text
pseudo-code
SIR
dataflow
memory view
call graph
```

---

# 32. DIFFERENTIAL DECOMPILATION

Novel idea:

Compare:

```text
static prediction
vs
dynamic trace
```

If static analysis predicts:

```text
possible branch A/B/C
```

but observed traces only contain A/B, keep both facts.

This avoids falsely treating dynamic traces as complete.

---

# 33. FUNCTION FINGERPRINTING

Create semantic function hashes based on:

```text
normalized CFG
constants
API calls
SIR operations
strings
```

Use for:

```text
library recognition
version comparison
binary diffing
patch analysis
duplicate-code identification
```

---

# 34. PATCH DIFF

Module:

```text
reverse.diff
```

Compare two binary versions.

Output:

```text
added functions
removed functions
changed CFGs
changed constants
new bounds checks
new error paths
new cryptographic calls
new authorization checks
```

This is useful for defensive vulnerability research without requiring source code.

---

# 35. MITIGATION ANALYZER

Inspect binary/OS mitigations.

Linux/ELF:

```text
NX
PIE
RELRO
stack canary
FORTIFY signals
CET/IBT where represented
```

Windows:

```text
DEP
ASLR
CFG
CET/shadow stack compatibility
SEH metadata
load configuration
```

ARM:

```text
PAC
BTI
MTE compatibility/usage signals
```

Do not reduce this to a simplistic red/green score.

Explain what each mitigation actually protects.

---

# 36. MEMORY-SAFETY HARDWARE AWARENESS

Plan architecture-awareness for:

```text
Arm MTE
Arm PAC/BTI
Intel CET
AMD shadow stacks
CHERI-style capabilities
```

The library should be able to:

- recognize supported binaries/platforms
- annotate traces with mitigation events
- identify when a crash appears related to a mitigation
- expose hardware security state to test harnesses

---

# 37. EMULATION

Module:

```text
oli.sec.emulate
```

Goals:

```text
CPU emulation
memory map
hook system
virtual files
virtual network
syscall models
snapshot
restore
deterministic clock
```

Architectures:

```text
x86-64
AArch64
```

Optional interoperability:

```text
Unicorn
QEMU
Qiling-like environments
```

Eventually build more native components.

---

# 38. DETERMINISTIC SECURITY SANDBOX

Create:

# TimeBox

A deterministic execution sandbox.

Control:

```text
clock
randomness
filesystem
network
environment
process IDs
thread scheduling where possible
```

This allows a security researcher to replay the same execution.

Example:

```oli
let box = TimeBox {
    clock: deterministic,
    network: recorded("session.otraf"),
    random: seed(42)
};
```

---

# 39. SNAPSHOTS

Support process/emulator snapshots:

```text
snapshot()
restore()
```

Primary use:

```text
fuzz initialization once
snapshot
test thousands of inputs
restore quickly
```

This can drastically reduce fuzzing overhead for expensive initialization.

---

# 40. FUZZING PLATFORM

Module:

```text
oli.sec.fuzz
```

Do not make fuzzing a single algorithm.

Architecture:

```text
Executor
Mutator
Corpus
Feedback
Oracle
Minimizer
Scheduler
CrashStore
```

Allow components to be replaced.

---

# 41. COVERAGE TYPES

Support more than edge coverage.

```text
edge coverage
block coverage
value coverage
comparison coverage
state coverage
protocol coverage
API coverage
authorization-decision coverage
parser-path coverage
```

This is one of the important innovation areas.

---

# 42. SEMANTIC COVERAGE

Create:

```text
SemanticCoverage
```

Examples:

HTTP parser:

```text
request-line parsed
chunked-body parsed
header normalization path
HTTP/2 translation path
cache decision path
```

Authentication system:

```text
anonymous
authenticated
MFA pending
MFA complete
token refresh
token rejected
```

A fuzzer can search for new states, not only new machine basic blocks.

---

# 43. VALUE-GUIDED FUZZING

Track:

```text
integer comparisons
string comparisons
length comparisons
magic bytes
checksums
```

Use them as mutation feedback.

Do not copy libFuzzer interfaces exactly.

Expose it as a generic Oli-- feedback channel.

---

# 44. STRUCTURE-AWARE FUZZING

Grammar representation:

```text
oli.sec.fuzz.grammar
```

Example concepts:

```text
Sequence
Choice
Integer
LengthField
Checksum
Reference
Optional
Repeat
```

Use the same grammar for:

```text
parsing
generation
mutation
minimization
```

---

# 45. PROTOCOL-STATE FUZZING

Network protocols require state.

Create:

```text
StateFuzzer
```

Example model:

```text
Disconnected
  ↓
Connected
  ↓
Handshake
  ↓
Authenticated
  ↓
Transaction
```

Mutate:

```text
messages
ordering
repetition
timing
state transitions
```

Useful for:

```text
TLS test implementations
WebSocket
HTTP/2
HTTP/3
custom protocols
IoT
```

---

# 46. DIFFERENTIAL FUZZING

Run the same logical input through multiple implementations.

Examples:

```text
URL parsers
JSON parsers
HTTP parsers
TLS libraries
Unicode normalizers
JWT libraries
archive parsers
```

Compare:

```text
accepted/rejected
normalized value
parsed structure
error
side effect
```

A difference is not automatically a vulnerability.

It becomes a research lead.

---

# 47. PARSER TRIBUNAL

Make differential analysis a first-class engine:

# Parser Tribunal

```oli
let tribunal = ParserTribunal[
    parser_a,
    parser_b,
    parser_c
];

tribunal.compare(input);
```

Output:

```text
Parser A: accepted host = example.com
Parser B: rejected
Parser C: accepted host = EXAMPLE.COM

Normalization disagreement detected.
```

Targets:

```text
URL
HTTP
JSON
XML
multipart
Unicode
path
IP address
domain/IDNA
JWT
email
archive formats
```

This directly targets parser-differential research.

---

# 48. CONCOLIC / SYMBOLIC ASSISTANCE

Module:

```text
oli.sec.symbolic
```

Do not attempt a perfect symbolic executor first.

Initial features:

```text
symbolic integers
branch constraints
solver interface
path hints
hybrid concrete/symbolic execution
```

Use to help a fuzzer pass:

```text
magic checks
nested conditions
checksums
length relationships
```

---

# 49. DISTRIBUTED FUZZING

Design from the beginning for:

```text
local threads
multiple processes
multiple hosts
```

Corpus synchronization:

```text
feature-based
content-addressed
deduplicated
```

Do not require a central database for small projects.

---

# 50. HARDWARE-TRACE FUZZING

For black-box/native targets, support execution feedback from:

```text
Intel PT
dynamic binary instrumentation
emulator traces
```

This is especially useful when recompilation with coverage instrumentation is unavailable.

---

# 51. CRASH TRIAGE

Module:

```text
oli.sec.crash
```

Crash information:

```text
signal/exception
fault address
registers
stack
modules
trace tail
input
memory map
sanitizer output
```

---

# 52. CRASH DNA

Create more robust deduplication than stack hash alone.

Fingerprint:

```text
fault type
top stack
CFG neighborhood
faulting instruction
taint relation
allocation context
exception code
```

Call it:

# CrashDNA

Use it to cluster millions of fuzzing crashes.

---

# 53. REPRODUCER MINIMIZATION

Minimize not only bytes.

Support:

```text
byte minimization
grammar minimization
request-sequence minimization
protocol-state minimization
race-sequence minimization
```

Preserve the security property that triggers the finding.

---

# 54. MEMORY SAFETY SANITIZERS

Native Oli-- should eventually provide its own instrumentation.

Plan:

```text
Address safety mode
Uninitialized memory mode
Integer safety mode
Data race mode
Lifetime mode
```

Interoperate initially with available platform tooling where necessary.

---

# 55. TAINT ANALYSIS

Module:

```text
oli.sec.taint
```

Taint origins:

```text
network
file
environment
IPC
user input
database
AI model output
```

Sinks:

```text
shell/process execution
SQL query
template execution
filesystem path
memory index
network destination
authorization decision
HTML output
AI tool call
```

Support:

```text
static taint
dynamic taint
hybrid taint
```

---

# 56. SECRET TAINT

Special taint class:

```text
Secret
```

Sources:

```text
private keys
passwords
tokens
session secrets
API keys
```

Track:

```text
copies
logs
network output
serialization
crash dumps
swap/persistence
```

This connects language-level secret types with runtime security analysis.

---

# 57. SECRET LIFETIME ANALYZER

Novel subsystem:

# SecretLife

For an Oli-- program, report:

```text
secret allocations
secret copies
zeroization
secret lifetime
unexpected persistence
logging exposure
```

Example:

```text
SecretKey #91
created: line 41
copies: 0
last use: line 93
zeroized: yes
lifetime: 14.2 ms
```

---

# 58. CRYPTOGRAPHY LIBRARY

Module:

```text
oli.sec.crypto
```

Submodules:

```text
hash
mac
kdf
aead
sign
kem
exchange
random
secret
encoding
x509
pqc
policy
test
```

---

# 59. MODERN CRYPTO PRIMITIVES

Hash:

```text
SHA-256
SHA-512
SHA-3
BLAKE2
BLAKE3
```

MAC:

```text
HMAC
Poly1305
```

KDF/password:

```text
HKDF
Argon2id
scrypt
PBKDF2 for compatibility
```

AEAD:

```text
AES-GCM
ChaCha20-Poly1305
XChaCha20-Poly1305
```

Public-key:

```text
Ed25519
X25519
ECDSA interoperability
RSA-PSS interoperability
```

---

# 60. POST-QUANTUM CRYPTO

Plan support for current standardized primitives:

```text
ML-KEM
ML-DSA
SLH-DSA
```

Design hybrid APIs:

```text
classical + PQ
```

Example conceptual type:

```text
HybridKeyExchange[X25519, MLKEM768]
```

Do not create proprietary cryptographic constructions.

Use verified implementations/backends until the Oli-- implementation receives serious review.

---

# 61. CRYPTO POLICY ENGINE

Create:

```text
CryptoPolicy
```

Example profiles:

```text
modern
legacy-compatible
post-quantum-transition
high-assurance
```

The policy can reject:

```text
weak hashes
short keys
deprecated algorithms
nonce misuse
unsafe random sources
```

Avoid one universal hardcoded policy.

---

# 62. NONCE / IV MISUSE DETECTION

Testing helper:

```text
NonceTracker
```

In a controlled application test, detect:

```text
reused nonce/key combinations
counter rollback
unexpected nonce length
```

This is especially valuable for embedded and application cryptography reviews.

---

# 63. CONSTANT-TIME ANALYSIS

Provide:

```text
ct_equal
ct_select
ct_mask
secure_zero
```

And research tooling:

```text
timing distribution collection
branch trace comparison
memory-access trace comparison
```

Never claim constant-time based only on source appearance.

---

# 64. CRYPTO TEST VECTORS

Use:

```text
standards test vectors
known-answer tests
negative vectors
interoperability tests
```

Allow test packs to be versioned independently from library code.

---

# 65. CRYPTOGRAPHIC BILL OF MATERIALS

Create:

```text
CBOM
```

Inventory:

```text
algorithm
purpose
key size
certificate
library/provider
call site
protocol
```

Integrate with supply-chain analysis.

---

# 66. NETWORK PROTOCOL CORE

`oli.sec.protocol` should include parsers/builders for:

```text
Ethernet
ARP
IPv4
IPv6
ICMP
TCP
UDP
SCTP
DNS
DHCP
TLS records
HTTP/1.1
HTTP/2
HTTP/3
QUIC
WebSocket
WebTransport
gRPC framing
```

Later:

```text
MQTT
AMQP
CoAP
CAN
UDS
BLE
```

---

# 67. ZERO-COPY PROTOCOL PARSING

A protocol field should normally be:

```text
view byte
```

into the original packet/stream.

Do not allocate strings for every header.

Example:

```oli
let req = Http1.parse(bytes)?;

let host = req.header_view("host");
```

---

# 68. INCREMENTAL PARSERS

Network parsers must support fragmented input.

```text
NeedMoreData
Parsed
Invalid
```

Never assume a complete message arrives in one buffer.

This matters for:

```text
TCP
TLS
HTTP/2
HTTP/3
WebSocket
```

---

# 69. PARSER RESOURCE LIMITS

Every parser needs configurable limits:

```text
maximum nesting
maximum fields
maximum header bytes
maximum body bytes
maximum recursion
maximum allocations
```

Defend the security library itself from malicious inputs.

---

# 70. HTTP RAW MODEL

Do not represent HTTP only as a dictionary of headers.

Need both:

```text
RawHttpMessage
SemanticHttpMessage
```

Raw representation preserves:

```text
header order
duplicate headers
whitespace
casing
line endings
original bytes
```

Semantic representation provides normalized access.

This is mandatory for security research.

---

# 71. HTTP/2 AND HTTP/3 FRAME MODEL

Expose frames directly.

HTTP/2:

```text
HEADERS
DATA
SETTINGS
RST_STREAM
GOAWAY
WINDOW_UPDATE
CONTINUATION
```

HTTP/3/QUIC:

```text
streams
frames
transport parameters
QPACK state
connection IDs
```

Do not hide frame boundaries.

---

# 72. PROTOCOL TRANSLATION OBSERVER

Create:

# ProtocolBridge

Model conversions such as:

```text
HTTP/2 -> HTTP/1.1
HTTP/3 -> HTTP/1.1
reverse proxy transformations
```

Goal:

Detect semantic differences between layers.

This is relevant to modern request desynchronization research.

The initial tool should focus on differential interpretation and safe detection rather than automatic exploitation.

---

# 73. DESYNC DIFFERENTIAL LAB

Create controlled test helpers:

```text
DesyncOracle
ParserPair
TranslationTrace
```

Run harmless marker requests through:

```text
front-end parser
back-end parser
```

Compare how message boundaries are interpreted.

Output:

```text
same
different
unknown
```

Do not silently perform queue poisoning against unrelated users.

---

# 74. CONNECTION-STATE ANALYZER

Track:

```text
connection reuse
pipelining
stream multiplexing
connection close
backend pool behavior
```

This is crucial for avoiding false positives in request-smuggling research.

---

# 75. CACHE GENOME

Create:

# CacheGenome

A safe cache-behavior mapper.

Infer:

```text
possible cache key inputs
normalization
cacheability
vary behavior
TTL behavior
path handling
query handling
header influence
```

Use unique cache-busters in safe-active mode to avoid affecting other users.

The engine should produce:

```text
CacheModel
```

not simply "cache poisoning possible".

---

# 76. NORMALIZATION MATRIX

Create:

# NormalizationMatrix

Compare normalization across:

```text
browser
CDN
reverse proxy
framework
router
application
filesystem
database
```

Inputs:

```text
Unicode
URL encoding
path separators
dot segments
case
IDNA
duplicate parameters
JSON duplicate keys
HTTP header whitespace
```

This directly addresses the modern parser/normalization research area.

---

# 77. UNICODE SECURITY LAB

Module:

```text
web.unicode
```

Analyze:

```text
NFC
NFD
NFKC
NFKD
case folding
confusables
IDNA
homoglyphs
zero-width characters
```

Report transformations, not just the final string.

---

# 78. SIDE-CHANNEL MEASUREMENT ENGINE

Create:

# LeakScope

For authorized experiments measure:

```text
timing
response size
redirect behavior
cache state
connection reuse
resource loading outcome
browser event timing
```

Use statistical analysis.

A signal is a hypothesis until significance and reproducibility thresholds are met.

Do not generate invasive cross-user exploit chains automatically.

---

# 79. RACE CONDITION ENGINE

Create:

# StateRace

Features:

```text
synchronized requests
connection warming
multiple endpoints
barrier release
precise timestamping
state comparison
```

Represent tests as state transitions.

Example:

```text
balance=100
   ↓ two concurrent transactions
final state=?
```

Safe-active mode requires explicit endpoints and low concurrency.

---

# 80. BUSINESS-LOGIC STATE GRAPH

Create:

```text
BusinessStateGraph
```

Nodes:

```text
account states
order states
subscription states
verification states
payment states
```

Edges:

```text
API operations
```

Testing then looks for:

```text
invalid transition accepted
transition skipped
one-time action repeated
state inconsistency
```

This is substantially more useful than payload-only scanning.

---

# 81. WEB MODULE

```text
oli.sec.web
```

Submodules:

```text
proxy
http
h2
h3
cache
race
normalize
session
cors
csp
csrf
upload
template
graphql
grpc
websocket
webtransport
browser
```

---

# 82. PROGRAMMABLE PROXY

Create a native proxy API.

Needs:

```text
HTTP/1.1
HTTP/2
CONNECT
WebSocket
TLS interception for explicitly configured test environments
```

HTTP/3 proxying can follow later.

Store both:

```text
raw message
parsed message
```

---

# 83. REQUEST TRANSFORM PIPELINE

Allow plugins:

```text
before_send
after_receive
before_parse
after_parse
```

Each mutation must be logged.

No invisible automatic modification.

---

# 84. WEB SECURITY POLICY ANALYZER

Analyze:

```text
CSP
Trusted Types
CORS
COOP
COEP
CORP
HSTS
Permissions-Policy
Referrer-Policy
cookies
Fetch Metadata
```

Explain interactions between policies.

---

# 85. BROWSER SECURITY GRAPH

Module:

```text
oli.sec.browser
```

Graph entities:

```text
Origin
Site
Frame
Window
Worker
ServiceWorker
Storage
Cookie
MessageChannel
```

Edges:

```text
navigation
postMessage
fetch
storage access
service-worker control
redirect
```

This allows browser security analysis as a graph problem.

---

# 86. POSTMESSAGE ANALYZER

For applications under test, map:

```text
message sender
origin validation
message type
handler
data sinks
```

Use static + dynamic evidence.

---

# 87. SERVICE WORKER ANALYZER

Inspect:

```text
scope
cache interaction
fetch handlers
offline responses
update behavior
```

Useful because service workers create a second application routing/cache layer.

---

# 88. GRAPHQL SECURITY

Module:

```text
web.graphql
```

Features:

```text
schema import
introspection handling
operation parser
query cost model
depth model
field authorization map
resolver timing
batching analysis
```

Security tests should focus on:

```text
authorization
schema exposure
resource exhaustion
CSRF conditions
unexpected field access
```

---

# 89. gRPC SECURITY

Module:

```text
web.grpc
```

Support:

```text
protobuf descriptors
reflection
unary
streaming
metadata
status
```

Build authorization matrices per method.

---

# 90. WEBSOCKET / WEBTRANSPORT

Model:

```text
handshake
session
message schema
state
authorization
origin
```

State fuzzing is more important than random payload mutation.

---

# 91. API SECURITY CORE

Module:

```text
oli.sec.api
```

Input descriptions:

```text
OpenAPI
GraphQL schema
protobuf
HAR
captured traffic
manual schema
```

Generate:

```text
EndpointGraph
ObjectGraph
RoleMatrix
StateGraph
```

---

# 92. OBJECT AUTHORIZATION MATRIX

Create:

# AuthMatrix

Given authorized test accounts:

```text
Account A
Account B
Admin
Anonymous
```

and objects:

```text
A-owned object
B-owned object
public object
```

replay equivalent operations and compare authorization decisions.

This targets BOLA/BFLA classes while remaining evidence-oriented.

Never infer test accounts the user has not provided.

---

# 93. RESPONSE DIFFERENTIALS

Do not compare only status codes.

Compare:

```text
status
headers
body schema
field presence
field values
timing
side effects
database-visible state if integration available
```

Normalize volatile fields.

---

# 94. API INVENTORY ENGINE

Discover from authorized sources:

```text
OpenAPI
GraphQL
client JavaScript
mobile app metadata
captured traffic
documentation
```

Build versioned endpoint inventory.

Identify:

```text
deprecated endpoints
shadow versions
debug routes
schema drift
```

---

# 95. MASS-ASSIGNMENT / SCHEMA DIFF TESTER

Use schema knowledge to compare:

```text
documented writable fields
observed accepted fields
returned fields
```

Focus on model/API mismatch rather than blind parameter spraying.

---

# 96. AUTHENTICATION PLATFORM

Module:

```text
oli.sec.auth
```

Components:

```text
sessions
cookies
JWT/JWS/JWE
OAuth 2.x
OIDC
SAML
WebAuthn
passkeys
mTLS
DPoP
```

---

# 97. OAUTH SECURITY MODEL

Implement validators based on current best practices.

Understand:

```text
redirect URI exactness
PKCE
state
nonce
issuer
audience
token type
refresh rotation
sender-constrained tokens
client authentication
```

Include modern support for:

```text
DPoP
PAR
JAR
```

---

# 98. TOKEN PROVENANCE

Track a token through:

```text
issuance
storage
transport
refresh
use
revocation
```

Report unexpected exposure:

```text
URL
logs
referer
local storage
crash dump
```

---

# 99. JWT/JWS/JWE LAB

Parser must preserve:

```text
raw header
raw claims
signature bytes
algorithm
key id
```

Validation must be separate from decoding.

API must make it difficult to accidentally treat:

```text
decoded
```

as:

```text
verified
```

---

# 100. WEBAUTHN / PASSKEY ANALYZER

Support WebAuthn Level 3 concepts.

Validate ceremonies:

```text
challenge
origin
RP ID
credential ID
user verification flags
attestation
signature counter where applicable
extensions
```

Represent:

```text
RegistrationCeremony
AuthenticationCeremony
```

Allow replay protection and challenge-lifecycle tests in a lab.

---

# 101. SAML ANALYZER

Support:

```text
metadata
assertions
signatures
audience
recipient
destination
InResponseTo
conditions
```

Focus on structural/reference validation.

Keep signature verification strict.

---

# 102. STATIC APPLICATION SECURITY ANALYSIS

Module:

```text
oli.sec.static
```

Use compiler-level information for Oli-- programs:

```text
AST
OIR
dataflow
control flow
types
capabilities
```

This can be substantially stronger than regex-based linters.

---

# 103. SECURITY QUERY LANGUAGE

Create:

# SecQL

A query system over:

```text
code
OIR/SIR
dataflow
call graph
binary
traffic
artifacts
```

Example conceptual query:

```text
source: HttpRequest.param
flows_to: Process.exec
without: ShellEscape
```

Do not finalize syntax prematurely.

---

# 104. PATH EXPLANATIONS

A static finding should show the flow:

```text
HTTP parameter
  ↓
parse()
  ↓
config.name
  ↓
build_command()
  ↓
process.exec()
```

Not just:

```text
"command injection warning"
```

---

# 105. CUSTOM SECURITY PACKS

Allow versioned query packs:

```text
oli-sec-web
oli-sec-crypto
oli-sec-kernel
oli-sec-mobile
oli-sec-cloud
```

Tests for security queries are mandatory.

---

# 106. REACHABILITY ANALYSIS

Integrate software components with code reachability.

Example:

```text
package vulnerable function
     ↓
imported by application?
     ↓
reachable from input?
```

This reduces noisy dependency alerts.

---

# 107. SOFTWARE SUPPLY CHAIN

Module:

```text
oli.sec.supply
```

Support:

```text
SBOM
CBOM
ML-BOM
VEX
provenance
attestations
signatures
dependency graph
```

Formats:

```text
CycloneDX
SPDX
in-toto attestations
```

---

# 108. BUILD PROVENANCE

Track:

```text
source revision
compiler
compiler version
flags
dependencies
build host/runner
artifact hash
```

Allow signed provenance.

Integrate with package manager.

---

# 109. REPRODUCIBLE BUILD VERIFICATION

Command concept:

```bash
oli sec supply reproduce package
```

Compare built artifact hashes or semantic binary differences.

When byte-for-byte reproducibility is impossible, explain why.

---

# 110. DEPENDENCY PEDIGREE

Record:

```text
origin
version
fork status
patches
maintainer/source
signature
transitive graph
```

This aligns with modern supply-chain assurance practices.

---

# 111. CONTAINER SECURITY

Module:

```text
oli.sec.container
```

Support:

```text
OCI image
image layers
manifest
config
filesystem
capabilities
seccomp
user
mounts
environment
```

Analyze:

```text
embedded secrets
unexpected setuid files
unsafe capabilities
outdated packages
exposed configuration
```

---

# 112. KUBERNETES SECURITY

Module:

```text
oli.sec.cloud.k8s
```

Parse:

```text
Pod
Deployment
Service
Ingress
Role
ClusterRole
RoleBinding
NetworkPolicy
ServiceAccount
Secret metadata
```

Build:

```text
RBACGraph
NetworkReachabilityGraph
WorkloadPrivilegeGraph
```

---

# 113. IAM GRAPH

Cloud authorization often becomes graph analysis.

Create generic:

```text
Identity
Principal
Role
Policy
Resource
Permission
Trust
```

Then provider adapters can map native IAM models.

Focus on:

```text
effective permission
privilege paths
trust relationships
unused privilege
```

---

# 114. MOBILE SECURITY

Module:

```text
oli.sec.mobile
```

Android:

```text
APK
AAB
DEX
AndroidManifest
resources
native libraries
network security config
intent filters
deep links
exported components
WebView configuration
```

iOS later:

```text
IPA
Mach-O
plist
entitlements
URL schemes
universal links
keychain configuration metadata
```

---

# 115. MOBILE STATIC + DYNAMIC CORRELATION

Example:

```text
Android deep link
      ↓
Activity
      ↓
Java/Kotlin method
      ↓
JNI
      ↓
native function
      ↓
network request
```

The artifact graph should connect these layers.

---

# 116. MOBILE INSTRUMENTATION

Allow integrations with:

```text
Frida-style instrumentation backend
ADB
platform debuggers
```

Long-term implement native Oli-- instrumentation.

Keep credential/token extraction features constrained to explicit authorized test sessions.

---

# 117. FIRMWARE SECURITY

Module:

```text
oli.sec.firmware
```

Support:

```text
firmware image identification
partition/container parsing
filesystem extraction
ELF/PE payload identification
entropy map
strings
certificates
keys metadata
configuration
```

Formats later:

```text
Intel HEX
SREC
UF2
UEFI
device trees
common embedded filesystems
```

---

# 118. ENTROPY MAP

Visualize entropy by offset.

Useful to identify:

```text
compressed areas
encrypted areas
code/data boundaries
packed blobs
```

Do not automatically label every high-entropy region as encrypted.

---

# 119. EMBEDDED PROTOCOLS

Future protocol modules:

```text
UART
USB
CAN
UDS
BLE
MQTT
CoAP
Modbus
```

These should reuse the same:

```text
capture
parser
state machine
fuzz
evidence
```

infrastructure.

---

# 120. AI / LLM SECURITY

Create:

```text
oli.sec.ai
```

This must be a first-class security domain in 2026.

Submodules:

```text
prompt
agent
tool
mcp
rag
embedding
model
policy
trace
budget
```

---

# 121. AGENT TOOL GRAPH

Map:

```text
agent
tool
permission
credential
downstream resource
```

Build:

# AgentCapabilityGraph

Detect:

```text
excessive tool permissions
destructive operations without confirmation
shared privileged identities
unexpected tool chains
```

---

# 122. PROMPT / DATA PROVENANCE

Every model context item should optionally carry provenance:

```text
system
developer
user
retrieval
tool output
remote webpage
memory
other agent
```

This makes it possible to reason about trust boundaries.

---

# 123. PROMPT-INJECTION TEST HARNESS

For applications owned/authorized by the user:

```text
direct injection cases
indirect injection cases
retrieval-content cases
tool-output cases
multi-agent cases
```

Measure:

```text
instruction boundary violations
tool invocation
data exposure
policy violation
```

Do not judge security based only on whether the model outputs a specific sentence.

---

# 124. TOOL MISUSE TESTER

Test whether untrusted model output can reach:

```text
file write
email send
database mutation
shell execution
payment
publication
account changes
```

Security policy should require confirmation or downstream authorization as appropriate.

---

# 125. AGENT TRANSACTION LOG

Record:

```text
prompt
context sources
model response
tool chosen
tool arguments
authorization decision
tool result
state change
```

This creates a deterministic-ish audit trail even when model outputs are stochastic.

---

# 126. MCP SECURITY ANALYZER

Plan:

```text
server capability inventory
tool schema analysis
permission mapping
credential boundaries
origin/trust metadata
tool-call validation
resource access
```

Do not assume an MCP tool is trustworthy just because it advertises a schema.

---

# 127. RAG SECURITY

Map:

```text
document source
chunk
embedding
index
retrieval
prompt inclusion
answer citation
```

Test:

```text
retrieval poisoning
cross-tenant retrieval
metadata filtering
sensitive chunk leakage
stale permissions
```

---

# 128. AI BUDGET / UNBOUNDED CONSUMPTION

Measure:

```text
tokens
tool calls
network calls
GPU time
wall time
money estimate
```

Define policy caps.

---

# 129. MODEL / DATA SUPPLY CHAIN

Integrate with:

```text
ML-BOM
model hash
dataset provenance
adapter/LoRA provenance
prompt templates
tool manifests
```

---

# 130. BUG BOUNTY WORKSPACE MODEL

Create:

```text
oli.sec.bounty
```

This is not an exploit automation engine.

It is a workflow layer.

Types:

```text
Program
Scope
Asset
Account
Role
Session
Finding
Evidence
Retest
```

---

# 131. ASSET GRAPH

Within user-provided authorization scope, maintain:

```text
domains
hosts
applications
API versions
mobile apps
services
certificates
known routes
technologies
```

Track discovery provenance.

---

# 132. JAVASCRIPT / CLIENT ROUTE EXTRACTION

Analyze client code and source maps when legitimately accessible.

Extract:

```text
routes
API paths
GraphQL operations
feature flags
public configuration
```

Do not treat strings as confirmed endpoints until observed/validated.

---

# 133. ROLE-AWARE TEST SESSION

Bug bounty often requires comparing user roles.

Represent:

```text
Account
Role
SessionJar
```

Then:

```text
replay request as account A
replay as account B
compare
```

Every account must be explicitly registered into the workspace by the user.

---

# 134. COOKIE JAR / SESSION ENGINE

Full support:

```text
Set-Cookie
domain/path matching
Secure
HttpOnly
SameSite
expiry
partitioned cookie metadata
```

Preserve cookie provenance.

---

# 135. REQUEST HISTORY AS A GRAPH

Do not store proxy history only as rows.

Represent relationships:

```text
login
  ↓
session created
  ↓
object created
  ↓
object fetched
  ↓
object edited
```

This enables second-order/stateful testing.

---

# 136. SECOND-ORDER TESTING

Create stateful test workflows.

Example abstract pattern:

```text
input stored in operation A
        ↓
processed later in operation B
```

The tool should track the relation automatically when possible.

---

# 137. AUTHORIZATION DECISION TRACING

When testing a local/staging app with tracing enabled:

```text
request
 ↓
middleware
 ↓
policy
 ↓
database lookup
 ↓
decision
```

Correlate an external 403/200 with internal decision path.

This is powerful for developers and gray-box pentests.

---

# 138. WEB CACHE + APPLICATION CACHE CORRELATION

Track:

```text
CDN
reverse proxy cache
framework cache
application object cache
browser cache
service worker cache
```

Cache vulnerabilities often emerge from disagreements between these layers.

---

# 139. REDIRECT GRAPH

Create:

```text
RedirectGraph
```

Track:

```text
HTTP redirect
meta refresh
JS navigation
OAuth redirect
mobile deep link
```

Useful for:

```text
OAuth analysis
browser side channels
open redirects
deep-link security
```

---

# 140. RESPONSE SIZE / ETag OBSERVABILITY

For controlled side-channel research, collect:

```text
Content-Length
actual body length
compressed size
ETag
cache hit/miss
timing
```

Use statistical comparison.

---

# 141. ERROR SEMANTICS ENGINE

Security testing often discovers subtle information through errors.

Represent:

```text
status
application error code
message structure
stack trace presence
timing
state mutation
```

Cluster errors semantically.

---

# 142. ORM QUERY OBSERVABILITY

For local/instrumented applications, map:

```text
API filter
ORM expression
generated SQL
returned fields
```

This creates a safe research environment for ORM leakage/authorization flaws.

---

# 143. TEMPLATE ENGINE TESTING

Build parsers/adapters for common template-expression semantics.

Focus on:

```text
input reaches template?
which interpreter?
which context?
escaping mode?
error behavior?
```

Do not default to executing arbitrary system commands as a vulnerability proof.

Prefer harmless deterministic proof primitives.

---

# 144. SAFE PROOF ENGINE

For bug bounty evidence, define:

```text
ProofPrimitive
```

Categories:

```text
read-only
state-isolated
self-account-only
lab-only
destructive
```

The system should prefer the lowest-impact proof available.

---

# 145. RATE / CONCURRENCY BUDGETS

Every active test session supports:

```text
max requests
requests/sec
max concurrent connections
max bytes/sec
max test duration
```

Race-condition testing requires explicit concurrency budget.

---

# 146. SECURITY TEST DSL

Create a declarative test layer.

Concept:

```oli
security test ObjectIsolation {
    given alice;
    given bob;

    object = alice.create();

    expect bob.read(object) => denied;
}
```

Compile into requests and assertions.

This makes regression tests reusable after a bug is fixed.

---

# 147. SECURITY REGRESSION TESTS

A found bug should be convertible into:

```text
permanent automated regression test
```

This is a major bridge between bug bounty/pentest and engineering.

---

# 148. EVIDENCE PACKAGING

Module:

```text
oli.sec.evidence
```

Export:

```text
Markdown
JSON
SARIF
HAR
PCAPNG
JUnit
```

Finding description:

```text
reproduction
affected request
observed behavior
expected behavior
impact evidence
scope
```

---

# 149. REDACTION ENGINE

Before exporting a report, automatically locate potential:

```text
access tokens
cookies
passwords
private keys
PII-like values
```

Offer:

```text
redact
hash
partial reveal
keep
```

Never silently destroy original local evidence.

---

# 150. DATASET FOR SECURITY RESEARCH

Create normalized local datasets:

```text
traffic corpus
parser corpus
crash corpus
binary corpus
test-case corpus
```

Content-addressed storage.

Deduplicate by cryptographic hash.

---

# 151. GPU ACCELERATION THROUGH `oli.compute`

Security workloads that can benefit from GPU without making GPU mandatory:

```text
large-scale byte-pattern scanning
entropy maps
binary similarity embeddings
PCAP analytics
large corpus feature extraction
fuzz corpus clustering
crash clustering
statistical side-channel analysis
```

Do not turn the security library into a password-cracking toolkit by default.

---

# 152. SIMD SECURITY PRIMITIVES

Use native SIMD for:

```text
byte search
delimiter scan
UTF-8 validation
Base64
hex conversion
hashing where appropriate
protocol parsing
signature scanning
```

Provide scalar fallback.

---

# 153. HIGH-SPEED PATTERN ENGINE

Create:

```text
oli.sec.pattern
```

Patterns:

```text
fixed bytes
masked bytes
multi-pattern
structured fields
```

Later:

```text
finite automata
SIMD scanning
GPU batch scanning
```

---

# 154. BINARY SIGNATURES

Use signatures for:

```text
file type
library identification
function recognition
firmware components
```

Signatures require versioning and confidence.

---

# 155. SECURITY KNOWLEDGE PACKS

Do not hardcode all security knowledge in core library.

Create signed/versioned packs:

```text
protocol rules
crypto policy
static queries
file signatures
API rules
browser policy rules
```

This allows security knowledge to evolve faster than the compiler.

---

# 156. RESEARCH MODE

Create:

```bash
oli sec research
```

Research mode exposes:

```text
raw parser decisions
normalization steps
frame boundaries
timing distributions
state graph
trace events
```

No attempt to simplify everything into one verdict.

---

# 157. EXPLAIN MODE

Every subsystem should support an explanation.

Examples:

```bash
oli sec explain capture
oli sec explain auth finding-12
oli sec explain parser input.bin
oli sec explain crash crash-91
```

---

# 158. "WHY DID THESE TWO SYSTEMS DISAGREE?"

Cross-layer comparison should be a core workflow.

Input:

```text
Artifact A
Artifact B
```

Output:

```text
first point of semantic divergence
```

Examples:

```text
browser URL parser vs reverse proxy
CDN cache key vs origin route
HTTP/2 frontend vs HTTP/1 backend
JWT decoder vs validator
mobile deep-link matcher vs server router
```

Call the engine:

# Divergence Finder

---

# 159. DIVERGENCE FINDER ALGORITHM

For supported transformations:

1. capture raw input
2. capture intermediate representations
3. normalize into comparable semantic nodes
4. align nodes
5. locate first differing interpretation
6. explain transformation responsible

This is a research engine, not a list of payloads.

---

# 160. STATE TIME MACHINE

Create:

# StateTime

Record application state checkpoints and request sequence.

Allow:

```text
rewind
replay
branch
compare
```

Useful in local/staging environments.

Example:

```text
state S0
 ├── request A -> S1
 │      └── request B -> S2
 └── request B -> S3
```

This can reveal order-dependent behavior.

---

# 161. TEMPORAL DIFF

Compare:

```text
same request
same app
different time/state
```

Identify:

```text
cache effects
session expiry
race effects
token rotation
eventual consistency
```

---

# 162. TEST ORACLES

A security test requires an oracle.

Support:

```text
crash oracle
memory oracle
authorization oracle
parser differential oracle
state invariant oracle
timing oracle
data exposure oracle
policy oracle
```

Make oracle construction explicit.

---

# 163. INVARIANT TESTING

Users can define:

```text
Invariant
```

Examples:

```text
user B never reads user A private object
anonymous user never receives admin field
secret never reaches log sink
request parser layers agree on body boundary
```

Fuzzers and state explorers can search for invariant violations.

This is one of the best ways to move beyond signature scanning.

---

# 164. SECURITY PROPERTY TYPES

Potential future Oli-- language integration:

```text
@secret
@untrusted
@authenticated
@validated
@constant_time
@bounded
```

Do not add these to the language until semantics are formally defined.

The library can prototype them first.

---

# 165. SANDBOXED PLUGINS

Security plugins process hostile data.

Run third-party analyzers with:

```text
memory limits
CPU limits
filesystem sandbox
network disabled by default
```

A malicious PCAP/parser pack should not compromise the analyzer host.

---

# 166. PLUGIN ABI

Plugin types:

```text
ParserPlugin
DissectorPlugin
FuzzMutator
FuzzOracle
SecurityQuery
ProtocolModel
EvidenceExporter
```

Plugins written in Oli-- should compile to native modules with a stable ABI.

---

# 167. SCRIPTABILITY WITHOUT PYTHON DEPENDENCY

Provide a compact Oli-- REPL/tool mode eventually:

```bash
oli sec shell
```

Example concept:

```oli
capture("eth0")
    .tcp()
    .where(.dst_port == 443)
    .group_by(.process)
```

This preserves native performance while remaining interactive.

---

# 168. MACHINE-READABLE OUTPUT

All CLI tools support:

```text
--json
--jsonl
--sarif where relevant
```

Human-readable output is not enough.

---

# 169. LARGE-DATA STREAMING

PCAPs and traces can be enormous.

Every analyzer should support:

```text
streaming
bounded memory
backpressure
partial indexes
```

Do not require loading a 100 GB capture into RAM.

---

# 170. CONTENT-ADDRESSED CACHE

Cache expensive results by:

```text
artifact hash
tool version
analysis configuration
```

Examples:

```text
binary lift
symbol analysis
PCAP index
decompilation
fuzz corpus feature set
```

---

# 171. PERFORMANCE PROFILER

`oli.sec.profiler` must show:

```text
packets/sec
Gbps
events/sec
fuzz executions/sec
disassembly MB/sec
parser MB/sec
memory
allocations
copy volume
```

---

# 172. ZERO-COPY METRICS

Report:

```text
bytes copied
bytes viewed
bytes mapped
```

This directly tests whether Oli-- is delivering on its low-level design goals.

---

# 173. PLATFORM TARGETS

Priority:

## Linux x86-64

Best initial target because it enables:

```text
AF_XDP
eBPF
io_uring
perf/Intel PT
raw networking
namespaces
```

Next:

## Windows x86-64

Focus:

```text
PE
ETW
debug APIs
WinSock
CFG/CET metadata
minidumps
```

Then:

## Linux ARM64 / Android

Focus:

```text
AArch64
MTE
PAC/BTI
APK/DEX
```

Then macOS/iOS.

---

# 174. WINDOWS EVENT / TRACE BACKEND

Plan:

```text
ETW
Windows debugging API
minidump
process/module enumeration
network telemetry
```

Do not attempt feature parity with Linux eBPF in one milestone.

---

# 175. LINUX NAMESPACE LAB

Create:

```text
oli.sec.lab.net
```

Build isolated:

```text
network namespaces
virtual Ethernet pairs
test proxy
backend service
packet taps
```

This provides a safe environment for protocol experiments.

---

# 176. CONTAINERIZED SECURITY LAB

Allow:

```text
target container
proxy container
observer
client
```

Generate repeatable local experiments.

---

# 177. PROTOCOL DIFFERENTIAL LAB

Users should be able to spin up multiple parser implementations and compare them.

Example:

```text
nginx
haproxy
custom server
test parser
```

Only in user-controlled laboratory environments unless external testing is explicitly authorized.

---

# 178. SECURITY BENCHMARKS

Create a benchmark suite for `oli.sec`.

Categories:

```text
PCAP parse throughput
HTTP parse throughput
TLS record parse
AF_XDP receive
binary disassembly
SIR lifting
fuzz executions/sec
snapshot restore latency
trace event throughput
taint overhead
```

---

# 179. SECURITY CORRECTNESS CORPUS

Maintain test corpora:

```text
valid
invalid
edge-case
regression
adversarial
```

For:

```text
HTTP
URL
Unicode
TLS
DNS
JWT
ASN.1
PE
ELF
PCAP
```

---

# 180. MALFORMED INPUT HARDENING

`oli.sec` itself processes hostile inputs.

Requirements:

```text
no panic on malformed input
bounded allocations
checked integer arithmetic
recursion limits
size limits
fuzz every parser
```

---

# 181. ASN.1 / DER

Implement a strict DER path and separate BER compatibility path.

Never blur:

```text
strict
permissive
```

because parser differences matter in security.

---

# 182. X.509

Support:

```text
certificate parsing
chain building
name constraints
SAN
KU/EKU
policy metadata
validity
signature verification
```

Separate:

```text
parse
validate
trust decision
```

---

# 183. DNS

Support:

```text
A
AAAA
CNAME
MX
TXT
NS
SOA
SRV
CAA
DNSSEC metadata
```

Parser protections:

```text
compression pointer loop limit
packet bounds
name length
record count
```

---

# 184. TLS

Layers:

```text
record parser
handshake parser
certificate inspection
configuration analyzer
interop backend
```

Inspect:

```text
versions
cipher suites
groups
signature algorithms
ALPN
SNI
extensions
session resumption
```

Do not implement an unaudited TLS stack as the default secure transport.

---

# 185. QUIC / HTTP/3

Treat QUIC as first-class.

Model:

```text
connection IDs
packet number spaces
crypto frames
streams
transport parameters
migration
```

This is essential because HTTP security research is moving beyond HTTP/1-only assumptions.

---

# 186. SECURITY STATE VISUALIZATION

Generate graph artifacts for UI tools:

```text
network flow graph
call graph
auth graph
state graph
dependency graph
agent capability graph
```

Core remains headless.

---

# 187. REPORT CONFIDENCE

A finding should include:

```text
observed
inferred
hypothesized
```

Example:

```text
observed:
  frontend and backend normalize path differently

inferred:
  cache key likely differs from origin route

not proven:
  cross-user impact
```

This reduces misleading scanner output.

---

# 188. FALSE-POSITIVE CONTROL

Every detector should document:

```text
what proves the condition
what can mimic it
what additional evidence increases confidence
```

Especially for:

```text
request desync
timing leaks
race conditions
cache behavior
authorization differences
```

---

# 189. BASELINE ENGINE

Security tests often need baseline behavior.

Collect:

```text
normal latency distribution
normal response variants
normal cache behavior
normal parser output
```

Only then compare anomalies.

---

# 190. STATISTICAL ENGINE

Use `oli.compute` for:

```text
confidence intervals
distribution comparison
outlier detection
correlation
clustering
```

Useful for timing/side-channel research and large trace sets.

---

# 191. DIFFERENTIAL VERSION TESTING

Given version A and B:

```text
binary diff
API behavior diff
network behavior diff
security policy diff
```

Correlate changes.

This is valuable after patches.

---

# 192. PATCH VERIFICATION

When a vulnerability is fixed:

```text
replay evidence bundle
run regression
compare state
compare traces
```

Produce:

```text
fixed
still reproducible
behavior changed
inconclusive
```

---

# 193. SECURITY DIGITAL TWIN

Long-term innovation:

# SecTwin

Create a simplified model of:

```text
network topology
services
roles
API state
policies
```

Feed observed traffic into it.

Use it to:

```text
replay
simulate
test invariants
compare deployment versions
```

Do not pretend it perfectly represents production.

---

# 194. BEHAVIOR FINGERPRINT

A service fingerprint should be based not only on headers.

Use:

```text
protocol quirks
normalization behavior
error shapes
timing
TLS features
HTTP settings
```

This can help recognize stack changes in owned infrastructure.

Avoid using fingerprinting as a stealth mechanism.

---

# 195. BUILD A "SECURITY MICROSCOPE", NOT ONLY A SCANNER

The ultimate UX idea:

A researcher selects an object:

```text
request
packet
function
token
crash
```

and asks:

```text
where did this come from?
what transformed it?
what process handled it?
what code touched it?
what state changed?
what security decision was made?
```

Oli-- should try to answer using the artifact/provenance graph.

---

# 196. RESEARCH AREAS TO TRACK CONTINUOUSLY

Claude Code should maintain:

```text
docs/research-watch/
```

Topics:

```text
HTTP parser differentials
HTTP/2 and HTTP/3
cache inconsistencies
browser side channels
Unicode normalization
OAuth/OIDC
WebAuthn/passkeys
API authorization
race conditions
AI agent security
MCP
post-quantum crypto
memory-safety hardware
eBPF/XDP
modern fuzzing
supply-chain security
```

Update quarterly.

Do not blindly implement every paper.

---

# 197. RESEARCH BASIS FOR THIS VERSION

This architecture intentionally incorporates trends and capabilities visible in current primary/major sources as of September 2026:

```text
Linux kernel:
- AF_XDP zero-copy and multi-buffer
- XDP metadata
- io_uring zero-copy receive
- eBPF program/attach types
- segmentation offloads

Wireshark:
- checksum/offload capture effects
- TLS key-log based decryption workflows

PortSwigger research:
- parser differentials
- HTTP/2 CONNECT
- XS-Leaks / side channels
- cache inconsistencies
- Unicode normalization
- request desynchronization
- race-condition state-machine research

OWASP:
- API Security
- software component verification
- CycloneDX SBOM/CBOM/ML-BOM
- GenAI/agentic AI security

IETF/W3C:
- OAuth security BCP
- DPoP
- PAR
- WebAuthn Level 3

NIST:
- ML-KEM
- ML-DSA
- SLH-DSA

Frida:
- dynamic instrumentation
- basic-block/instruction tracing

modern fuzzing ecosystems:
- coverage-guided fuzzing
- persistent execution
- distributed feature/corpus models
- black-box instrumentation

modern hardware protections:
- CET/shadow stacks
- Arm MTE
- pointer authentication
- CHERI research
```

---

# 198. PHASED IMPLEMENTATION PLAN

Do not implement all of `oli.sec` at once.

## Phase 0 — Security foundations

Implement:

```text
Artifact
ArtifactGraph
ByteView
Byte provenance
Scope
Evidence
Result/errors
limits
```

Tests:

```text
malformed artifacts
scope denial
provenance preservation
zero-copy views
```

---

# 199. PHASE 1 — Binary + bytes

Implement:

```text
hex/base64
endian
BinaryReader
BinaryWriter
ELF
PE
basic disassembly x86-64
```

Fuzz every parser.

---

# 200. PHASE 2 — Network capture foundation

Implement:

```text
libpcap/AF_PACKET
PCAPNG
Ethernet
IPv4/IPv6
TCP/UDP
flow reconstruction
```

Then AF_XDP.

Milestone:

```text
capture 10+ Gbps where hardware permits
without unbounded memory growth
with accurate drop accounting
```

Actual performance must be benchmarked on real hardware.

---

# 201. PHASE 3 — Packet Reality

Implement:

```text
offload detection
XDP capture
TCX observation
process/socket mapping
multi-plane correlation
```

Demo:

```text
one TCP flow shown simultaneously at XDP + socket + app level
```

---

# 202. PHASE 4 — HTTP research core

Implement:

```text
raw HTTP/1
semantic HTTP/1
HTTP/2 frames
proxy
connection model
normalization matrix
Parser Tribunal
```

No exploit automation required.

---

# 203. PHASE 5 — API / auth

Implement:

```text
OpenAPI
GraphQL
AuthMatrix
cookie/session jar
JWT
OAuth analysis
```

Then:

```text
DPoP
PAR
WebAuthn
```

---

# 204. PHASE 6 — Fuzzing

Implement:

```text
executor
corpus
edge coverage
comparison/value feedback
minimizer
crash store
```

Then:

```text
grammar fuzzing
state fuzzing
differential fuzzing
```

---

# 205. PHASE 7 — Reverse engineering

Implement:

```text
PE/ELF loader
x86-64 decoder
SIR
CFG
call graph
basic type recovery
```

Then AArch64.

---

# 206. PHASE 8 — Dynamic instrumentation

Implement:

```text
process attach
module map
function probes
basic block trace
```

Use existing OS mechanisms initially.

Add Intel PT backend.

---

# 207. PHASE 9 — Snapshot fuzzing / emulation

Implement:

```text
emulator abstraction
snapshot
restore
deterministic environment
```

Benchmark restoration overhead.

---

# 208. PHASE 10 — Static + taint

Implement:

```text
SecQL prototype
Oli-- source/OIR analysis
dataflow
taint
path explanations
```

---

# 209. PHASE 11 — Supply chain / containers

Implement:

```text
SBOM
CycloneDX
dependency graph
provenance
OCI image parser
```

Then reachability.

---

# 210. PHASE 12 — Mobile

Implement:

```text
APK
DEX
manifest
deep links
native libs
```

Then dynamic correlation.

---

# 211. PHASE 13 — AI security

Implement:

```text
prompt provenance
tool graph
agent transaction trace
permission lint
budget guard
```

Then:

```text
MCP analysis
RAG analysis
agentic test harness
```

---

# 212. PHASE 14 — Advanced research engines

Implement:

```text
LeakScope
StateRace
CacheGenome
Divergence Finder
SecTwin prototype
```

These are differentiating features.

---

# 213. PHASE 15 — GPU / massive-scale analysis

Use `oli.compute` for:

```text
corpus clustering
trace analytics
pattern scanning
side-channel statistics
binary similarity
```

Only after CPU correctness exists.

---

# 214. REQUIRED CLI

Initial CLI family:

```bash
oli sec capture
oli sec pcap
oli sec packet
oli sec binary
oli sec disasm
oli sec trace
oli sec fuzz
oli sec web
oli sec api
oli sec auth
oli sec crypto
oli sec supply
oli sec mobile
oli sec ai
oli sec report
```

---

# 215. EXAMPLE: PACKET REALITY CLI

```bash
oli sec capture \
    --interface eth0 \
    --planes xdp,tc,socket \
    --output trace.pcapng
```

Then:

```bash
oli sec capture explain trace.pcapng
```

Output:

```text
Flow #91

XDP frames:            41
TC ingress objects:    39
socket receives:       17
application reads:      8

GRO observed: yes
estimated coalescing events: 14

packet loss:
XDP -> TC: 2
TC -> socket: no loss proven; representation changed
```

The words "estimated" and "proven" matter.

---

# 216. EXAMPLE: PARSER TRIBUNAL

```oli
let result = tribunal.compare(url);

for interpretation in result {
    print(interpretation);
}
```

Possible output:

```text
nginx-like parser:
  path = /admin

framework parser:
  path = /public/../admin

difference:
  dot-segment normalization order
```

This is a lead for investigation, not an automatic vulnerability verdict.

---

# 217. EXAMPLE: AUTH MATRIX

```oli
security test PrivateObjectIsolation {
    let object = alice.create_private_note("hello");

    expect alice.read(object) => allowed;
    expect bob.read(object)   => denied;
}
```

The engine records:

```text
requests
responses
state
account
object ownership
```

---

# 218. EXAMPLE: REVERSE TRACE

```bash
oli sec trace ./server \
    --network request-17 \
    --show-functions
```

Potential view:

```text
POST /parse
 ↓
http_parse_request
 ↓
decode_payload
 ↓
parse_custom_format
 ↓
copy_field
```

Now the researcher can fuzz `parse_custom_format` directly.

---

# 219. EXAMPLE: FUZZ PROMOTION

Observed traffic can become a corpus seed:

```text
PCAP packet
 ↓
protocol parser
 ↓
message object
 ↓
grammar seed
 ↓
state fuzzer
```

This cross-subsystem workflow should be intentionally supported.

---

# 220. EXAMPLE: CRASH TO PATCH

Workflow:

```text
fuzzer crash
 ↓
CrashDNA
 ↓
minimized input
 ↓
SIR trace
 ↓
source mapping
 ↓
patch
 ↓
regression test
 ↓
replay evidence
```

---

# 221. CLAUDE CODE DEVELOPMENT RULES

For each subsystem create:

```text
docs/design/<subsystem>.md
docs/threat-model/<subsystem>.md
docs/tests/<subsystem>.md
bench/<subsystem>/
fuzz/<subsystem>/
```

Before implementation document:

1. threat model
2. hostile input assumptions
3. allocation behavior
4. copy behavior
5. concurrency model
6. platform privileges
7. failure modes
8. resource limits
9. fuzz strategy
10. benchmark strategy

---

# 222. DO NOT BUILD FAKE IMPLEMENTATIONS

Forbidden:

```text
hardcoded parser output
mock "scanner" that only matches strings
fake protocol implementation
empty fuzz command
pretend decompiler
hardcoded vulnerability verdicts
```

If a feature is not implemented:

```text
NotImplemented
```

must be returned clearly.

---

# 223. EVERY PARSER GETS FUZZED

Mandatory fuzz targets:

```text
HTTP/1
HTTP/2
URL
Unicode
DNS
TLS structures
JWT
ASN.1
X.509
PCAP
PCAPNG
ELF
PE
DEX
CycloneDX
```

---

# 224. EVERY ACTIVE TEST GETS SCOPE TESTS

Test:

```text
allowed host
denied host
allowed port
denied port
redirect leaving scope
DNS change
IPv4/IPv6 resolution
```

The scope engine must check post-resolution destinations where appropriate.

---

# 225. BENCHMARK HONESTLY

Never claim Oli-- is faster than:

```text
Wireshark
tcpdump
Suricata
Frida
AFL++
Ghidra
Burp
```

without a comparable benchmark.

Benchmark the component, not the marketing claim.

---

# 226. INNOVATION TARGETS

The following features should be treated as candidate flagship ideas for Oli--:

```text
Packet Reality
ArtifactGraph
Byte Provenance
Parser Tribunal
Divergence Finder
CacheGenome
StateRace
BusinessStateGraph
AuthMatrix
LeakScope
CrashDNA
SecretLife
StateTime
SecTwin
AgentCapabilityGraph
```

These names can change later.

The important point is that the concepts are native to the architecture.

---

# 227. FLAGSHIP IDEA: CROSS-DOMAIN SECURITY GRAPH

Long-term, all analyses should be able to contribute to one graph:

```text
NetworkArtifact
    ↓
ProtocolArtifact
    ↓
ApplicationOperation
    ↓
Process
    ↓
Function
    ↓
Source/OIR
    ↓
SecurityDecision
```

And separately:

```text
Identity
    ↓
Token
    ↓
Session
    ↓
API Object
    ↓
State Transition
```

Then connect the graphs.

This may enable queries impossible or awkward across today's separate tools.

---

# 228. FLAGSHIP IDEA: FIRST POINT OF DIVERGENCE

Many modern vulnerabilities arise because two components interpret the same input differently.

Make this a core research question:

```text
Where is the first point at which interpretation diverges?
```

Targets:

```text
protocol parsers
URL parsers
Unicode normalizers
cache vs origin
browser vs server
frontend vs backend
JWT decode vs verify
ORM vs database
proxy vs app
```

---

# 229. FLAGSHIP IDEA: SECURITY COST MODEL

Just like `oli.compute` exposes compute cost, `oli.sec` should expose test cost.

Example:

```text
network requests: 18
connections: 2
bytes transmitted: 31 KiB
parallelism: 1
state mutations: 0
scope crossings: 0
```

Before running an active test, user can inspect estimated impact.

---

# 230. FLAGSHIP IDEA: PROOF MINIMIZATION

When a test finds something interesting, automatically search for the lowest-impact reproducer.

Prioritize:

```text
read-only proof
self-account proof
isolated object
minimal requests
minimal bytes
```

This is useful for responsible bug bounty reporting.

---

# 231. FLAGSHIP IDEA: SECURITY TIME TRAVEL

Combine:

```text
network recording
process trace
filesystem snapshot
state model
```

to replay a failure.

This is difficult across real distributed production systems, so begin with:

```text
local services
containers
test environments
```

---

# 232. FLAGSHIP IDEA: OBSERVABILITY BEFORE EXPLOITATION

For every vulnerability class, build visibility first.

Example:

Instead of beginning request-smuggling support with payload generation:

```text
1. raw message model
2. frontend/back-end parser comparison
3. connection reuse observer
4. protocol translation trace
5. safe divergence detector
```

Only then consider controlled lab exploit modules.

---

# 233. MAINTENANCE RULE

Security evolves quickly.

Separate:

```text
engine code
```

from:

```text
security knowledge
```

Knowledge packs should update independently.

---

# 234. RESEARCH REVIEW LOOP

Every quarter Claude Code should generate:

```text
RESEARCH_REVIEW_YYYY_QN.md
```

Sections:

```text
new protocol standards
new web research
new auth standards
new fuzzing techniques
new hardware mitigations
new AI-security risks
new supply-chain standards
impact on oli.sec
features worth prototyping
features rejected
```

Do not add features merely because they are fashionable.

---

# 235. SUCCESS CRITERIA

`oli.sec` succeeds when a researcher can use one coherent native environment to:

```text
capture
inspect
trace
reverse
fuzz
compare
model state
test authorization
analyze crypto
analyze supply chain
analyze AI agents
generate evidence
```

without losing the ability to inspect raw bytes, machine instructions, or exact protocol state.

The desired experience is:

```text
high-level enough for security research
low-level enough for systems research
```

---

# 236. FIRST CLAUDE CODE TASK

Do NOT begin by implementing all modules.

Create the architecture documents and prototypes for these six foundations only:

```text
1. Artifact / ArtifactGraph
2. ByteView + Byte Provenance
3. Scope / authorized test model
4. Packet Reality architecture
5. Parser Tribunal architecture
6. SIR architecture
```

For each, deliver:

```text
formal data model
memory ownership
zero-copy rules
error model
threading model
example Oli-- API
test plan
fuzz plan
benchmark plan
security/threat model
```

Then write:

```text
OLI_SEC_ARCHITECTURE_V0.md
OLI_SEC_PACKET_REALITY_V0.md
OLI_SEC_PARSER_TRIBUNAL_V0.md
OLI_SEC_SIR_V0.md
OLI_SEC_SCOPE_V0.md
```

Do not start implementation until those designs are internally consistent.

---

# 237. FINAL DIRECTIVE TO CLAUDE CODE

At every stage ask:

> Does this feature merely reproduce an existing security tool, or does Oli-- allow us to connect machine-level, network-level, application-level, and security-state information in a way that existing fragmented toolchains usually do not?

Prefer the second.

The library should not be "a giant scanner."

It should become a:

# SECURITY RESEARCH OPERATING LAYER

built directly into the Oli-- ecosystem.
