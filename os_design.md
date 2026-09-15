# A machine model for cheap execution objects

A design note, not a proposal to build anything yet. It records what a
conversation converged on, so the settled parts stop being re-argued and the
unsettled parts can be measured.

## The question

Zircon and seL4 established that a capability-and-object foundation is
implementable, machine-checkable, and shippable. BEAM established that millions
of independently scheduled, independently failing execution objects are
practical when isolation is enforced by a runtime rather than by hardware.
Neither answers the other's question. This note is about the join:

> Can the fundamental execution object be made cheap enough, stackless enough,
> dynamically composable enough, and resource-accountable enough that "actor"
> becomes a native machine abstraction, without requiring every actor to be a
> hardware protection domain?

Cheap is the operative word, and it decomposes into three separately
measurable costs: the footprint of a parked execution context, the cost of
manipulating authority, and the cost of accounting for resources in flight.

## Non-goals

Stating these first, because most objections to a design like this are
objections to a workload nobody chose.

- **Not competing at line-rate packet forwarding.** The performance arguments
  against layered network services assume a datacenter forwarding workload. If
  that is the target, this design loses to kernel bypass and should not be
  attempted.
- **Not a general-purpose host for third-party binaries.** Fusion under an
  authority constraint puts a compiler in the trusted base, and a verified
  toolchain cannot ingest code it did not produce. Singularity hit this. It is
  an architectural entailment, not an ecosystem complaint, and it is accepted
  here rather than worked around.
- **Not POSIX.** Compatibility, if it is ever wanted, is a service built above
  the model, and it will be slow.
- **Not "BEAM implemented by the kernel."** The kernel supplies mechanism. The
  runtime supplies actor semantics. Collapsing the two was the original mistake.
- **Device recovery is out of scope.** The model can restart an execution
  context. It cannot restore a device that was mid-transaction. That requires a
  per-device recovery protocol which the machine model neither provides nor can.

## Invariants

What the machine must guarantee, before any mechanism is chosen.

**I1 — Confinement is capability-determined.** An execution context can name
only what it was given at spawn, what it created, and what was explicitly
transferred to it. There is no global registry and no ambient name that
resolves to authority.

**I2 — Exhaustion is quota-determined, and separately.** Holding a capability
entitles a context to *address* a resource, never to *consume* an unbounded
quantity of it. Confinement and exhaustion are different threats and are
answered by different mechanisms; neither substitutes for the other.

**I3 — Isolation is the default; sharing is an explicit operation.** Every
sharing relationship has a named grantor, a recorded derivation, and an
attenuation. Two contexts that were never explicitly joined share nothing.

**I4 — Every in-flight message has exactly one accountable owner at every
instant.** Ownership transfers at a defined point, and there is no interval in
which a message is charged to nobody or to both parties.

**I5 — Exposure to any single peer is bounded.** No peer can pin more than a
declared window of another context's resources. Sender-pays accounting without
this invariant lets a slow or hostile receiver exhaust its sender.

**I6 — Cancellation is total.** Any operation that can block can be withdrawn,
and the outcome is always *delivered* or *withdrawn*. An indeterminate outcome
is a defect in the model, not a case to be handled by callers.

**I7 — A parked context's footprint is independent of the depth of the
computation it was executing when it parked.** This is the load-bearing
invariant for density. It states the requirement without choosing the mechanism
(see Fork 1).

**I8 — Budgets are shares of a parent's budget.** The spawn tree is the
accounting tree; the root budget is the machine. Absolute quotas are rejected:
they are either over-provisioned or they fail under legitimate load, and a
dynamic reallocator is itself a context needing a budget.

**I9 — Fusion may not collapse an authority boundary the capability graph does
not already permit collapsing, and every fusion decision is emitted as an
artifact checkable against that graph by a verifier smaller than the compiler.**
Without this, a bug in an optimizer pass is a privilege escalation.

**I10 — Device-initiated memory access is inside the protection model.** A
driver's DMA reach is a capability subject to the same graph as CPU-initiated
access. A bus-mastering device outside the graph makes every other invariant
decorative.

**I11 — Revocation is a common-path operation.** Dynamic instantiation implies
capability churn implies revocation churn. Its cost is budgeted like any other
frequent operation, not treated as an exceptional path.

## Primitive machine model

Four object kinds:

    Memory object     content, independently namable, no intrinsic owner
    Execution context registers, budget share, scheduling state,
                      parked representation (Fork 1)
    Capability        (object, permissions, lifetime, derivation)
                      held in a per-context table, unforgeable,
                      not addressable as memory
    Channel           bounded queue, producer capability,
                      consumer capability, accounting policy

Five operations:

    spawn(code, memory, capability-set, budget-share)
        -> execution context, and a capability to it

    send(channel-cap, message, window)
        -> accepted | blocked(future) | rejected

    receive(channel-cap)
        -> message, with ownership transfer

    share(object-cap, target, attenuation)
        -> derived capability

    revoke(capability)
        -> derivation subtree invalidated

Accounting is not an operation. It is a property every operation carries.

There is deliberately no `lookup`. Discovery is what a parent handed down, which
is what makes I1 hold without a separate confinement mechanism: the same act
answers "how do I find it" and "may I use it."

## Three unresolved forks

These are research questions. Picking winners before measuring is how the
conversation went wrong the first time.

### Fork 1 — stackful or stackless

The decision criterion is I7: parked footprint as a function of computation
depth.

If parked footprint is O(depth), the system is bounded by stack memory
regardless of how small the descriptor gets, and density is capped well below
the interesting range. If it is O(1) in depth — continuations, compiled state
machines — density becomes a question about descriptors and mailbox windows
only.

Measure: suspend and resume, migration across protection domains, blocking,
cancellation, growth, and debuggability. The known cost of the stackless branch
is function coloring and the inability to park inside code not compiled for it —
a cost substantially reduced by the third-party-binary non-goal, which is one of
the few places these decisions help each other.

The experiment that matters is not how many contexts can be spawned. It is what
a *parked* one costs.

### Fork 2 — synchronous or asynchronous IPC

Synchronous rendezvous, the L4 answer:

    A ------------------> B
            rendezvous

Nothing is in flight, so I4 and I5 are satisfied by construction and there is no
accounting question at all. The price is coupled scheduling and a sender whose
liveness depends on a receiver it may not trust.

Asynchronous bounded channel:

    A --> [ bounded queue ] --> B
                 |
                 +-- cancellation

Decoupled, and needs I4, I5 and I6 to be genuinely implemented rather than
asserted.

Measure both, and note that latency and throughput are the easy half. The result
that decides the fork is behavior under a *hostile or arbitrarily slow
receiver*, plus fairness, plus how much memory is held in flight at steady
state.

### Fork 3 — revocation mechanism

Four candidates, none free:

    derivation tree     seL4's CDT. Correct, proven, and revoke has no bounded
                        execution time; seL4 needed preemption points for its
                        real-time analysis.
    indirection         bounded revoke, at the price of a dereference on every
                        capability use.
    epoch / versioning  cheap validity check, monotonic state growth, ABA
                        hazards.
    lifetime scoping    cheap when the object graph's shape matches the
                        lifetime's, and silent about transitively derived
                        capabilities that outlive their grantor.

The measurement must separate **steady-state access cost** from **revocation
cost**, and weight them by expected churn — which I11 says is high. A design
that made every capability use expensive in order to make revocation cheap has
probably made the wrong trade, and only separated measurements reveal that.

## Derived abstractions

The model earns its keep only if the familiar things fall out of it.

    Process     an execution context, its memory objects, its capability set,
                and its budget share. Nothing further.
    Actor       a small execution context holding one channel and a receive
                loop. A library pattern, not a kernel concept.
    Service     a spawn-factory capability plus a protocol. Instances are
                ordinary contexts; there is no privileged service class.
    File        a capability to a memory object managed by a filesystem service.
    Filesystem  a service holding a block-device capability.
    Driver      a service holding a device capability and a DMA capability
                (I10). Restart returns the context, not the device.
    Network     services with independently owned state, referenced through
                capabilities, whose execution graph may be fused where I9
                permits. The architectural graph and the execution graph are
                deliberately not required to be the same graph.

## Experiments, in order

**E0 — Baseline and attack.** Build the smallest object/capability/channel/
budget machine that can hold the invariants, then attack it: capability churn,
channel flooding, slow receivers, cancellation races, context churn, revocation
concurrent with IPC, resource exhaustion, priority inversion, and mass
suspension. Settles whether the invariants are jointly implementable, and which
one breaks first. That ordering — which breaks first — is the actual output.

**E1 — Parked footprint (Fork 1).** Depth-independence, per I7. Include timers:
pending-timer data structures that are fine at a million are a different
structure at a hundred million. Note that under sender-pays accounting the
mailbox memory sits on the sender's side, which changes what is being measured.

**E2 — IPC under adversarial receivers (Fork 2).** Both branches, measured
against a receiver that is trying to hurt the sender rather than one that is
merely slow.

**E3 — Revocation curves (Fork 3).** Steady-state access cost and revocation
cost as separate curves, over a churn rate taken from E0 rather than assumed.

**E4 — Everything about devices and networking.** Only after E1 through E3.
These are the questions the conversation reached for first and they cannot be
answered before the model underneath them is priced.

## Existing systems as evidence

Read for constraints that appeared, not for verdicts. History is evidence, not
a theorem — the useful question is always which constraint appeared and whether
current hardware still imposes it.

**Zircon / Fuchsia.** Objects, handles in per-process tables, bidirectional
bounded channels carrying handles, jobs as a hierarchical accounting tree, and
discovery via namespaces handed over at spawn. That is very nearly this
primitive list, shipped on real hardware at product scale, which makes it the
strongest evidence the foundation is sound. Extract: why channel IPC is not
faster than Linux syscalls for equivalent work, and what handle exhaustion and
channel backlog look like as operational problems rather than design ones.

**seL4.** Settles that a capability kernel can be machine-checked, and prices
derivation-tree revocation. Also a caution directly relevant to I10: the
original proof assumed away DMA.

**BEAM.** Settles that enormous numbers of parked processes are practical when
parked footprint is depth-independent and isolation is runtime-enforced. The
counter-evidence is equally valuable: unbounded mailboxes are its canonical
production failure, which is precisely what I4 and I5 exist to prevent.

**CHERI / Morello.** Settles that capabilities can be a hardware primitive with
per-pointer bounds and permissions. Open where it matters most here: temporal
safety. Revocation sweeps are the current answer and they are not cheap, which
is Fork 3 appearing again one level down.

**io_uring.** Settles that "the kernel establishes the channel and then leaves
the data path" works at scale — with a small number of rings and one privileged
counterparty. It does not settle context-to-context rings, where the number of
regions grows with the communication graph rather than with the number of
contexts. Expect a brokered path for the long tail and rings only for hot pairs.

**L4.** Prices synchronous IPC, and is the reference number for Fork 2's
synchronous branch.

**Singularity.** Prices the compiler-in-the-trusted-base branch, and
demonstrates the consequence recorded in the non-goals.

## What would falsify this

If E1 shows parked footprint cannot be made depth-independent without giving up
the ability to write ordinary sequential code, the density argument collapses
and what remains is Zircon with better accounting — worth having, but not this.

If E3 shows revocation cannot be made cheap at high churn without an
indirection that taxes every capability use, then dynamic instantiation is
priced out, and the design should retreat to long-lived services with static
authority graphs.

Either result is worth more than continuing to reason about it.
