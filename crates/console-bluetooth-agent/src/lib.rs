//! Introducing a device to this machine over Bluetooth, and answering bluez
//! while it happens.
//!
//! Three things have to be true before a keyboard someone has just bought
//! works twice. Pairing exchanges the keys, trusting is what lets it back in
//! after a reboot without anyone being asked again, and connecting is what
//! makes it work now. bluez has a word for each, and a press on a row means
//! all three.
//!
//! Pairing is a conversation, and bluez only holds it with an agent: the
//! program that answers what the radio cannot decide by itself -- a code to
//! put in front of somebody, a device asking to be let in. The tab used to
//! pair by running `bluetoothctl pair`, and bluetoothctl registers no agent
//! when its command is on its own line. With none, bluez pairs as a machine
//! that can show nothing, keeps the adapter unpairable, so a Bluetooth LE
//! device pairs without keeping its keys, and turns away the device that
//! connects back before it has been trusted. Nothing had ever been paired from
//! the tab, and that was the second of two reasons.
//!
//! So the program that pairs is the agent, for exactly as long as the pairing
//! lasts. It says it can show a number and nothing else, which is the truth
//! about a panel that has started a pairing and then gets out of the way: a
//! keyboard is given a code to type, headphones, a pad and a mouse are asked
//! nothing, and "do these two numbers match" -- the one question with no one
//! here to answer it -- is never put. While it is registered it answers for
//! the one device it was started for and turns everything else away, and when
//! it ends the adapter is unpairable again, which is how a phone holds it
//! too: pairable while the person is pairing.
//!
//! `introducing` is what a press asks for and what is said when it did not
//! happen, `agent` what bluez is answered and what its errors mean, and
//! `introduction` the conversation itself, as a machine: nothing in this
//! library waits on anything or opens a socket. The program is the loop that
//! carries the machine's calls to the system bus.

pub mod agent;
pub mod calls;
pub mod cards;
pub mod command;
pub mod errors;
pub mod introduction;
