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
two numbers: the process that has the microphone, and the recording it is
filling. A press that finds them stops, a press that does not starts. The pid
is checked against `/proc` rather than believed, because a session that ends
mid-sentence leaves the number behind, and a press reading the number alone
would write down a file from an hour ago.

Stopping waits for the recorder to go before it reads the file. A wav says how
long it is in its first bytes and the length is only known once the recording
ends, so a file read too early is a header claiming nothing follows it, and the
hearing answers with silence for a sentence sitting on the disk.

Each recording is named after the press that made it, `said-1234.wav`, and the
press that stops one is the only thing that ever touches it. That is not
tidiness. Every recording used to be `said.wav`, and the press that stops a
recording goes on to read it, which takes as long as the hearing takes -- so a
press arriving inside that window started a new recording into the same name
and had it deleted underneath by the first press finishing up. What that leaves
is a recorder writing to a file with no name left in the directory, and the
next press handing whisper a path to nothing.

Which is a sentence eaten, silently, and it is not a rare case. It is what
pressing the button again because it seemed not to work does every time. The
button seemed dead, so it got pressed again, and the pressing again was the
thing breaking it.

## Nothing said

Whisper does not answer an empty room with an empty answer. It answers with
"Thank you." -- confidently, at every gain this machine has, and no amount of
`--suppress-nst` or moving the no-speech threshold stops it while still hearing
real speech. So a recording is measured before it is asked about.

Not for how loud it is. Loudness does not survive the gain knob: this
microphone hearing nothing is 0.4 per cent of full scale at one boost setting
and 14 per cent at another, so any line drawn across the level sits above a
real sentence at one end of that knob or below an empty room at the other. It
is measured for its shape instead. A room is the same all the way through and a
person is not.

The recording is cut into twenty-millisecond frames and two of them are kept:
the middle one, which is the room, and the one nine tenths of the way up, which
is whoever is talking. Measured on this device, silence sits between 1.3 and
1.9 times its own middle at every gain the microphone has, and speech at 10.9.
The line is 2.5 -- nearer the room than the speech, because the two mistakes do
not cost the same. A guard set high eats a sentence somebody actually spoke,
quietly, with no way to tell that it did. A guard set low lets a "Thank you."
through into a field now and then, where it is visible and one backspace away.

There is a second way to be speech, for somebody talking without drawing
breath: louder, all the way through, than any room this machine has recorded.

## What is kept

Nothing.

The recording lives in `$XDG_RUNTIME_DIR/console/voice/`, which on this machine
is a tmpfs -- memory, not the disk -- and it is deleted as soon as the reading
of it ends, however that reading ends. Not only when it worked: the taking-away
is wrapped around the whole of the work rather than written at the bottom of
it, because the bottom of it is the one path a failure never reaches, and a
failure is exactly the case where somebody's voice would otherwise sit there.

What survives that is a recording nothing is reading any more -- a session that
ended mid-sentence, a `dictate` that was killed. Those go with the runtime
directory at logout, which is what a runtime directory is for.

Nothing is sent anywhere. The hearing is a program on this machine reading a
file on this machine, and the only thing that leaves the room is the words,
into the box that had the focus.

## The model

`ggml-large-v3-turbo-q5_0.bin`, in `~/.local/share/console/voice/`, fetched
once from the whisper.cpp project on the first press that needs it.
`dictate --fetch` asks for it in advance.

Large because what is dictated here is English, Dutch, Thai and Chinese, and
the small models are good at two of those. That was the original reason and it
is still the reason. What is worth writing down is the few hours it was not.

Timed on the processor, this model spent 19.8 seconds reading a two-second
sentence -- 19.83 of it in the encoder and 0.015 in the decoder. Turbo is the
large encoder with a cut-down decoder, so it buys back the part that was
already free and pays full price for the part that is not. Small did the same
clip in 4.5 seconds and was put in for that alone, at a cost to two of the four
languages that nobody wanted to pay.

Then the hearing was pointed at the machine's own graphics, and this model came
back in 2.7 seconds -- quicker than small had ever managed on the processor. So
the trade was handed back. The accurate model is now also the fast one, and the
right fix turned out to be under the model rather than in it.

| | processor | this machine's graphics |
| --- | --- | --- |
| small | 4.5 s | 0.8 s |
| large-v3-turbo | 19.4 s | **2.7 s** |

Language is detected per press rather than configured: which one is being
spoken is a thing the recording knows and the button does not. It is not free.
Detection is a whole extra pass of the encoder, and it is exactly double: 2.7
seconds asked against 1.4 told. It is worth it here, where the four languages
are the whole point of the model.

It is not a package and not in this repository. Weights are not source, and a
device rebuilt from the manifest fetches them the first time somebody speaks.

## The hearing

Not the packaged one. `whisper-cpp` from the repositories is built for the
processor alone -- `system_info` reports no backend beside the CPU -- on a
handheld whose iGPU is built for exactly the arithmetic the encoder is made of.
There is no build in any repository that knows about the card in this device,
so the device makes one: whisper.cpp at a pinned tag, configured with
`-DGGML_VULKAN=ON`, built static, and the one program carried out of the tree
and kept beside the model.

Measured on the same clip:

| | processor | this machine's graphics |
| --- | --- | --- |
| small | 4.5 s | **0.8 s** |
| large-v3-turbo | 19.4 s | 2.7 s |

Static because the machine already has a different `libwhisper` in `/usr/lib`,
and a build carrying its own libraries to be found first is a build that
quietly decides which of the two is running. One file, under one name, beside
a model.

It is built once, on the first press that wants it, and that press does not
wait for it: a C++ project takes four minutes and the sentence somebody just
spoke is worth more than the speed of the one after it. So the press that finds
no build starts one behind itself and uses the packaged hearing this time.
`dictate --fetch` asks for both the model and the build in advance.

The tag is pinned. This is a compiler being pointed at somebody else's
repository on a device somebody is holding, and a branch is whatever it happens
to be on the morning the machine is rebuilt.

It is the only thing on this desktop built from anybody else's source, and it
is not a habit worth acquiring.

## What it does not do

The daemon that reads the paddle is paused while the on-screen keyboard is up,
so the keyboard and the microphone are two answers to the same question rather
than two halves of one. Close the keyboard, then speak.

There is no indicator on the bar. A notification says it is listening and
another says it is writing down, both replacing the last rather than stacking,
because those are the states of one thing happening rather than a list of
events.

Both stay up until they stop being true. A state is not an event: listening is
true until the next press, and writing it down is true until the words are
there. "Writing it down" used to take itself off the screen after two seconds
while the hearing was still going -- so the message left before the words came,
and the wait it existed to explain was the part somebody sat through with
nothing on the screen at all. What ends the pair is the ending: the words
themselves for a moment, or that nothing was said, or that it could not be
read.

Which means a press that is never followed by a second one leaves "Listening"
on the screen, and the bell in the bar counts it until it is taken down. That
is the honest reading -- something is waiting, and it is.

Neither is waited for. The first time this ran on the device there was no
notification daemon on the machine at all -- see [notifications](notifications.md)
-- and D-Bus answers a name nobody owns by trying to start it and failing fifty
seconds later. Waited for, that is fifty seconds between the press and the
microphone, and fifty more between the second press and the words. The button
worked perfectly and looked completely dead: the recording ran, whisper read
it, and the sentence was typed out nearly two minutes later into whatever had
the focus by then.

So `told` starts `notify-send` and walks away. It is worth saying again in the
one place it is easy to get wrong: nothing this desktop says about itself is
ever worth the thing that was doing the saying.
