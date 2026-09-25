//! Books, in a library and on a page.
//!
//! This desktop had nothing to read a book with. Readest was the model: a
//! library of covers with how far through each one is written on it, a search
//! over the titles, and a book that opens where it was put down. Readest is a
//! web page in a Tauri shell, and the page is foliate-js laying out EPUB with
//! the browser's own engine -- a JavaScript runtime and a web view carried to
//! draw paragraphs. What is here instead is the same library drawn out of the
//! shapes every surface on this machine is drawn out of, and a book read as
//! what it is: a zip of pages, read by `console-core-zip-files`, cut into
//! paragraphs by [`flow`] and into pages by [`pages`], with Pango breaking the
//! lines.
//!
//! Three kinds of book open. An EPUB is text and is laid out here, at a size
//! meant for arm's length. A PDF is already laid out, so poppler draws each page
//! and the page is shown whole. A comic -- a CBZ, which is what manga arrives as
//! -- is a zip of pictures shown one to a screen.
//!
//! What is worked out and what is drawn are kept apart the way the viewer
//! keeps them: every module here is arithmetic over text and numbers, tested on
//! a laptop with no screen, and the `books` program is the one that opens
//! files, runs poppler and draws.

pub mod appearance;
pub mod publication;
pub mod flow;
pub mod library;
pub mod markup;
pub mod open;
pub mod pages;
pub mod progress;
pub mod reading;
pub mod grid;
