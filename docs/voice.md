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

## The marks

Whisper writes prose. Everything it hears is a sentence to it, so it capitalises
the front and puts a full stop on the back and commas through the middle. That
is right for a paragraph and wrong for most of what this paddle is pressed for:
a search box, a filename, a name to look somebody up by. "Settings." looks for a
word this machine does not have, and the stop is then a backspace to find on a
device whose whole problem is that it has no keyboard.

So the short things come back bare. Six words or fewer and everything that is
not a letter, a number or a space is taken out; past that it is left exactly as
whisper wrote it, because somebody dictating a message that long would only have
to put the stops back by hand, which is the same chore the other way round.

| said | typed |
| --- | --- |
| "Settings." | `Settings` |
| "Blåhaj, please?" | `Blåhaj please` |
| "สวัสดีค่ะ" | `สวัสดีค่ะ` |
| "I'll be late, so don't wait for me, I'll find you there." | unchanged |

An apostrophe or a hyphen with a letter on either side of it stays, because
between two letters those are spelling rather than punctuation: `don't` and
`well-known` are one word each, and a rule that pulled the marks out of them
would be a rule that misspells things. A mark that goes becomes a space rather
than nothing, so what stood on both sides of it stays two words.

Six is a guess at where a name turns into a sentence and is meant to be one. The
two mistakes are not the same size: a stop left on a search term is a wrong
search, and a stop missing from a message is a message somebody reads anyway.

Words are counted by the gaps between them, except in the scripts that have
none. A Thai sentence puts its spaces between phrases and not between words, so
it is one word to anything counting gaps and would come back stripped however
long it ran. Its characters count for themselves instead. That is not one word
each, but the only thing this number is asked is which side of the line it
falls on.

### The marks that are not marks

Thai hangs its vowels and its tones on the consonants rather than beside them,
and to anything asking whether a character is a letter they are not: they are
nonspacing marks, which is the same answer a comma gets. So the rule above,
left to itself, took the vowels out of every Thai word it was given and broke
what was left into pieces at each place one had stood.

| said | typed | should have been |
| --- | --- | --- |
| "สวัสดีค่ะ" | `สว สด ค ะ` | `สวัสดีค่ะ` |

Which is every short thing said in Thai -- which is most of what this paddle is
pressed for -- failing in the way that is hardest to see. Something arrives, it
is in the right script, and it is not what was said. A mark that stands on a
letter is now part of the letter, and only the marks that stand beside one are
taken off.

Dutch had a smaller version of the same fault, found by asking what a good test
of this would even be. The rule that keeps `don't` whole looks for a letter on
either side of the mark, and several of the commonest words in Dutch put the
apostrophe at the front, where there is no letter to the left of it:

| said | typed | should have been |
| --- | --- | --- |
| "'s ochtends drink ik koffie" | `s Ochtends drink ik koffie` | `'s Ochtends drink ik koffie` |

A quotation looks the same to that rule, so the two are told apart by how much
follows the mark: `'s` is an apostrophe, one letter and the end of the word,
and `'hello'` is an apostrophe and then a whole word. The first is kept and the
second is still taken off, which is the way round that matters -- one of them
is a word somebody said and the other is a mark nobody dictated.

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

Large because what is dictated here is English, Dutch and Thai, and the small
models are good at one of those. That was the original reason and it is still
the reason. What is worth writing down is the few hours it was not.

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

It is not a package and not in this repository. Weights are not source, and a
device rebuilt from the manifest fetches them the first time somebody speaks.

Turbo is worth one caveat, and it is about the language it is least good at.
Turbo is large-v3 with the decoder cut from thirty-two layers to four, and
OpenAI's own note on it is that it holds up against large-v2 everywhere except
a few languages, of which Thai is one. Which matters here more than it does
almost anywhere else. It has not been measured on this device and it should be:
on a two-second sentence the encoder is essentially all of the work -- 19.83
seconds of the 19.85 on the processor -- so the decoder turbo cuts down is the
part that was already free, and full large-v3 may cost very little of the 2.7
seconds and be better at one of the three languages. See
[Is this the right hearing](#is-this-the-right-hearing).

## Which language

Detection was the whole of it once: which language is being spoken is a thing
the recording knows and the button does not. That is true of a sentence and
false of a word.

Detection is a guess made on what was said, and most of what is said to this
paddle is one or two words into a search box. There is not enough of them to
guess from, and what whisper guesses when there is not enough is English. So a
Dutch word came back as the English word it sounds nearest to and a Thai one
came back as English letters spelling the sound of it -- which is the case this
button is pressed in most, failing in the way that is hardest to see. It is a
word, it is spelled correctly, and it is the wrong word.

So it can be told instead. Settings, Configuration, Dictation:

| | |
| --- | --- |
| Whichever is spoken | ask the recording, as before |
| English | |
| Dutch | |
| Thai | |

Somebody writing Dutch all afternoon says so once, and every press that
afternoon is read as Dutch. It is also half the wait: detection is a whole
extra pass of the encoder, and it is exactly double -- 2.7 seconds asked
against 1.4 told.

The choice is a line in `~/.config/console/defaults`, which is the file the
search engine is already written into, under `dictation`. It is read on every
press rather than held anywhere, because a press is the next thing that happens
after the panel closes and there is nothing in between to tell.

Asking is still what it does until it is told otherwise. There is no language
to default to that is not wrong for two of the three, and a guess that is
sometimes wrong beats a setting that is always wrong for somebody.

The list is not what whisper can hear. The model has ninety-nine languages in
it and "whichever is spoken" reaches all of them; the list is the short one of
the languages worth a row here. Chinese was on it and is not any more, because
nobody was dictating any, and a row nobody presses is a row between somebody
and the row they wanted.

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

## Is this the right hearing

It was chosen because it worked, which is not the same as having been compared
against anything. So, compared, in September 2026.

Three things decide it and the third is the one that does most of the deciding.
Nothing may leave the machine. It has to run on this handheld's own graphics,
which means a Vulkan backend, because there is no CUDA here and the processor
alone is nineteen seconds. And it has to hear English, Dutch **and Thai**.

Thai removes most of the field on its own.

| | Thai | Dutch | runs here | |
| --- | --- | --- | --- | --- |
| whisper.cpp large-v3-turbo | yes | yes | Vulkan | **what this runs** |
| whisper.cpp large-v3 | yes, better | yes | Vulkan | worth measuring |
| Parakeet TDT 0.6b v3 | **no** | yes | NeMo or ONNX, no Vulkan | out |
| Canary 1b v2 | **no** | yes | as above | out |
| Moonshine | **no** | no | English only | out |
| Vosk | **no** | yes | CPU | out on accuracy anyway |
| Qwen3-ASR 0.6b / 1.7b | yes | yes | llama.cpp, Vulkan | the one to watch |
| anybody's API | yes | yes | somebody else's computer | out on the first rule |

The fast models -- Parakeet and Canary, which are the ones anybody
benchmarking English would tell you to use, and which are genuinely quicker
than this -- are trained on twenty-five European languages. Thai is not one of
them. Moonshine is English. Vosk has no Thai model worth the name and is a
generation behind on the languages it does have. The hosted services are the
best transcription there is and the first line of this document is that nothing
is sent anywhere.

What is left is whisper, and one real challenger.

**Qwen3-ASR**, from January 2026, is thirty languages including both Dutch and
Thai, in 0.6b and 1.7b, and there is a GGUF of it under ggml-org's own account
and support for it in llama.cpp's multimodal path since about April 2026. On
paper it is what would replace this. In practice, not yet, and for reasons that
are about this device rather than about the model: it means a second engine
built from somebody else's source rather than the one already here, a model and
an mmproj rather than a model, 2.17 GB at eight bits against 574 MB, and
llama.cpp's issue tracker still has open reports about Qwen3-ASR transcribing
incorrectly and about the shape of what it returns. It is worth trying when
those close. It is not worth the paddle today.

So the engine is right, and the honest answer about the model is that it has
one loose end: turbo is the model that is weakest at Thai, and the reason turbo
was chosen -- speed -- is a reason that mostly does not apply to what this
button does. Turbo cuts the decoder, and on a two-second sentence this device
spends all but a hundredth of a second in the encoder, which turbo does not
touch. Measuring `ggml-large-v3.bin` against the one in use, on Thai, is the next
thing to do here. `tools/voice-compare` is that measurement.

The rest of what is wrong with dictation on this machine was never the model.
It was a language guessed from one word, and a Thai word taken apart by a rule
about full stops. Both of those are above.

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
