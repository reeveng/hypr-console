# Dictation

The bottom left paddle takes what is said and types it where the keyboard
would have. One press starts listening, the next writes it down. There is
nothing to hold and nothing to aim at, which is what a button on the back of a
device has to be.

`crates/console-voice` is the whole of it: a library that decides the shape of
each call, and `dictate`, which makes them.

## What it is made of

Three programs already on the machine do the work.

| | |
| --- | --- |
| `pw-record` | takes the microphone, one channel at 16 kHz, which is the shape the hearing wants |
| `whisper-cli` | turns the recording into words |
| `wtype` | types them into whatever holds the focus |

Nothing in the library starts a program, so what would be run can be asked for
and read in a test without a microphone in the room.

## The two presses

The recording is the state. `$XDG_RUNTIME_DIR/console/voice/taking.pid` holds
the process that has the microphone: a press that finds it stops, a press that
does not starts. The pid is checked against `/proc` rather than believed,
because a session that ends mid-sentence leaves the number behind, and a press
reading the number alone would write down a file from an hour ago.

Stopping waits for the recorder to go before it reads the file. A wav says how
long it is in its first bytes and the length is only known once the recording
ends, so a file read too early is a header claiming nothing follows it, and the
hearing answers with silence for a sentence sitting on the disk.

## The model

`ggml-large-v3-turbo-q5_0.bin`, in `~/.local/share/console/voice/`, fetched
once from the whisper.cpp project on the first press that needs it.
`dictate --fetch` asks for it in advance.

Turbo rather than one of the small models because what is dictated here is
English, Dutch, Thai and Chinese, and the small ones are good at one of those.
Quantised because the difference is half a gigabyte of disk against a
difference in the words nobody has been able to hear. The language is detected
per press rather than configured: which one is being spoken is a thing the
recording knows and the button does not.

It is not a package and not in this repository. Half a gigabyte of weights is
not source, and a device rebuilt from the manifest fetches it the first time
somebody speaks.

## What it does not do

The daemon that reads the paddle is paused while the on-screen keyboard is up,
so the keyboard and the microphone are two answers to the same question rather
than two halves of one. Close the keyboard, then speak.

There is no indicator on the bar. A notification says it is listening and
another says it is writing down, both replacing the last rather than stacking,
because those are the states of one thing happening rather than a list of
events.
