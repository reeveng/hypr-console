# State machines

Everything on this desktop that holds a state and decides something is meant
to be written as one machine, in one way, so that a run can be recorded and
replayed, a screen can come back exactly as it was left, and the map can draw
how control moves rather than only which program starts which.
`console-core-state-machine` is that way, and it is arithmetic: nothing in it
touches the machine it runs on. `console-actor` is one of the things that runs
it, and where a kept machine rests is the actor's too, because the disk is the
machine and a core crate never reaches it.

## The words are effect's

`@effect/experimental/Machine` already named every part of this, and a reader
who has met it knows what each word means before opening a file:

- a `Machine` is `initialize`d from an `Input` and, when it was saved, from
  the state it had before;
- it `handle`s one `Request` at a time and answers with the next `State`;
- one that can be written down and read back is a `SerializableMachine`, and
  the two functions that do it are `snapshot` and `restore`;
- where it rests is a `KeyValueStore`, and what goes wrong there is a
  `PlatformError`; what goes wrong reading a snapshot is a `ParseError`;
- one running on a thread of its own is an `Actor`, started by `boot`, sent
  requests by `send`, read by `get`, and ended by closing its `Scope`;
- a request it fell under is a `MachineDefect`.

`vocabulary.conf` holds the lines, so the next name argued about is looked up
there rather than here.

What is not effect's is the one decision everything else stands on: nothing
here performs anything. A handler in effect forks the work it wants done. A
handler here offers an `Effect` to a `Queue` and returns, and whoever drives
the machine carries the effect out. That is the split
`console-program-contract` was written on, and it is what makes a machine a
function a test can call on a laptop with no compositor, no controller and no
network.

## A handler cannot fail

`handle` returns `Result<State, Never>`. A request that means nothing in the
state it arrives in is an arm that returns the same state; a fault met while
an effect was being carried out comes back as a request of its own, and the
machine decides what it means. An error a handler could return would be a
third outcome beside the state and the effects -- one with no state after it,
and the one nobody draws on the map.

The first draft of this crate had a `Fault` per machine and a `step` that could
return it. Every machine written to it named its fault `Never` or `Full`, the
second of which was a `Vec` refusing to grow, which Rust answers by aborting
anyway.

## The state is moved

`handle` takes the state by value and gives one back. A large state -- a
playlist, a library of books -- is never copied to change a small part of it,
and a machine made of machines hands each child its own part and puts back
what the child returns. The draft before asked every state, request and
effect to be `Copy`, which nothing that holds a path, a name or a list can be:
the music player's state holds a path and every program's effects hold a
command, so the programs that draft named as the first to move could not have.

## Saving is a machine's to choose

Every machine can implement `SerializableMachine`; none has to. The ones that
should are the ones holding something a person would be annoyed to lose: the
page a book was open on, which workspace an app was on, a half-typed line.

`initialize` is handed the state a saved machine had and decides what of it to
keep. That decision is the machine's because only the machine knows which of
its parts are owned and which are read. What a person owns comes back. A
reading of the world -- which panel was up, how full the battery was, whether
the network was there -- is asked again, because a reading restored after a
reboot is one that is confidently wrong, and `docs/programs.md` already made
the rule for held state: hold only what something refreshes.

A snapshot is a mark, the layout's version, the state, and a CRC-32 over all of
it. The sum is read first, so a file cut short by a power cut or flipped by a
bad write is `Corrupt` before any field is decoded. A machine whose layout
changed reads the old one in `migrate`; one that cannot is told the version it
does not read.

What happens then is two different decisions, and they are different on
purpose:

- a snapshot that is there and **will not decode** -- corrupt, or from a layout
  nobody migrates -- is said in the journal and the machine starts fresh.
  Losing a page somebody was on is a smaller fault than an app that will not
  open;
- a key that is there and **will not be read** -- a permission, a directory
  where the file should be -- refuses to boot. Starting fresh over it would
  overwrite, at the first idle moment, something that may be perfectly good.

## When a machine is saved

Not on every request. A held stick sends hundreds of them a second, and a write
synced to the disk for each is a handheld spending its flash on the same few
bytes, with the input waiting on `fsync`. The actor writes when its mailbox
runs dry, and only when the bytes differ from what it wrote last; closing its
scope writes once more on the way out. A power cut loses at most what arrived
since the last quiet moment, which on a person's timescale is nothing.

## The actor

`console-actor` is `console-panel`'s `actor` moved down and made to run
any machine, because a state with one owner and a mailbox is not a panel's
idea. Two things changed on the way.

**The effects go back to whoever sent the request.** `send` waits until the
request is handled and returns what it decided, and the sender carries it out.
The actor never does, because the sender is the one who knows where an effect
can be carried: a panel's are carried on GTK's thread, which is not the
actor's. The draft before had a send that returned at once, and the effects of
those requests were queued where nothing read them -- a machine that decided
and was never obeyed. A fire-and-forget send can come back for machines whose
effect is `Never`, when one asks for it.

**A fall costs the state and nothing else.** A panic in `handle` is caught,
the machine is initialized again from where it was last kept -- or from its
input, when it is not kept -- and the sender is told `MachineDefect` with the
effects that put the world back. The draft before stepped every request twice,
once to catch a panic and again uncaught, so the request that fell took the
thread down with it.

## Where it is going: every machine as one

The aim is that every machine a person owns -- the handheld, the desktop, a
server, a phone -- behaves as one: a request sent here can be handled there,
and a book put down on one is open at the same page on the next.

Nothing above stands in the way of that, and most of it was decided so that
nothing would. A request is a value, so it can cross a wire. A state that is
`Serializable` is bytes, so it can be handed to another machine as easily as to
a file. An actor is reached only through `send`, so an actor on another machine
is an `Actor` whose mailbox is a connection rather than a channel, and nothing
that sends to it has to know.

What is not written is the wire. The first sketch reserved a crate each for
transport, discovery, membership, gossip, remote calls and a registry, and none
of them held anything yet: a crate written before anything asks for it is the
piece most likely to be built bigger than anything needs, and its name decides
its shape before the first request does. The plan is in the backlog. The first
thing to write is one request crossing one wire between two machines, and the
crates come out of what that needed.

## What is not here

**An offline queue.** It persisted every request a remote actor could not take
and replayed it on reconnection. A request replayed an hour late into a state
that has moved on is a request deciding against a world it never saw; what a
machine owns is already saved, and what it read is asked again.
