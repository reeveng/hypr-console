use console_core_character_maps::{BfString, CMap, CMapName, load_embedded};
use console_core_never::Never;
use std::fmt;

#[derive(Debug)]
enum Error {
  NotEmbedded(String),
  NotParsed(String),
  Drifted(String),
}

impl fmt::Display for Error {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Error::NotEmbedded(name) => write!(to, "{name} is not in the bundle"),
      Error::NotParsed(name) => write!(to, "{name} does not parse"),
      Error::Drifted(found) => write!(to, "the maps answer differently: {found}"),
    }
  }
}

impl std::error::Error for Error {}

const STRIDE: u32 = 0x0101_0101 / 3;

const ANSWERS: [(&str, u64); 65] = [
  ("83pv-RKSJ-H", 0x6025_f948_7dd0_58f0),
  ("90ms-RKSJ-H", 0xf712_4131_8e5c_fe5c),
  ("90ms-RKSJ-V", 0x73d2_5af1_49bf_710f),
  ("90msp-RKSJ-H", 0x1c73_7c09_a36d_01c6),
  ("90msp-RKSJ-V", 0x8aaa_8346_7487_5b35),
  ("90pv-RKSJ-H", 0xc8d4_cb31_5c92_f28d),
  ("Add-RKSJ-H", 0x04cd_1f8d_8db5_64e3),
  ("Add-RKSJ-V", 0x7554_60e7_3b01_fb88),
  ("Adobe-CNS1-UCS2", 0x7ed0_656a_e58e_d3c4),
  ("Adobe-GB1-UCS2", 0x9694_7161_0aeb_c82c),
  ("Adobe-Japan1-UCS2", 0xe054_ac6a_7a5c_6338),
  ("Adobe-Korea1-UCS2", 0xd209_8f19_35b2_fbd7),
  ("B5pc-H", 0x2ef0_d25e_4e51_3a41),
  ("B5pc-V", 0x4c5d_ffc3_0373_0a1d),
  ("CNS-EUC-H", 0x91ee_6af4_fe5f_ba8f),
  ("CNS-EUC-V", 0xc3d9_7281_31a2_8e5d),
  ("ETen-B5-H", 0x6c5d_9705_8777_7d0d),
  ("ETen-B5-V", 0x7ca7_8e9d_93d8_dd81),
  ("ETenms-B5-H", 0xd6f9_6319_ba42_518c),
  ("ETenms-B5-V", 0x3858_16dc_f697_e01b),
  ("EUC-H", 0xc2ad_049f_934a_5ba0),
  ("EUC-V", 0x7417_5c0c_4e4e_caa7),
  ("Ext-RKSJ-H", 0xae21_aa1a_cdd8_346f),
  ("Ext-RKSJ-V", 0x4725_1d53_2e52_81a8),
  ("GB-EUC-H", 0x9c80_f72c_9fff_1a80),
  ("GB-EUC-V", 0x984b_fc64_c4f9_2045),
  ("GBK-EUC-H", 0xa628_fd31_6fff_14c3),
  ("GBK-EUC-V", 0x3cb2_ef5b_805f_b9ca),
  ("GBK2K-H", 0x2e1b_dc7e_569b_46ef),
  ("GBK2K-V", 0xfbe4_27b6_31bb_585d),
  ("GBKp-EUC-H", 0x12c2_26e6_ae27_1169),
  ("GBKp-EUC-V", 0x1d02_21af_617f_af94),
  ("GBpc-EUC-H", 0x728a_f937_8c4a_37f6),
  ("GBpc-EUC-V", 0x050e_bee1_1942_c79b),
  ("H", 0x82dc_b907_1fbb_de2c),
  ("HKscs-B5-H", 0xc1f3_e1a7_cba5_07f0),
  ("HKscs-B5-V", 0x1479_dc45_9ffb_5f5c),
  ("Identity-H", 0xc706_e07a_840d_8f78),
  ("Identity-V", 0xfae2_f6df_0a2f_6e7d),
  ("KSC-EUC-H", 0x792a_74d9_1a03_5566),
  ("KSC-EUC-V", 0xe391_3a86_6de8_1c18),
  ("KSCms-UHC-H", 0x094e_905b_bf3a_b0de),
  ("KSCms-UHC-HW-H", 0xdeb0_8fc4_5aaa_b3cc),
  ("KSCms-UHC-HW-V", 0x5a65_2074_64a2_72a2),
  ("KSCms-UHC-V", 0xfc8e_1e74_694b_7514),
  ("KSCpc-EUC-H", 0xea9d_74a3_99a8_3f97),
  ("UniCNS-UCS2-H", 0xdd3d_2987_07a5_575d),
  ("UniCNS-UCS2-V", 0x6478_d48f_db88_82e1),
  ("UniCNS-UTF16-H", 0x459f_d0f0_31b2_2587),
  ("UniCNS-UTF16-V", 0xd2fb_4ab6_0aa3_fe73),
  ("UniGB-UCS2-H", 0x5ed5_1cb9_4f35_18e8),
  ("UniGB-UCS2-V", 0x4d4a_e594_693d_b5f4),
  ("UniGB-UTF16-H", 0x19a8_0cc2_3a8d_ec72),
  ("UniGB-UTF16-V", 0x1605_2e6c_1adc_1b01),
  ("UniJIS-UCS2-H", 0xb0b1_38a2_1bb8_b5c2),
  ("UniJIS-UCS2-HW-H", 0xed2b_94fa_a55f_f03a),
  ("UniJIS-UCS2-HW-V", 0x71b3_2e4c_4ed7_2ad2),
  ("UniJIS-UCS2-V", 0x9a9c_eab2_a25d_d07a),
  ("UniJIS-UTF16-H", 0x2d35_e823_7fa6_e735),
  ("UniJIS-UTF16-V", 0x054b_5559_c5c3_d1c2),
  ("UniKS-UCS2-H", 0x8eee_dab4_cd70_17e9),
  ("UniKS-UCS2-V", 0x8dc6_d86b_35a2_ef83),
  ("UniKS-UTF16-H", 0xce4b_8ca7_399a_33e7),
  ("UniKS-UTF16-V", 0xacda_bc2c_4ae5_ab5d),
  ("V", 0x8824_386e_544c_8407),
];

struct Fingerprint(u64);

impl Fingerprint {
  fn new() -> Result<Fingerprint, Never> {
    Ok(Fingerprint(console_core_checksums::fnv1a::OFFSET64))
  }

  fn mix(&mut self, value: u64) -> Result<(), Never> {
    let Ok(console_core_checksums::fnv1a::State64(next)) =
      console_core_checksums::fnv1a::State64(self.0).mixed(value);

    self.0 = next;

    Ok(())
  }

  fn mix_cid(&mut self, found: Option<u32>) -> Result<(), Never> {
    match found {
      Some(cid) => self.mix(u64::from(cid)),
      None => self.mix(u64::MAX),
    }
  }
}

fn fingerprint(name: &str) -> Result<u64, Error> {
  let shown = String::from(name);
  let Ok(wanted) = CMapName::from_bytes(name.as_bytes());
  let Ok(found) = load_embedded(wanted);
  let data = found.ok_or_else(|| Error::NotEmbedded(shown.clone()))?;
  let cmap = CMap::parse(data, |base| match load_embedded(base) {
    Ok(embedded) => embedded,
  })
  .map_err(|_unreadable| Error::NotParsed(shown))?;
  let Ok(mut held) = Fingerprint::new();
  let metadata = &cmap.metadata;

  for byte in format!(
    "{:?} {:?}",
    metadata.writing_mode, metadata.character_collection
  )
  .bytes()
  {
    let Ok(()) = held.mix(u64::from(byte));
  }

  for length in 1u8..=4 {
    for code in 0u32..=0xFFFF {
      let Ok(cid) = cmap.lookup_cid_code(code, length);

      let Ok(()) = held.mix_cid(cid);
    }

    for step in 0u32..0x4000 {
      let Ok(surrogates) = cmap.lookup_cid_code(
        0xD800_DC00u32.wrapping_add(step.wrapping_mul(0x0001_0007)),
        length,
      );
      let Ok(strided) = cmap.lookup_cid_code(step.wrapping_mul(STRIDE), length);

      let Ok(()) = held.mix_cid(surrogates);
      let Ok(()) = held.mix_cid(strided);
    }
  }

  for code in 0u32..=0x1FFFF {
    let Ok(mapping) = cmap.lookup_bf_string(code);

    match mapping {
      None => {
        let Ok(()) = held.mix(u64::MAX);
      }
      Some(BfString::Char(character)) => {
        let Ok(()) = held.mix(u64::from(u32::from(character)));
      }
      Some(BfString::String(text)) => {
        let Ok(()) = held.mix(7);

        for byte in text.bytes() {
          let Ok(()) = held.mix(u64::from(byte));
        }
      }
    }
  }

  Ok(held.0)
}

#[test]
fn every_embedded_map_answers_as_it_did_when_it_came_in() -> Result<(), Error> {
  let mut drifted = Vec::new();

  for (name, expected) in ANSWERS {
    let found = fingerprint(name)?;

    match found.cmp(&expected) {
      core::cmp::Ordering::Equal => {}
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
        drifted.push(format!("{name} {found:#018x}"));
      }
    }
  }

  match drifted.as_slice() {
    [] => Ok(()),
    [_, ..] => Err(Error::Drifted(drifted.join("\n"))),
  }
}
