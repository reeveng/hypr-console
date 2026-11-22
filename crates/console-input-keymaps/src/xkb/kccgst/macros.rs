#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  allow(
    explicit080_macro_budget,
    reason = "the lexer and the parser compare tokens by the punctuation itself, `token![;]`, and only a macro takes `;` as an argument; a reader holds one thing, that the token is the punctuation it names"
  )
)]
macro_rules! token {
    ($($tt:tt)*) => {
        Token::Punctuation(punctuation!($($tt)*))
    };
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  allow(
    explicit080_macro_budget,
    reason = "the parser names the punctuation it expects by writing it, `punctuation![+]`, and only a macro takes `+` as an argument; each arm is the one line that ties a character to its variant"
  )
)]
macro_rules! punctuation {
  (;) => {
    Punctuation::Semicolon
  };
  (=) => {
    Punctuation::Equals
  };
  (.) => {
    Punctuation::Dot
  };
  (,) => {
    Punctuation::Comma
  };
  (+) => {
    Punctuation::Plus
  };
  (-) => {
    Punctuation::Minus
  };
  (*) => {
    Punctuation::Times
  };
  (/) => {
    Punctuation::Divide
  };
  (!) => {
    Punctuation::Exclam
  };
  (~) => {
    Punctuation::Invert
  };
}
