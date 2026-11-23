//! The inverse transform at every size the decoders use and the ones between,
//! at the scales they use and a few more, folded into one number each. The
//! numbers were taken from the transform before it was rewritten under the
//! workspace's rules, so a rewrite that moves one output by one bit is red here.
//! A negative zero is folded as a zero: which of the two a sparse input leaves
//! behind is not a thing any decoder reads.
//!
//! Beside them the transform is held to the definition, summed directly in
//! double precision, so the digests cannot agree on something that is wrong.

use console_core_frequency_transforms::{
  TransformError,
  mdct::{Imdct, Scale},
};
use console_core_never::Never;
use console_core_number_conversion::index;
use core::f64::consts::PI;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn folded(outputs: &[f32]) -> Result<u64, Never> {
  Ok(outputs.iter().fold(FNV_OFFSET, |hash, value| {
    (hash ^ u64::from((value + 0.0).to_bits())).wrapping_mul(FNV_PRIME)
  }))
}

fn dense(seed: u64, count: u32) -> Result<Vec<f32>, Never> {
  Ok(
    (0..count)
      .scan(seed, |state, _| {
        *state = state
          .wrapping_mul(6_364_136_223_846_793_005)
          .wrapping_add(1_442_695_040_888_963_407);

        let [_, _, _, _, _, b, c, d] = state.wrapping_shr(16).to_be_bytes();

        Some(f32::from_bits(0x3f80_0000 | (u32::from_be_bytes([0, b, c, d]) & 0x007f_ffff)) - 1.5)
      })
      .collect(),
  )
}

fn sparse([count, at]: [u32; 2]) -> Result<Vec<f32>, Never> {
  Ok(
    (0..count)
      .map(|position| f32::from(u8::from(position == at)))
      .collect(),
  )
}

fn transformed(transform: &mut Imdct, spec: &[f32]) -> Result<Vec<f32>, TransformError> {
  let Ok(length) = index(spec.len());
  let mut out = vec![7.0; length.saturating_mul(2).min(131072)];

  transform.imdct(spec, &mut out)?;

  Ok(out)
}

fn rows() -> Result<Vec<(String, u64)>, TransformError> {
  let mut rows = Vec::new();

  for n in [
    4u32, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 65536,
  ] {
    let scales: &[f64] = match n {
      65536 => &[1.0],
      _ => &[1.0, 0.25, 1.0 / 2048.0, -1.0, -0.5],
    };

    for &scale in scales {
      let mut transform = Imdct::new_scaled(n, Scale(scale))?;
      let Ok(first) = dense(u64::from(n), n);
      let Ok(again) = dense(u64::from(n).saturating_add(1), n);
      let Ok(low) = sparse([n, 0]);
      let Ok(second) = sparse([n, 1]);
      let Ok(middle) = sparse([n, n.wrapping_shr(1)]);
      let Ok(last) = sparse([n, n.saturating_sub(1)]);

      for (name, spec) in [
        ("dense", first),
        ("again", again),
        ("first", low),
        ("second", second),
        ("middle", middle),
        ("last", last),
      ] {
        let out = transformed(&mut transform, &spec)?;
        let Ok(hash) = folded(&out);

        rows.push((format!("{n} {scale} {name}"), hash));
      }
    }
  }

  let mut plain = Imdct::new(64)?;
  let Ok(spec) = dense(5, 64);
  let out = transformed(&mut plain, &spec)?;
  let Ok(hash) = folded(&out);

  rows.push(("plain 64".to_string(), hash));

  Ok(rows)
}

#[test]
fn every_size_transforms_as_it_did_before_the_rewrite() -> Result<(), TransformError> {
  let found = rows()?;
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

  Ok(())
}

fn analytic(spec: &[f32], n: u32, scale: Scale) -> Result<Vec<f64>, Never> {
  let count = f64::from(n);
  let step = PI / (2.0 * (2.0 * count));

  Ok(
    (0..n.saturating_mul(2))
      .map(|i| {
        let accumulated: f64 = spec
          .iter()
          .zip(0u32..)
          .map(|(value, j)| {
            f64::from(*value)
              * (step * ((2.0 * f64::from(i) + 1.0 + count) * (2.0 * f64::from(j) + 1.0))).cos()
          })
          .sum();

        scale.0 * accumulated
      })
      .collect(),
  )
}

#[test]
fn every_size_agrees_with_the_definition() -> Result<(), TransformError> {
  for n in [4u32, 8, 16, 32, 64, 256, 1024] {
    for scale in [
      1.0,
      2.0 / f64::from(n.saturating_mul(2)).sqrt().powi(2),
      (2.0 / f64::from(n.saturating_mul(2))).sqrt(),
    ] {
      let Ok(spec) = dense(u64::from(n), n);
      let mut transform = Imdct::new_scaled(n, Scale(scale))?;
      let actual = transformed(&mut transform, &spec)?;
      let Ok(expected) = analytic(&spec, n, Scale(scale));

      for (found, wanted) in actual.iter().zip(expected) {
        assert!(
          (f64::from(*found) - wanted).abs() < 1e-4 * scale.max(1.0),
          "n={n} scale={scale}: {found} against {wanted}"
        );
      }
    }
  }

  Ok(())
}

#[test]
fn a_size_that_is_not_a_power_of_two_is_refused() {
  for n in [0u32, 1, 2, 3, 6, 12, 100, 131072, 262144] {
    assert!(matches!(Imdct::new(n), Err(TransformError::Size)), "{n}");
  }
}

#[test]
fn a_slice_of_the_wrong_length_is_refused() -> Result<(), TransformError> {
  let mut transform = Imdct::new(8)?;

  assert_eq!(
    transform.imdct(&[0.0; 7], &mut [0.0; 16]),
    Err(TransformError::Spectrum)
  );
  assert_eq!(
    transform.imdct(&[0.0; 8], &mut [0.0; 15]),
    Err(TransformError::Output)
  );
  assert_eq!(transform.imdct(&[0.0; 8], &mut [0.0; 16]), Ok(()));

  Ok(())
}

const GOLDEN: &[(&str, u64)] = &[
  ("4 1 dense", 0xd9fdab26e55e0951),
  ("4 1 again", 0x247d8d05d4144c21),
  ("4 1 first", 0x6e50fb5656dc34d1),
  ("4 1 second", 0x7682a6a48b2b5a79),
  ("4 1 middle", 0x568b6f10ec35d171),
  ("4 1 last", 0x2d398c287e733669),
  ("4 0.25 dense", 0x151a5914eb5e0951),
  ("4 0.25 again", 0xfd28a57046144c21),
  ("4 0.25 first", 0xd12feb33f6dc34d1),
  ("4 0.25 second", 0xe6e16908772b5a79),
  ("4 0.25 middle", 0x674bf7b78c35d171),
  ("4 0.25 last", 0x6d12fdc51e733669),
  ("4 0.00048828125 dense", 0x8bd22f1b63bd436b),
  ("4 0.00048828125 again", 0x16764f2bff59c701),
  ("4 0.00048828125 first", 0x5736472c6fdc34d1),
  ("4 0.00048828125 second", 0xb3a140721e2b5a79),
  ("4 0.00048828125 middle", 0x1486d7362535d171),
  ("4 0.00048828125 last", 0x5839746174733669),
  ("4 -1 dense", 0x179154b5e55e0951),
  ("4 -1 again", 0xd71298f7d4144c21),
  ("4 -1 first", 0xdcc167a756dc34d1),
  ("4 -1 second", 0xacf762408b2b5a79),
  ("4 -1 middle", 0x71ed6978ec35d171),
  ("4 -1 last", 0x6fb536157e733669),
  ("4 -0.5 dense", 0x4eb1e0cec7bd436b),
  ("4 -0.5 again", 0xa374b239e159c701),
  ("4 -0.5 first", 0x776fd46c7ddc34d1),
  ("4 -0.5 second", 0x6c7b56261c2b5a79),
  ("4 -0.5 middle", 0xa25db531c335d171),
  ("4 -0.5 last", 0xc44842fcac733669),
  ("8 1 dense", 0xca17773c9f549a11),
  ("8 1 again", 0x23670a62e984c14d),
  ("8 1 first", 0x6b4c44d3f670375d),
  ("8 1 second", 0xd70904f3a4cfa9e7),
  ("8 1 middle", 0x266ae26a0d6a458d),
  ("8 1 last", 0x71e868ceeda0918d),
  ("8 0.25 dense", 0xb18ab33f29549a11),
  ("8 0.25 again", 0x32e85f419784c14d),
  ("8 0.25 first", 0xfd3987080c70375d),
  ("8 0.25 second", 0xa480b466eccfa9e7),
  ("8 0.25 middle", 0xc1c1d7718d6a458d),
  ("8 0.25 last", 0xf504a23c77a0918d),
  ("8 0.00048828125 dense", 0x895f940ec03b769f),
  ("8 0.00048828125 again", 0x6b945165dd2b4693),
  ("8 0.00048828125 first", 0x4b949da74be7a2d1),
  ("8 0.00048828125 second", 0xfa69ed62b06fb607),
  ("8 0.00048828125 middle", 0x38a1d800f0eeb30b),
  ("8 0.00048828125 last", 0xdfc097d7ebf5aef1),
  ("8 -1 dense", 0xeb3be2d79f549a11),
  ("8 -1 again", 0x4668a436e984c14d),
  ("8 -1 first", 0x75882030f670375d),
  ("8 -1 second", 0x22155be5a4cfa9e7),
  ("8 -1 middle", 0xa2e4795d0d6a458d),
  ("8 -1 last", 0xe15c8f89eda0918d),
  ("8 -0.5 dense", 0x2f5ed6ba383b769f),
  ("8 -0.5 again", 0xb0c21dd02f2b4693),
  ("8 -0.5 first", 0x26fe1abcade7a2d1),
  ("8 -0.5 second", 0xe7804d831e6fb607),
  ("8 -0.5 middle", 0x4c947f642eeeb30b),
  ("8 -0.5 last", 0xea235bc49bf5aef1),
  ("16 1 dense", 0x437ecfeb972b9eb),
  ("16 1 again", 0x2057122c738a1d33),
  ("16 1 first", 0x20c7c0e8f5750b3f),
  ("16 1 second", 0x801ea4c12a41c391),
  ("16 1 middle", 0x3fa3bb622aebcfd3),
  ("16 1 last", 0xf0eced33d738a19f),
  ("16 0.25 dense", 0x48c37d386372b9eb),
  ("16 0.25 again", 0x283ccba9998a1d33),
  ("16 0.25 first", 0x29a83e4d4d750b3f),
  ("16 0.25 second", 0x2218ca6c9e41c391),
  ("16 0.25 middle", 0x30a4e6d1d4ebcfd3),
  ("16 0.25 last", 0x242b5a9cfb38a19f),
  ("16 0.00048828125 dense", 0x873442c2c8747db),
  ("16 0.00048828125 again", 0x3446fb921a783d43),
  ("16 0.00048828125 first", 0xc2995da034bde1a3),
  ("16 0.00048828125 second", 0x84e32c27e191b1bb),
  ("16 0.00048828125 middle", 0x9278a9c00739bd07),
  ("16 0.00048828125 last", 0x29c31b4f236d5173),
  ("16 -1 dense", 0x2f7d5029b972b9eb),
  ("16 -1 again", 0xcb9f681738a1d33),
  ("16 -1 first", 0xb2b2bc5ff5750b3f),
  ("16 -1 second", 0xc9241f412a41c391),
  ("16 -1 middle", 0xe2aa2fd82aebcfd3),
  ("16 -1 last", 0x3e1137dbd738a19f),
  ("16 -0.5 dense", 0x93088d60188747db),
  ("16 -0.5 again", 0x5c6abf3600783d43),
  ("16 -0.5 first", 0x9d934537b4bde1a3),
  ("16 -0.5 second", 0x6602925ff591b1bb),
  ("16 -0.5 middle", 0xb3db6ec78139bd07),
  ("16 -0.5 last", 0x4e0ff383856d5173),
  ("32 1 dense", 0x42c0c92699e5f56b),
  ("32 1 again", 0xbfd311f922ea0ca9),
  ("32 1 first", 0x496fa3775ea171f),
  ("32 1 second", 0x28a6aa3cfd32ca83),
  ("32 1 middle", 0x6524082cbacf94f5),
  ("32 1 last", 0x97fbdc807cdc3f6f),
  ("32 0.25 dense", 0x18adfd3305e5f56b),
  ("32 0.25 again", 0x93957fb440ea0ca9),
  ("32 0.25 first", 0xb5b40b498bea171f),
  ("32 0.25 second", 0x24bc5ed43132ca83),
  ("32 0.25 middle", 0x84cfcbcbc2cf94f5),
  ("32 0.25 last", 0xedbd024bf6dc3f6f),
  ("32 0.00048828125 dense", 0x73227dc3943d1fe1),
  ("32 0.00048828125 again", 0xcd0d9fd12845192b),
  ("32 0.00048828125 first", 0x160b712a1bc4a52b),
  ("32 0.00048828125 second", 0x2523d29a6b1466af),
  ("32 0.00048828125 middle", 0x5eb31887bb717cbd),
  ("32 0.00048828125 last", 0x60a6550ed03f4fcb),
  ("32 -1 dense", 0xc67d02899e5f56b),
  ("32 -1 again", 0x9df4129c22ea0ca9),
  ("32 -1 first", 0xe06ea2a375ea171f),
  ("32 -1 second", 0x5f935f4afd32ca83),
  ("32 -1 middle", 0x9ec9f5e3bacf94f5),
  ("32 -1 last", 0x2947a57cdc3f6f),
  ("32 -0.5 dense", 0xa21b2efd7e3d1fe1),
  ("32 -0.5 again", 0x20258209c045192b),
  ("32 -0.5 first", 0xf9eaab3e0dc4a52b),
  ("32 -0.5 second", 0xc8cc088db71466af),
  ("32 -0.5 middle", 0x6db5a15751717cbd),
  ("32 -0.5 last", 0x1838d601663f4fcb),
  ("64 1 dense", 0xd1377d5e1db5e253),
  ("64 1 again", 0x24f211bb1c7fe18f),
  ("64 1 first", 0x373c58caa535e625),
  ("64 1 second", 0x7bffd631304ad707),
  ("64 1 middle", 0xacb7b67370454c13),
  ("64 1 last", 0x7c990de31f75f225),
  ("64 0.25 dense", 0xc30433e0cbb5e253),
  ("64 0.25 again", 0x37469305b67fe18f),
  ("64 0.25 first", 0x9e6b11617735e625),
  ("64 0.25 second", 0xaceaa685ce4ad707),
  ("64 0.25 middle", 0x8a7d187706454c13),
  ("64 0.25 last", 0x16aa11299b75f225),
  ("64 0.00048828125 dense", 0x5909fa5226907e75),
  ("64 0.00048828125 again", 0x56ffdd5e72b6dd63),
  ("64 0.00048828125 first", 0x300315548abe7215),
  ("64 0.00048828125 second", 0x4cddbeb6e1e35801),
  ("64 0.00048828125 middle", 0x863cd41634794af7),
  ("64 0.00048828125 last", 0xff12884904e40a95),
  ("64 -1 dense", 0xc73639ad1db5e253),
  ("64 -1 again", 0x961424341c7fe18f),
  ("64 -1 first", 0xb1cc8b86a535e625),
  ("64 -1 second", 0x271d357b304ad707),
  ("64 -1 middle", 0xdf5179a270454c13),
  ("64 -1 last", 0x96cce8bd1f75f225),
  ("64 -0.5 dense", 0x8ee802e7c907e75),
  ("64 -0.5 again", 0xd41edb1340b6dd63),
  ("64 -0.5 first", 0xe967fdb574be7215),
  ("64 -0.5 second", 0xd1cd408681e35801),
  ("64 -0.5 middle", 0xdf3087acca794af7),
  ("64 -0.5 last", 0x2ef724fbbae40a95),
  ("128 1 dense", 0xe203e1fe3361fc8d),
  ("128 1 again", 0xd7003bd63c6d6ca7),
  ("128 1 first", 0x6247cf24f6f0a5d1),
  ("128 1 second", 0x81625391b722cf03),
  ("128 1 middle", 0x1907c8709fc9ce75),
  ("128 1 last", 0x8bae8f307e000b81),
  ("128 0.25 dense", 0xda28badbf961fc8d),
  ("128 0.25 again", 0x9a51f8d3ba6d6ca7),
  ("128 0.25 first", 0x2f1c998cbcf0a5d1),
  ("128 0.25 second", 0x7c394219c522cf03),
  ("128 0.25 middle", 0xcf5bdb28d7c9ce75),
  ("128 0.25 last", 0x63ebf2451a000b81),
  ("128 0.00048828125 dense", 0x8bdf6ed7d12fa49),
  ("128 0.00048828125 again", 0x45c0232ce92f851b),
  ("128 0.00048828125 first", 0x4bf83fe0ae8b3197),
  ("128 0.00048828125 second", 0xb35307f75478274d),
  ("128 0.00048828125 middle", 0x8a0d6f980c16112f),
  ("128 0.00048828125 last", 0x7f185373748c73a7),
  ("128 -1 dense", 0xa3faab533361fc8d),
  ("128 -1 again", 0xd50c92533c6d6ca7),
  ("128 -1 first", 0xf72e3daaf6f0a5d1),
  ("128 -1 second", 0x85375ac3b722cf03),
  ("128 -1 middle", 0x165b13699fc9ce75),
  ("128 -1 last", 0xd946011f7e000b81),
  ("128 -0.5 dense", 0xd1caefb9a112fa49),
  ("128 -0.5 again", 0xcb277315a72f851b),
  ("128 -0.5 first", 0x6e330df1968b3197),
  ("128 -0.5 second", 0xf9838d807c78274d),
  ("128 -0.5 middle", 0x36342e6e3e16112f),
  ("128 -0.5 last", 0xeaaeba84468c73a7),
  ("256 1 dense", 0x84fbef4d400e4cbf),
  ("256 1 again", 0x360b776e6d40ff6d),
  ("256 1 first", 0xc8bc3383687b0a1f),
  ("256 1 second", 0x3f2d1528eff689f7),
  ("256 1 middle", 0x738a27894daa249d),
  ("256 1 last", 0xcf541676f5af3a6f),
  ("256 0.25 dense", 0x97c09f1a3a0e4cbf),
  ("256 0.25 again", 0xe818cfe10740ff6d),
  ("256 0.25 first", 0x4843d3a7e67b0a1f),
  ("256 0.25 second", 0x734439357ff689f7),
  ("256 0.25 middle", 0x80271822bfaa249d),
  ("256 0.25 last", 0xbacad546daf3a6f),
  ("256 0.00048828125 dense", 0x44466020161eee59),
  ("256 0.00048828125 again", 0x232c66164796296f),
  ("256 0.00048828125 first", 0x7cf12f225771e121),
  ("256 0.00048828125 second", 0x7cb64623c66e6adb),
  ("256 0.00048828125 middle", 0x7bbe84522850b8c7),
  ("256 0.00048828125 last", 0x7827128bc7491d01),
  ("256 -1 dense", 0x8bf08991400e4cbf),
  ("256 -1 again", 0x8b9b04d56d40ff6d),
  ("256 -1 first", 0x9fbd4e64687b0a1f),
  ("256 -1 second", 0xc7fd0054eff689f7),
  ("256 -1 middle", 0x4cf0af1a4daa249d),
  ("256 -1 last", 0x41efbebdf5af3a6f),
  ("256 -0.5 dense", 0x9e07e783d01eee59),
  ("256 -0.5 again", 0xe86065b34796296f),
  ("256 -0.5 first", 0xa94d62d8d971e121),
  ("256 -0.5 second", 0xb8ba53d8b86e6adb),
  ("256 -0.5 middle", 0xfae274ddd650b8c7),
  ("256 -0.5 last", 0xd4bd5fb51491d01),
  ("512 1 dense", 0x3d8839edadcb3fdb),
  ("512 1 again", 0xb60d8cae79d94a31),
  ("512 1 first", 0x7bf17003deb7927),
  ("512 1 second", 0x577711b2637304ef),
  ("512 1 middle", 0xffb4bef746631923),
  ("512 1 last", 0x45089b4f34c1a97),
  ("512 0.25 dense", 0x2cb116a1bdcb3fdb),
  ("512 0.25 again", 0x14f5d23c57d94a31),
  ("512 0.25 first", 0x27c35e3817eb7927),
  ("512 0.25 second", 0xe793c7d2f7304ef),
  ("512 0.25 middle", 0x7455d7249a631923),
  ("512 0.25 last", 0x39a05b47894c1a97),
  ("512 0.00048828125 dense", 0x9a929bd70ca027e1),
  ("512 0.00048828125 again", 0x39db3cdd256fb713),
  ("512 0.00048828125 first", 0xfc991cf8224743a5),
  ("512 0.00048828125 second", 0xf5f3935c42a606eb),
  ("512 0.00048828125 middle", 0x6c2c72e3857a70dd),
  ("512 0.00048828125 last", 0x28995822ed9a8af5),
  ("512 -1 dense", 0x6ef65868adcb3fdb),
  ("512 -1 again", 0xc369dffc79d94a31),
  ("512 -1 first", 0xfa8e58e43deb7927),
  ("512 -1 second", 0x572b3ddd637304ef),
  ("512 -1 middle", 0x8761f98d46631923),
  ("512 -1 last", 0xfb2f133bf34c1a97),
  ("512 -0.5 dense", 0x18839cad84a027e1),
  ("512 -0.5 again", 0x6360b82af96fb713),
  ("512 -0.5 first", 0x1f8a9200584743a5),
  ("512 -0.5 second", 0xf3266d90f2a606eb),
  ("512 -0.5 middle", 0x9af662e7457a70dd),
  ("512 -0.5 last", 0x10e5e44c6d9a8af5),
  ("1024 1 dense", 0x9e7a73b68ea50293),
  ("1024 1 again", 0x5a6aa08bd9088723),
  ("1024 1 first", 0xeae45ed28f398895),
  ("1024 1 second", 0x6ddfa0779e1a6e77),
  ("1024 1 middle", 0xd1c190a8e7226cb7),
  ("1024 1 last", 0xca39dc72ad209265),
  ("1024 0.25 dense", 0xdf64d9266aa50293),
  ("1024 0.25 again", 0xa15c394e0f088723),
  ("1024 0.25 first", 0x571630378d398895),
  ("1024 0.25 second", 0xb045dc09f61a6e77),
  ("1024 0.25 middle", 0x472485550b226cb7),
  ("1024 0.25 last", 0xd1e918d7d5209265),
  ("1024 0.00048828125 dense", 0x111864dafd23e38d),
  ("1024 0.00048828125 again", 0x30dc2cefdc033389),
  ("1024 0.00048828125 first", 0x6ec2d3ba7d1ab81),
  ("1024 0.00048828125 second", 0x93cc089747843e63),
  ("1024 0.00048828125 middle", 0x134c106cdad3b741),
  ("1024 0.00048828125 last", 0xb769c67bf14de721),
  ("1024 -1 dense", 0x6e1217ed8ea50293),
  ("1024 -1 again", 0x4fcf921cd9088723),
  ("1024 -1 first", 0xd4250fbe8f398895),
  ("1024 -1 second", 0x76593ced9e1a6e77),
  ("1024 -1 middle", 0x1cad2bc9e7226cb7),
  ("1024 -1 last", 0xde707190ad209265),
  ("1024 -0.5 dense", 0x37b2ceb50523e38d),
  ("1024 -0.5 again", 0xd781bc71c2033389),
  ("1024 -0.5 first", 0x72d0fe33e9d1ab81),
  ("1024 -0.5 second", 0xefbf6637fd843e63),
  ("1024 -0.5 middle", 0xd44a904e08d3b741),
  ("1024 -0.5 last", 0x262be1284b4de721),
  ("2048 1 dense", 0x6429658b4f8d206f),
  ("2048 1 again", 0xf98b6aff69d9581),
  ("2048 1 first", 0xcc9f467935d6fda1),
  ("2048 1 second", 0x81b98b9706707c65),
  ("2048 1 middle", 0xfb494dd0970b8f0b),
  ("2048 1 last", 0x31126fb01b1471e1),
  ("2048 0.25 dense", 0x13636c150b8d206f),
  ("2048 0.25 again", 0xc6612cdee89d9581),
  ("2048 0.25 first", 0x3e8c5ec277d6fda1),
  ("2048 0.25 second", 0x694f8b9dbc707c65),
  ("2048 0.25 middle", 0xa60d31cbef0b8f0b),
  ("2048 0.25 last", 0x8b7ddbd0eb1471e1),
  ("2048 0.00048828125 dense", 0x4158a5d90dcd5281),
  ("2048 0.00048828125 again", 0xaa5371058a7933a3),
  ("2048 0.00048828125 first", 0x47fb78a8bfd60447),
  ("2048 0.00048828125 second", 0xe2ec1f43f699bce3),
  ("2048 0.00048828125 middle", 0x8282fb11bb3a17a3),
  ("2048 0.00048828125 last", 0xd33a820f9ec14c87),
  ("2048 -1 dense", 0x874f911b4f8d206f),
  ("2048 -1 again", 0x671ab29bf69d9581),
  ("2048 -1 first", 0x43fe217f35d6fda1),
  ("2048 -1 second", 0x440bf01d06707c65),
  ("2048 -1 middle", 0x528eb9d0970b8f0b),
  ("2048 -1 last", 0xa1bc10b91b1471e1),
  ("2048 -0.5 dense", 0x9ca54f8b17cd5281),
  ("2048 -0.5 again", 0x2777c547c7933a3),
  ("2048 -0.5 first", 0x52d496c43bd60447),
  ("2048 -0.5 second", 0x689d6a691699bce3),
  ("2048 -0.5 middle", 0xb20b8a53c33a17a3),
  ("2048 -0.5 last", 0x9585bd9d96c14c87),
  ("4096 1 dense", 0x89add8b50296803f),
  ("4096 1 again", 0x2019111c2c730fdb),
  ("4096 1 first", 0x64805747696d5655),
  ("4096 1 second", 0x1bde7dc771425527),
  ("4096 1 middle", 0xbade0f25f51903b),
  ("4096 1 last", 0x496e35c60029cc25),
  ("4096 0.25 dense", 0x852950eda296803f),
  ("4096 0.25 again", 0xcb298a6712730fdb),
  ("4096 0.25 first", 0x89c3347e5b6d5655),
  ("4096 0.25 second", 0x19c2dd848b425527),
  ("4096 0.25 middle", 0x7bb6a98f7551903b),
  ("4096 0.25 last", 0xf7556726d429cc25),
  ("4096 0.00048828125 dense", 0xc413f7a9d7cd4fe9),
  ("4096 0.00048828125 again", 0x50147cb6a3d47311),
  ("4096 0.00048828125 first", 0x86421d2026d75d73),
  ("4096 0.00048828125 second", 0x524562c47616c4d5),
  ("4096 0.00048828125 middle", 0x669fa49c120c5c7f),
  ("4096 0.00048828125 last", 0x46aa85d918c232b3),
  ("4096 -1 dense", 0xca7e7c850296803f),
  ("4096 -1 again", 0x95bc06eb2c730fdb),
  ("4096 -1 first", 0x235e3e45696d5655),
  ("4096 -1 second", 0x5c24204171425527),
  ("4096 -1 middle", 0xf64509945f51903b),
  ("4096 -1 last", 0x8375d9d40029cc25),
  ("4096 -0.5 dense", 0xfc40100b9cd4fe9),
  ("4096 -0.5 again", 0xac89ecd2ffd47311),
  ("4096 -0.5 first", 0xdf09411abcd75d73),
  ("4096 -0.5 second", 0x4deddfe17e16c4d5),
  ("4096 -0.5 middle", 0xc7c22717d40c5c7f),
  ("4096 -0.5 last", 0xb3bda49ed8c232b3),
  ("8192 1 dense", 0xc64496a2d61021b7),
  ("8192 1 again", 0x6902ca71126ec09d),
  ("8192 1 first", 0xc547499a6e1f139),
  ("8192 1 second", 0x6a3bb5ae1f2b75af),
  ("8192 1 middle", 0x6711555c9a2a4cc1),
  ("8192 1 last", 0xd48b8ed68cd15ce9),
  ("8192 0.25 dense", 0x89f35ff7a01021b7),
  ("8192 0.25 again", 0x5db42c17c46ec09d),
  ("8192 0.25 first", 0xbd86c58636e1f139),
  ("8192 0.25 second", 0x421f91fe992b75af),
  ("8192 0.25 middle", 0x7d5624134c2a4cc1),
  ("8192 0.25 last", 0x314c3e9264d15ce9),
  ("8192 0.00048828125 dense", 0x60d07e6eb8601619),
  ("8192 0.00048828125 again", 0xb0cb127bcd7c74f9),
  ("8192 0.00048828125 first", 0xa0129bbf7ac3ab4b),
  ("8192 0.00048828125 second", 0xc0fe6031f857b81d),
  ("8192 0.00048828125 middle", 0x6ba191f54765d1f1),
  ("8192 0.00048828125 last", 0x1ebf75264d9613ab),
  ("8192 -1 dense", 0x7681c4bbd61021b7),
  ("8192 -1 again", 0x2752311a126ec09d),
  ("8192 -1 first", 0xf17baf0ca6e1f139),
  ("8192 -1 second", 0x7c55a73c1f2b75af),
  ("8192 -1 middle", 0xe6a4c7129a2a4cc1),
  ("8192 -1 last", 0x1a70e9b68cd15ce9),
  ("8192 -0.5 dense", 0xad076fd50601619),
  ("8192 -0.5 again", 0x115bb784217c74f9),
  ("8192 -0.5 first", 0xe4f022c618c3ab4b),
  ("8192 -0.5 second", 0xd31b05c24257b81d),
  ("8192 -0.5 middle", 0x1fa1a5939b65d1f1),
  ("8192 -0.5 last", 0x396fd6f9d79613ab),
  ("65536 1 dense", 0x27bb703d4e748cc9),
  ("65536 1 again", 0xfa3c239107f9d14f),
  ("65536 1 first", 0xf949d1bf3ec7888b),
  ("65536 1 second", 0xedb48dc245923f93),
  ("65536 1 middle", 0xec537ed7f2cb6cdb),
  ("65536 1 last", 0x7e9554b2ec8b3abb),
  ("plain 64", 0x71b0291181d6ae2f),
];
