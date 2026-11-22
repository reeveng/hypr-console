//! The table is the one PDFBox keeps, at <https://github.com/apache/pdfbox/blob/4438b8fdc67a3a9ebfb194595d0e81f88b708a37/pdfbox/src/main/java/org/apache/pdfbox/pdmodel/font/encoding/MacOSRomanEncoding.java.java>.

use console_core_never::Never;
use console_core_perfect_hash::SortedMap;

pub fn get(code: u8) -> Result<Option<&'static str>, Never> {
  MAC_OS_ROMAN.get(&code)
}

pub fn get_inverse(name: &str) -> Result<Option<u8>, Never> {
  MAC_OS_ROMAN_INVERSE.get(name)
}

static MAC_OS_ROMAN: SortedMap<u8, &'static str> = SortedMap(&[
  (173_u8, "notequal"),
  (176_u8, "infinity"),
  (178_u8, "lessequal"),
  (179_u8, "greaterequal"),
  (182_u8, "partialdiff"),
  (183_u8, "summation"),
  (184_u8, "product"),
  (185_u8, "pi"),
  (186_u8, "integral"),
  (189_u8, "Omega"),
  (195_u8, "radical"),
  (197_u8, "approxequal"),
  (198_u8, "Delta"),
  (215_u8, "lozenge"),
  (219_u8, "Euro"),
  (240_u8, "apple"),
]);

static MAC_OS_ROMAN_INVERSE: SortedMap<&'static str, u8> = SortedMap(&[
  ("Delta", 0o0306_u8),
  ("Euro", 0o0333_u8),
  ("Omega", 0o0275_u8),
  ("apple", 0o0360_u8),
  ("approxequal", 0o0305_u8),
  ("greaterequal", 0o0263_u8),
  ("infinity", 0o0260_u8),
  ("integral", 0o0272_u8),
  ("lessequal", 0o0262_u8),
  ("lozenge", 0o0327_u8),
  ("notequal", 0o0255_u8),
  ("partialdiff", 0o0266_u8),
  ("pi", 0o0271_u8),
  ("product", 0o0270_u8),
  ("radical", 0o0303_u8),
  ("summation", 0o0267_u8),
]);
