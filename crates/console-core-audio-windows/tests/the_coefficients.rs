//! Every window the decoders ask for, at the sizes they ask for it and a few
//! more, folded into one number each. The numbers were taken from the
//! generators before they were rewritten under the workspace's rules, so a
//! rewrite that moves one coefficient by one bit is red here.

use console_core_audio_windows::{
  Extent, Gain, KaiserAlpha, WindowType, generate_window, vorbis_half,
};
use console_core_never::Never;
use console_core_number_conversion::index;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn folded(window: &[f32]) -> Result<u64, Never> {
  Ok(window.iter().fold(FNV_OFFSET, |hash, value| {
    (hash ^ u64::from(value.to_bits())).wrapping_mul(FNV_PRIME)
  }))
}

fn rows() -> Result<Vec<(String, u64)>, Never> {
  let mut rows = Vec::new();

  for size in [
    1u32, 2, 3, 8, 12, 32, 64, 120, 128, 240, 256, 480, 960, 1024, 1920, 2048,
  ] {
    let Ok(length) = index(size);
    let mut window = vec![0.0; length.min(2048)];

    let Ok(()) = vorbis_half(&mut window);
    let Ok(hash) = folded(&window);

    rows.push((format!("vorbis {size}"), hash));
  }

  for (name, kind) in [
    ("sine", WindowType::Sine),
    ("kb4", WindowType::KaiserBessel(KaiserAlpha(4.0))),
    ("kb6", WindowType::KaiserBessel(KaiserAlpha(6.0))),
    ("kb2.5", WindowType::KaiserBessel(KaiserAlpha(2.5))),
  ] {
    for size in [1u32, 2, 8, 12, 128, 1000, 1024] {
      for (half, extent) in [("true", Extent::Half), ("false", Extent::Whole)] {
        for scale in [1.0f32, 0.5, 2.0] {
          let Ok(length) = index(size);
          let mut window = vec![42.0; length.saturating_add(3).min(1027)];

          let Ok(()) = generate_window(kind, Gain(scale), size, extent, &mut window);
          let Ok(hash) = folded(&window);

          rows.push((format!("{name} {size} half={half} scale={scale}"), hash));
        }
      }
    }
  }

  Ok(rows)
}

#[test]
fn every_window_comes_out_as_it_did_before_the_rewrite() {
  let Ok(found) = rows();
  let wrong: Vec<&str> = found
    .iter()
    .filter(|(name, digest)| {
      GOLDEN
        .iter()
        .find(|(wanted, _)| wanted == name)
        .map(|(_, expected)| expected)
        != Some(digest)
    })
    .map(|(name, _)| name.as_str())
    .collect();

  assert_eq!(wrong, Vec::<&str>::new(), "{found:#x?}");
  assert_eq!(found.len(), GOLDEN.len());
}

const GOLDEN: &[(&str, u64)] = &[
  ("vorbis 1", 0xa4686ea9e858b0a2),
  ("vorbis 2", 0x833d19a57894be7e),
  ("vorbis 3", 0x6434cff393486920),
  ("vorbis 8", 0x2e20ec0b6420a686),
  ("vorbis 12", 0xa8b19a4d9fd2843),
  ("vorbis 32", 0xb5d560413b5fa1cd),
  ("vorbis 64", 0x7afeca53f9cbd8c0),
  ("vorbis 120", 0x88db0e03caf8c048),
  ("vorbis 128", 0x4428365edb4758da),
  ("vorbis 240", 0xeea2631003c3c81b),
  ("vorbis 256", 0x7b3c024a591b462),
  ("vorbis 480", 0x71a968a6899bb7d),
  ("vorbis 960", 0xba29e61a8a585599),
  ("vorbis 1024", 0xad16b22898a635ca),
  ("vorbis 1920", 0x2e590984fa6f5cab),
  ("vorbis 2048", 0x4da07e9bc8914f7b),
  ("sine 1 half=true scale=1", 0xb921648f5784b836),
  ("sine 1 half=true scale=0.5", 0xb756a9e56304b836),
  ("sine 1 half=true scale=2", 0xbd4c7f746c04b836),
  ("sine 1 half=false scale=1", 0x9aa763c8723613f5),
  ("sine 1 half=false scale=0.5", 0x9668b54ed5b613f5),
  ("sine 1 half=false scale=2", 0xe1708c9096b613f5),
  ("sine 2 half=true scale=1", 0x8162f52f1bd278e7),
  ("sine 2 half=true scale=0.5", 0x5b40d2f873d278e7),
  ("sine 2 half=true scale=2", 0x66eaeb423dd278e7),
  ("sine 2 half=false scale=1", 0xdfd098c4aaab9de1),
  ("sine 2 half=false scale=0.5", 0xed20fd07dfab9de1),
  ("sine 2 half=false scale=2", 0xf8c8314191ab9de1),
  ("sine 8 half=true scale=1", 0xe43ee995e6d28de),
  ("sine 8 half=true scale=0.5", 0xf2cdd406d86d28de),
  ("sine 8 half=true scale=2", 0x357963ad3e6d28de),
  ("sine 8 half=false scale=1", 0x29591cac319ad352),
  ("sine 8 half=false scale=0.5", 0x940bcaa6269ad352),
  ("sine 8 half=false scale=2", 0xd1c8c3bb4c9ad352),
  ("sine 12 half=true scale=1", 0x6c01fa8ea8f41407),
  ("sine 12 half=true scale=0.5", 0x5b603137e0f41407),
  ("sine 12 half=true scale=2", 0xa199942ea8f41407),
  ("sine 12 half=false scale=1", 0x450444b0f53cfdf0),
  ("sine 12 half=false scale=0.5", 0x1e75d94ace3cfdf0),
  ("sine 12 half=false scale=2", 0x6393866923cfdf0),
  ("sine 128 half=true scale=1", 0x773329aaf69418a5),
  ("sine 128 half=true scale=0.5", 0x60b16c590f9418a5),
  ("sine 128 half=true scale=2", 0xfb4dbdba2b9418a5),
  ("sine 128 half=false scale=1", 0x220cb017184c35a3),
  ("sine 128 half=false scale=0.5", 0x7f4b155f7f4c35a3),
  ("sine 128 half=false scale=2", 0x9608b307214c35a3),
  ("sine 1000 half=true scale=1", 0x76b42852d9e1a5ac),
  ("sine 1000 half=true scale=0.5", 0x13f9b32283e1a5ac),
  ("sine 1000 half=true scale=2", 0x7e0e2b2109e1a5ac),
  ("sine 1000 half=false scale=1", 0xe351082966e27c8),
  ("sine 1000 half=false scale=0.5", 0x2c167127106e27c8),
  ("sine 1000 half=false scale=2", 0xc3b54e51346e27c8),
  ("sine 1024 half=true scale=1", 0x3103eb38f097d246),
  ("sine 1024 half=true scale=0.5", 0xec4dfd02a197d246),
  ("sine 1024 half=true scale=2", 0x63baa9e6db97d246),
  ("sine 1024 half=false scale=1", 0x3a19a9172ed1c6a1),
  ("sine 1024 half=false scale=0.5", 0xd0519a204bd1c6a1),
  ("sine 1024 half=false scale=2", 0x42eb612b89d1c6a1),
  ("kb4 1 half=true scale=1", 0xb921648f5784b836),
  ("kb4 1 half=true scale=0.5", 0xb921648f5784b836),
  ("kb4 1 half=true scale=2", 0xb921648f5784b836),
  ("kb4 1 half=false scale=1", 0xb921648f5784b836),
  ("kb4 1 half=false scale=0.5", 0xb921648f5784b836),
  ("kb4 1 half=false scale=2", 0xb921648f5784b836),
  ("kb4 2 half=true scale=1", 0xe76f971898dd13b5),
  ("kb4 2 half=true scale=0.5", 0xe76f971898dd13b5),
  ("kb4 2 half=true scale=2", 0xe76f971898dd13b5),
  ("kb4 2 half=false scale=1", 0x136bb07faa6ae145),
  ("kb4 2 half=false scale=0.5", 0x136bb07faa6ae145),
  ("kb4 2 half=false scale=2", 0x136bb07faa6ae145),
  ("kb4 8 half=true scale=1", 0x53814b4f4efa1361),
  ("kb4 8 half=true scale=0.5", 0x53814b4f4efa1361),
  ("kb4 8 half=true scale=2", 0x53814b4f4efa1361),
  ("kb4 8 half=false scale=1", 0x505e72721ca0fd0f),
  ("kb4 8 half=false scale=0.5", 0x505e72721ca0fd0f),
  ("kb4 8 half=false scale=2", 0x505e72721ca0fd0f),
  ("kb4 12 half=true scale=1", 0x3b8a3f9455248569),
  ("kb4 12 half=true scale=0.5", 0x3b8a3f9455248569),
  ("kb4 12 half=true scale=2", 0x3b8a3f9455248569),
  ("kb4 12 half=false scale=1", 0x6ab2e240c9acd448),
  ("kb4 12 half=false scale=0.5", 0x6ab2e240c9acd448),
  ("kb4 12 half=false scale=2", 0x6ab2e240c9acd448),
  ("kb4 128 half=true scale=1", 0x44c4bff113d79743),
  ("kb4 128 half=true scale=0.5", 0x44c4bff113d79743),
  ("kb4 128 half=true scale=2", 0x44c4bff113d79743),
  ("kb4 128 half=false scale=1", 0x575fa18f03a77c22),
  ("kb4 128 half=false scale=0.5", 0x575fa18f03a77c22),
  ("kb4 128 half=false scale=2", 0x575fa18f03a77c22),
  ("kb4 1000 half=true scale=1", 0xca364469fd567181),
  ("kb4 1000 half=true scale=0.5", 0xca364469fd567181),
  ("kb4 1000 half=true scale=2", 0xca364469fd567181),
  ("kb4 1000 half=false scale=1", 0xd698c09b6308c55a),
  ("kb4 1000 half=false scale=0.5", 0xd698c09b6308c55a),
  ("kb4 1000 half=false scale=2", 0xd698c09b6308c55a),
  ("kb4 1024 half=true scale=1", 0x7d8e57d16c7bc814),
  ("kb4 1024 half=true scale=0.5", 0x7d8e57d16c7bc814),
  ("kb4 1024 half=true scale=2", 0x7d8e57d16c7bc814),
  ("kb4 1024 half=false scale=1", 0x1aadd1f14495983a),
  ("kb4 1024 half=false scale=0.5", 0x1aadd1f14495983a),
  ("kb4 1024 half=false scale=2", 0x1aadd1f14495983a),
  ("kb6 1 half=true scale=1", 0xb921648f5784b836),
  ("kb6 1 half=true scale=0.5", 0xb921648f5784b836),
  ("kb6 1 half=true scale=2", 0xb921648f5784b836),
  ("kb6 1 half=false scale=1", 0xb921648f5784b836),
  ("kb6 1 half=false scale=0.5", 0xb921648f5784b836),
  ("kb6 1 half=false scale=2", 0xb921648f5784b836),
  ("kb6 2 half=true scale=1", 0xb55162e90c31227a),
  ("kb6 2 half=true scale=0.5", 0xb55162e90c31227a),
  ("kb6 2 half=true scale=2", 0xb55162e90c31227a),
  ("kb6 2 half=false scale=1", 0x4a6d028b5b63cb99),
  ("kb6 2 half=false scale=0.5", 0x4a6d028b5b63cb99),
  ("kb6 2 half=false scale=2", 0x4a6d028b5b63cb99),
  ("kb6 8 half=true scale=1", 0x1b757afb2c5a4dcd),
  ("kb6 8 half=true scale=0.5", 0x1b757afb2c5a4dcd),
  ("kb6 8 half=true scale=2", 0x1b757afb2c5a4dcd),
  ("kb6 8 half=false scale=1", 0x27cac57424cdbb74),
  ("kb6 8 half=false scale=0.5", 0x27cac57424cdbb74),
  ("kb6 8 half=false scale=2", 0x27cac57424cdbb74),
  ("kb6 12 half=true scale=1", 0xc224c078ab1c21a5),
  ("kb6 12 half=true scale=0.5", 0xc224c078ab1c21a5),
  ("kb6 12 half=true scale=2", 0xc224c078ab1c21a5),
  ("kb6 12 half=false scale=1", 0xfb36dfabdb90b774),
  ("kb6 12 half=false scale=0.5", 0xfb36dfabdb90b774),
  ("kb6 12 half=false scale=2", 0xfb36dfabdb90b774),
  ("kb6 128 half=true scale=1", 0x92ddb47a041df4c3),
  ("kb6 128 half=true scale=0.5", 0x92ddb47a041df4c3),
  ("kb6 128 half=true scale=2", 0x92ddb47a041df4c3),
  ("kb6 128 half=false scale=1", 0x5a25401c69ae54e0),
  ("kb6 128 half=false scale=0.5", 0x5a25401c69ae54e0),
  ("kb6 128 half=false scale=2", 0x5a25401c69ae54e0),
  ("kb6 1000 half=true scale=1", 0x4f55288b51d61305),
  ("kb6 1000 half=true scale=0.5", 0x4f55288b51d61305),
  ("kb6 1000 half=true scale=2", 0x4f55288b51d61305),
  ("kb6 1000 half=false scale=1", 0x8a532d5d3baff321),
  ("kb6 1000 half=false scale=0.5", 0x8a532d5d3baff321),
  ("kb6 1000 half=false scale=2", 0x8a532d5d3baff321),
  ("kb6 1024 half=true scale=1", 0xbddf23d3a30dc1e0),
  ("kb6 1024 half=true scale=0.5", 0xbddf23d3a30dc1e0),
  ("kb6 1024 half=true scale=2", 0xbddf23d3a30dc1e0),
  ("kb6 1024 half=false scale=1", 0xb59d3734a3e8d7dc),
  ("kb6 1024 half=false scale=0.5", 0xb59d3734a3e8d7dc),
  ("kb6 1024 half=false scale=2", 0xb59d3734a3e8d7dc),
  ("kb2.5 1 half=true scale=1", 0xb921648f5784b836),
  ("kb2.5 1 half=true scale=0.5", 0xb921648f5784b836),
  ("kb2.5 1 half=true scale=2", 0xb921648f5784b836),
  ("kb2.5 1 half=false scale=1", 0xb921648f5784b836),
  ("kb2.5 1 half=false scale=0.5", 0xb921648f5784b836),
  ("kb2.5 1 half=false scale=2", 0xb921648f5784b836),
  ("kb2.5 2 half=true scale=1", 0x401f1236dbed320a),
  ("kb2.5 2 half=true scale=0.5", 0x401f1236dbed320a),
  ("kb2.5 2 half=true scale=2", 0x401f1236dbed320a),
  ("kb2.5 2 half=false scale=1", 0x30f08c88669deedd),
  ("kb2.5 2 half=false scale=0.5", 0x30f08c88669deedd),
  ("kb2.5 2 half=false scale=2", 0x30f08c88669deedd),
  ("kb2.5 8 half=true scale=1", 0xb7eb18b0d408f20),
  ("kb2.5 8 half=true scale=0.5", 0xb7eb18b0d408f20),
  ("kb2.5 8 half=true scale=2", 0xb7eb18b0d408f20),
  ("kb2.5 8 half=false scale=1", 0x747cebdf0c0e7cd1),
  ("kb2.5 8 half=false scale=0.5", 0x747cebdf0c0e7cd1),
  ("kb2.5 8 half=false scale=2", 0x747cebdf0c0e7cd1),
  ("kb2.5 12 half=true scale=1", 0x7ea8337bc75d8a84),
  ("kb2.5 12 half=true scale=0.5", 0x7ea8337bc75d8a84),
  ("kb2.5 12 half=true scale=2", 0x7ea8337bc75d8a84),
  ("kb2.5 12 half=false scale=1", 0xb0d7e6cf838c250e),
  ("kb2.5 12 half=false scale=0.5", 0xb0d7e6cf838c250e),
  ("kb2.5 12 half=false scale=2", 0xb0d7e6cf838c250e),
  ("kb2.5 128 half=true scale=1", 0xe8e938e2b8f9eee8),
  ("kb2.5 128 half=true scale=0.5", 0xe8e938e2b8f9eee8),
  ("kb2.5 128 half=true scale=2", 0xe8e938e2b8f9eee8),
  ("kb2.5 128 half=false scale=1", 0x4d8159579930e962),
  ("kb2.5 128 half=false scale=0.5", 0x4d8159579930e962),
  ("kb2.5 128 half=false scale=2", 0x4d8159579930e962),
  ("kb2.5 1000 half=true scale=1", 0x757574135f7ff5a3),
  ("kb2.5 1000 half=true scale=0.5", 0x757574135f7ff5a3),
  ("kb2.5 1000 half=true scale=2", 0x757574135f7ff5a3),
  ("kb2.5 1000 half=false scale=1", 0x58781c872c7128fe),
  ("kb2.5 1000 half=false scale=0.5", 0x58781c872c7128fe),
  ("kb2.5 1000 half=false scale=2", 0x58781c872c7128fe),
  ("kb2.5 1024 half=true scale=1", 0x654377aeb2bdc0f1),
  ("kb2.5 1024 half=true scale=0.5", 0x654377aeb2bdc0f1),
  ("kb2.5 1024 half=true scale=2", 0x654377aeb2bdc0f1),
  ("kb2.5 1024 half=false scale=1", 0x99e387830bacbd11),
  ("kb2.5 1024 half=false scale=0.5", 0x99e387830bacbd11),
  ("kb2.5 1024 half=false scale=2", 0x99e387830bacbd11),
];
