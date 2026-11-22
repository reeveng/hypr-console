# KBVM

[![crates.io](https://img.shields.io/crates/v/kbvm.svg)](http://crates.io/crates/kbvm)
[![docs.rs](https://docs.rs/kbvm/badge.svg)](http://docs.rs/kbvm)
![MSRV](https://img.shields.io/crates/msrv/kbvm)

KBVM is a rust implementation of the XKB specification and associated protocols. It
supports

- creating keymaps from XKB files,
- creating keymaps from RMLVO names,
- creating keymaps from X11 connections,
- creating a composition state machine from XCompose files, and
- loading the RMLVO registry.

A keymap can be turned into a compositor-side state machine or a client-side lookup table.

## MSRV

The MSRV is `stable - 3`.

## License

This project is licensed under either of

- Apache License, Version 2.0
- MIT License

at your option.

## Shared symbol and table ownership in this workspace

`console-input-key-symbols` owns the key symbols, names, character mappings,
casing, and constants. This crate re-exports the same `Keysym` type and keeps
XKB compilation and modifier/group lookup. `console-core-perfect-hash` reads
the precomputed candidate tables used by both the symbol reader and XKB keyword
reader; each owner still validates its own names. ASCII integer parsing is
shared through `console-core-number-conversion::FromBytes`.
