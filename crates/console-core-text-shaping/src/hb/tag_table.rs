use alloc::vec::Vec;
use read_fonts::types::Tag;

pub(crate) struct LangTag {
  pub language: [u8; 4],
  pub tag: Tag,
}

const fn lang(language: &[u8]) -> [u8; 4] {
  let mut bytes = [0; 4];
  let mut i = 0;

  while i < language.len() {
    bytes[i] = language[i];
    i += 1;
  }

  bytes
}

use super::tag::{lang_matches, strncmp, subtag_matches};

#[rustfmt::skip]
pub(crate) static OPEN_TYPE_LANGUAGES: &[LangTag] = &[
    LangTag { language: lang(b"aa"), 	tag: ({ let Ok(value) = Tag::new(b"AFR "); value }) },
    LangTag { language: lang(b"aae"), 	tag: ({ let Ok(value) = Tag::new(b"SQI "); value }) },
    LangTag { language: lang(b"aao"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },

    LangTag { language: lang(b"aat"), 	tag: ({ let Ok(value) = Tag::new(b"SQI "); value }) },
    LangTag { language: lang(b"ab"), 	tag: ({ let Ok(value) = Tag::new(b"ABK "); value }) },
    LangTag { language: lang(b"aba"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"abh"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"abq"), 	tag: ({ let Ok(value) = Tag::new(b"ABA "); value }) },
    LangTag { language: lang(b"abs"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"abv"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"acf"), 	tag: ({ let Ok(value) = Tag::new(b"FAN "); value }) },
    LangTag { language: lang(b"acf"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"acm"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"acq"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"acr"), 	tag: ({ let Ok(value) = Tag::new(b"ACR "); value }) },
    LangTag { language: lang(b"acr"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"acw"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"acx"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"acy"), 	tag: ({ let Ok(value) = Tag::new(b"ACY "); value }) },
    LangTag { language: lang(b"acy"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ada"), 	tag: ({ let Ok(value) = Tag::new(b"DNG "); value }) },
    LangTag { language: lang(b"adf"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"adp"), 	tag: ({ let Ok(value) = Tag::new(b"DZN "); value }) },

    LangTag { language: lang(b"aeb"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"aec"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"af"), 	tag: ({ let Ok(value) = Tag::new(b"AFK "); value }) },
    LangTag { language: lang(b"afb"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"afk"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"afs"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"agu"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"agw"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ahg"), 	tag: ({ let Ok(value) = Tag::new(b"AGW "); value }) },
    LangTag { language: lang(b"aht"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"aig"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"aii"), 	tag: ({ let Ok(value) = Tag::new(b"SWA "); value }) },
    LangTag { language: lang(b"aii"), 	tag: ({ let Ok(value) = Tag::new(b"SYR "); value }) },

    LangTag { language: lang(b"aiw"), 	tag: ({ let Ok(value) = Tag::new(b"ARI "); value }) },
    LangTag { language: lang(b"ajp"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ajt"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ak"), 	tag: ({ let Ok(value) = Tag::new(b"AKA "); value }) },
    LangTag { language: lang(b"akb"), 	tag: ({ let Ok(value) = Tag::new(b"AKB "); value }) },
    LangTag { language: lang(b"akb"), 	tag: ({ let Ok(value) = Tag::new(b"BTK "); value }) },
    LangTag { language: lang(b"aln"), 	tag: ({ let Ok(value) = Tag::new(b"SQI "); value }) },
    LangTag { language: lang(b"als"), 	tag: ({ let Ok(value) = Tag::new(b"SQI "); value }) },

    LangTag { language: lang(b"am"), 	tag: ({ let Ok(value) = Tag::new(b"AMH "); value }) },
    LangTag { language: lang(b"amf"), 	tag: ({ let Ok(value) = Tag::new(b"HBN "); value }) },
    LangTag { language: lang(b"amw"), 	tag: ({ let Ok(value) = Tag::new(b"SYR "); value }) },
    LangTag { language: lang(b"an"), 	tag: ({ let Ok(value) = Tag::new(b"ARG "); value }) },

    LangTag { language: lang(b"aoa"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"apa"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"apc"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"apd"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"apj"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"apk"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"apl"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"apm"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"apw"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"ar"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"arb"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ari"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ark"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"arn"), 	tag: ({ let Ok(value) = Tag::new(b"MAP "); value }) },
    LangTag { language: lang(b"arq"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ars"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ary"), 	tag: ({ let Ok(value) = Tag::new(b"MOR "); value }) },
    LangTag { language: lang(b"ary"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"arz"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"as"), 	tag: ({ let Ok(value) = Tag::new(b"ASM "); value }) },


    LangTag { language: lang(b"atj"), 	tag: ({ let Ok(value) = Tag::new(b"RCR "); value }) },

    LangTag { language: lang(b"atv"), 	tag: ({ let Ok(value) = Tag::new(b"ALT "); value }) },
    LangTag { language: lang(b"auj"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"auz"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"av"), 	tag: ({ let Ok(value) = Tag::new(b"AVR "); value }) },
    LangTag { language: lang(b"avl"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },


    LangTag { language: lang(b"ay"), 	tag: ({ let Ok(value) = Tag::new(b"AYM "); value }) },
    LangTag { language: lang(b"ayc"), 	tag: ({ let Ok(value) = Tag::new(b"AYM "); value }) },
    LangTag { language: lang(b"ayh"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ayl"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ayn"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ayp"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ayr"), 	tag: ({ let Ok(value) = Tag::new(b"AYM "); value }) },
    LangTag { language: lang(b"az"), 	tag: ({ let Ok(value) = Tag::new(b"AZE "); value }) },
    LangTag { language: lang(b"azb"), 	tag: ({ let Ok(value) = Tag::new(b"AZB "); value }) },
    LangTag { language: lang(b"azb"), 	tag: ({ let Ok(value) = Tag::new(b"AZE "); value }) },
    LangTag { language: lang(b"azd"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"azj"), 	tag: ({ let Ok(value) = Tag::new(b"AZE "); value }) },
    LangTag { language: lang(b"azn"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"azz"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"ba"), 	tag: ({ let Ok(value) = Tag::new(b"BSH "); value }) },
    LangTag { language: lang(b"bad"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"bag"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"bah"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"bai"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"bal"), 	tag: ({ let Ok(value) = Tag::new(b"BLI "); value }) },


    LangTag { language: lang(b"bau"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"bbc"), 	tag: ({ let Ok(value) = Tag::new(b"BBC "); value }) },
    LangTag { language: lang(b"bbc"), 	tag: ({ let Ok(value) = Tag::new(b"BTK "); value }) },
    LangTag { language: lang(b"bbj"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"bbp"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"bbr"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"bbz"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"bcc"), 	tag: ({ let Ok(value) = Tag::new(b"BLI "); value }) },
    LangTag { language: lang(b"bch"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"bci"), 	tag: ({ let Ok(value) = Tag::new(b"BAU "); value }) },
    LangTag { language: lang(b"bcl"), 	tag: ({ let Ok(value) = Tag::new(b"BIK "); value }) },
    LangTag { language: lang(b"bcq"), 	tag: ({ let Ok(value) = Tag::new(b"BCH "); value }) },
    LangTag { language: lang(b"bcr"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },


    LangTag { language: lang(b"be"), 	tag: ({ let Ok(value) = Tag::new(b"BEL "); value }) },
    LangTag { language: lang(b"bea"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"beb"), 	tag: ({ let Ok(value) = Tag::new(b"BTI "); value }) },

    LangTag { language: lang(b"ber"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"bew"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"bfl"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"bfq"), 	tag: ({ let Ok(value) = Tag::new(b"BAD "); value }) },
    LangTag { language: lang(b"bft"), 	tag: ({ let Ok(value) = Tag::new(b"BLT "); value }) },
    LangTag { language: lang(b"bfu"), 	tag: ({ let Ok(value) = Tag::new(b"LAH "); value }) },
    LangTag { language: lang(b"bfy"), 	tag: ({ let Ok(value) = Tag::new(b"BAG "); value }) },
    LangTag { language: lang(b"bg"), 	tag: ({ let Ok(value) = Tag::new(b"BGR "); value }) },

    LangTag { language: lang(b"bgn"), 	tag: ({ let Ok(value) = Tag::new(b"BLI "); value }) },
    LangTag { language: lang(b"bgp"), 	tag: ({ let Ok(value) = Tag::new(b"BLI "); value }) },
    LangTag { language: lang(b"bgq"), 	tag: ({ let Ok(value) = Tag::new(b"BGQ "); value }) },
    LangTag { language: lang(b"bgq"), 	tag: ({ let Ok(value) = Tag::new(b"RAJ "); value }) },
    LangTag { language: lang(b"bgr"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"bhb"), 	tag: ({ let Ok(value) = Tag::new(b"BHI "); value }) },

    LangTag { language: lang(b"bhk"), 	tag: ({ let Ok(value) = Tag::new(b"BIK "); value }) },

    LangTag { language: lang(b"bhr"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"bi"), 	tag: ({ let Ok(value) = Tag::new(b"BIS "); value }) },
    LangTag { language: lang(b"bi"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"bil"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"bin"), 	tag: ({ let Ok(value) = Tag::new(b"EDO "); value }) },
    LangTag { language: lang(b"biu"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },

    LangTag { language: lang(b"bjn"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"bjo"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"bjq"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"bjs"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"bjt"), 	tag: ({ let Ok(value) = Tag::new(b"BLN "); value }) },
    LangTag { language: lang(b"bkf"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"bko"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"bla"), 	tag: ({ let Ok(value) = Tag::new(b"BKF "); value }) },
    LangTag { language: lang(b"ble"), 	tag: ({ let Ok(value) = Tag::new(b"BLN "); value }) },
    LangTag { language: lang(b"blg"), 	tag: ({ let Ok(value) = Tag::new(b"IBA "); value }) },
    LangTag { language: lang(b"bli"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"blk"), 	tag: ({ let Ok(value) = Tag::new(b"BLK "); value }) },
    LangTag { language: lang(b"blk"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"bln"), 	tag: ({ let Ok(value) = Tag::new(b"BIK "); value }) },
    LangTag { language: lang(b"blt"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"bm"), 	tag: ({ let Ok(value) = Tag::new(b"BMB "); value }) },
    LangTag { language: lang(b"bmb"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"bml"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"bmm"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"bn"), 	tag: ({ let Ok(value) = Tag::new(b"BEN "); value }) },
    LangTag { language: lang(b"bo"), 	tag: ({ let Ok(value) = Tag::new(b"TIB "); value }) },
    LangTag { language: lang(b"bpd"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"bpl"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"bpq"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"bqi"), 	tag: ({ let Ok(value) = Tag::new(b"LRC "); value }) },
    LangTag { language: lang(b"bqk"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"br"), 	tag: ({ let Ok(value) = Tag::new(b"BRE "); value }) },
    LangTag { language: lang(b"bra"), 	tag: ({ let Ok(value) = Tag::new(b"BRI "); value }) },
    LangTag { language: lang(b"brc"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"bri"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"brm"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"bs"), 	tag: ({ let Ok(value) = Tag::new(b"BOS "); value }) },
    LangTag { language: lang(b"bsh"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"btb"), 	tag: ({ let Ok(value) = Tag::new(b"BTI "); value }) },
    LangTag { language: lang(b"btd"), 	tag: ({ let Ok(value) = Tag::new(b"BTD "); value }) },
    LangTag { language: lang(b"btd"), 	tag: ({ let Ok(value) = Tag::new(b"BTK "); value }) },
    LangTag { language: lang(b"bti"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"btj"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },

    LangTag { language: lang(b"btm"), 	tag: ({ let Ok(value) = Tag::new(b"BTM "); value }) },
    LangTag { language: lang(b"btm"), 	tag: ({ let Ok(value) = Tag::new(b"BTK "); value }) },
    LangTag { language: lang(b"bto"), 	tag: ({ let Ok(value) = Tag::new(b"BIK "); value }) },
    LangTag { language: lang(b"bts"), 	tag: ({ let Ok(value) = Tag::new(b"BTS "); value }) },
    LangTag { language: lang(b"bts"), 	tag: ({ let Ok(value) = Tag::new(b"BTK "); value }) },
    LangTag { language: lang(b"btx"), 	tag: ({ let Ok(value) = Tag::new(b"BTX "); value }) },
    LangTag { language: lang(b"btx"), 	tag: ({ let Ok(value) = Tag::new(b"BTK "); value }) },
    LangTag { language: lang(b"btz"), 	tag: ({ let Ok(value) = Tag::new(b"BTZ "); value }) },
    LangTag { language: lang(b"btz"), 	tag: ({ let Ok(value) = Tag::new(b"BTK "); value }) },

    LangTag { language: lang(b"bum"), 	tag: ({ let Ok(value) = Tag::new(b"BTI "); value }) },
    LangTag { language: lang(b"bve"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"bvu"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"bwe"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"bxk"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"bxo"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"bxp"), 	tag: ({ let Ok(value) = Tag::new(b"BTI "); value }) },
    LangTag { language: lang(b"bxr"), 	tag: ({ let Ok(value) = Tag::new(b"RBU "); value }) },
    LangTag { language: lang(b"byn"), 	tag: ({ let Ok(value) = Tag::new(b"BIL "); value }) },
    LangTag { language: lang(b"byv"), 	tag: ({ let Ok(value) = Tag::new(b"BYV "); value }) },
    LangTag { language: lang(b"byv"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"bzc"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"bzj"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"bzk"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"ca"), 	tag: ({ let Ok(value) = Tag::new(b"CAT "); value }) },
    LangTag { language: lang(b"caa"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"cac"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"caf"), 	tag: ({ let Ok(value) = Tag::new(b"CRR "); value }) },
    LangTag { language: lang(b"caf"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"cak"), 	tag: ({ let Ok(value) = Tag::new(b"CAK "); value }) },
    LangTag { language: lang(b"cak"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },


    LangTag { language: lang(b"cbk"), 	tag: ({ let Ok(value) = Tag::new(b"CBK "); value }) },
    LangTag { language: lang(b"cbk"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"cbl"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"ccl"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"ccm"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"cco"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },
    LangTag { language: lang(b"ccq"), 	tag: ({ let Ok(value) = Tag::new(b"ARK "); value }) },
    LangTag { language: lang(b"cdo"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"ce"), 	tag: ({ let Ok(value) = Tag::new(b"CHE "); value }) },

    LangTag { language: lang(b"cek"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"cey"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"cfm"), 	tag: ({ let Ok(value) = Tag::new(b"HAL "); value }) },
    LangTag { language: lang(b"cfm"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },

    LangTag { language: lang(b"ch"), 	tag: ({ let Ok(value) = Tag::new(b"CHA "); value }) },
    LangTag { language: lang(b"chf"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"chg"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"chh"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"chj"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },
    LangTag { language: lang(b"chk"), 	tag: ({ let Ok(value) = Tag::new(b"CHK0"); value }) },
    LangTag { language: lang(b"chm"), 	tag: ({ let Ok(value) = Tag::new(b"HMA "); value }) },
    LangTag { language: lang(b"chm"), 	tag: ({ let Ok(value) = Tag::new(b"LMA "); value }) },
    LangTag { language: lang(b"chn"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"chp"), 	tag: ({ let Ok(value) = Tag::new(b"CHP "); value }) },
    LangTag { language: lang(b"chp"), 	tag: ({ let Ok(value) = Tag::new(b"SAY "); value }) },
    LangTag { language: lang(b"chp"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"chq"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },


    LangTag { language: lang(b"chz"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },
    LangTag { language: lang(b"ciw"), 	tag: ({ let Ok(value) = Tag::new(b"OJB "); value }) },


    LangTag { language: lang(b"cjy"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"cka"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"ckb"), 	tag: ({ let Ok(value) = Tag::new(b"KUR "); value }) },
    LangTag { language: lang(b"ckn"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"cks"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"ckt"), 	tag: ({ let Ok(value) = Tag::new(b"CHK "); value }) },
    LangTag { language: lang(b"ckz"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"clc"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"cld"), 	tag: ({ let Ok(value) = Tag::new(b"SYR "); value }) },
    LangTag { language: lang(b"cle"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },
    LangTag { language: lang(b"clj"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"cls"), 	tag: ({ let Ok(value) = Tag::new(b"SAN "); value }) },
    LangTag { language: lang(b"clt"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },

    LangTag { language: lang(b"cmn"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"cmr"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"cnb"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"cnh"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"cnk"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"cnl"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },
    LangTag { language: lang(b"cnp"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"cnr"), 	tag: ({ let Ok(value) = Tag::new(b"SRB "); value }) },
    LangTag { language: lang(b"cnt"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },
    LangTag { language: lang(b"cnu"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"cnw"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"co"), 	tag: ({ let Ok(value) = Tag::new(b"COS "); value }) },
    LangTag { language: lang(b"coa"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"cob"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },


    LangTag { language: lang(b"coq"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"cpa"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },
    LangTag { language: lang(b"cpe"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"cpf"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"cpi"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"cpx"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"cqd"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"cqu"), 	tag: ({ let Ok(value) = Tag::new(b"QUH "); value }) },
    LangTag { language: lang(b"cqu"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"cr"), 	tag: ({ let Ok(value) = Tag::new(b"CRE "); value }) },
    LangTag { language: lang(b"crh"), 	tag: ({ let Ok(value) = Tag::new(b"CRT "); value }) },
    LangTag { language: lang(b"cri"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"crj"), 	tag: ({ let Ok(value) = Tag::new(b"ECR "); value }) },
    LangTag { language: lang(b"crj"), 	tag: ({ let Ok(value) = Tag::new(b"YCR "); value }) },
    LangTag { language: lang(b"crj"), 	tag: ({ let Ok(value) = Tag::new(b"CRE "); value }) },
    LangTag { language: lang(b"crk"), 	tag: ({ let Ok(value) = Tag::new(b"WCR "); value }) },
    LangTag { language: lang(b"crk"), 	tag: ({ let Ok(value) = Tag::new(b"YCR "); value }) },
    LangTag { language: lang(b"crk"), 	tag: ({ let Ok(value) = Tag::new(b"CRE "); value }) },
    LangTag { language: lang(b"crl"), 	tag: ({ let Ok(value) = Tag::new(b"ECR "); value }) },
    LangTag { language: lang(b"crl"), 	tag: ({ let Ok(value) = Tag::new(b"YCR "); value }) },
    LangTag { language: lang(b"crl"), 	tag: ({ let Ok(value) = Tag::new(b"CRE "); value }) },
    LangTag { language: lang(b"crm"), 	tag: ({ let Ok(value) = Tag::new(b"MCR "); value }) },
    LangTag { language: lang(b"crm"), 	tag: ({ let Ok(value) = Tag::new(b"LCR "); value }) },
    LangTag { language: lang(b"crm"), 	tag: ({ let Ok(value) = Tag::new(b"CRE "); value }) },
    LangTag { language: lang(b"crp"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"crr"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"crs"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"crt"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"crx"), 	tag: ({ let Ok(value) = Tag::new(b"CRR "); value }) },
    LangTag { language: lang(b"crx"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"cs"), 	tag: ({ let Ok(value) = Tag::new(b"CSY "); value }) },
    LangTag { language: lang(b"csa"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },

    LangTag { language: lang(b"csh"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"csj"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"csl"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"cso"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },
    LangTag { language: lang(b"csp"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"csv"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"csw"), 	tag: ({ let Ok(value) = Tag::new(b"NCR "); value }) },
    LangTag { language: lang(b"csw"), 	tag: ({ let Ok(value) = Tag::new(b"NHC "); value }) },
    LangTag { language: lang(b"csw"), 	tag: ({ let Ok(value) = Tag::new(b"CRE "); value }) },
    LangTag { language: lang(b"csy"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"ctc"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"ctd"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"cte"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },

    LangTag { language: lang(b"cth"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"ctl"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },

    LangTag { language: lang(b"cts"), 	tag: ({ let Ok(value) = Tag::new(b"BIK "); value }) },

    LangTag { language: lang(b"ctu"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"cu"), 	tag: ({ let Ok(value) = Tag::new(b"CSL "); value }) },
    LangTag { language: lang(b"cuc"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },

    LangTag { language: lang(b"cv"), 	tag: ({ let Ok(value) = Tag::new(b"CHU "); value }) },
    LangTag { language: lang(b"cvn"), 	tag: ({ let Ok(value) = Tag::new(b"CCHN"); value }) },
    LangTag { language: lang(b"cwd"), 	tag: ({ let Ok(value) = Tag::new(b"DCR "); value }) },
    LangTag { language: lang(b"cwd"), 	tag: ({ let Ok(value) = Tag::new(b"TCR "); value }) },
    LangTag { language: lang(b"cwd"), 	tag: ({ let Ok(value) = Tag::new(b"CRE "); value }) },
    LangTag { language: lang(b"cy"), 	tag: ({ let Ok(value) = Tag::new(b"WEL "); value }) },
    LangTag { language: lang(b"czh"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"czo"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"czt"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"da"), 	tag: ({ let Ok(value) = Tag::new(b"DAN "); value }) },

    LangTag { language: lang(b"dao"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"dap"), 	tag: ({ let Ok(value) = Tag::new(b"NIS "); value }) },


    LangTag { language: lang(b"dcr"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"de"), 	tag: ({ let Ok(value) = Tag::new(b"DEU "); value }) },
    LangTag { language: lang(b"den"), 	tag: ({ let Ok(value) = Tag::new(b"SLA "); value }) },
    LangTag { language: lang(b"den"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"dep"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"dgo"), 	tag: ({ let Ok(value) = Tag::new(b"DGO "); value }) },
    LangTag { language: lang(b"dgo"), 	tag: ({ let Ok(value) = Tag::new(b"DGR "); value }) },
    LangTag { language: lang(b"dgr"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"dhd"), 	tag: ({ let Ok(value) = Tag::new(b"MAW "); value }) },

    LangTag { language: lang(b"dhv"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"dib"), 	tag: ({ let Ok(value) = Tag::new(b"DNK "); value }) },
    LangTag { language: lang(b"dik"), 	tag: ({ let Ok(value) = Tag::new(b"DNK "); value }) },
    LangTag { language: lang(b"din"), 	tag: ({ let Ok(value) = Tag::new(b"DNK "); value }) },
    LangTag { language: lang(b"dip"), 	tag: ({ let Ok(value) = Tag::new(b"DNK "); value }) },
    LangTag { language: lang(b"diq"), 	tag: ({ let Ok(value) = Tag::new(b"DIQ "); value }) },
    LangTag { language: lang(b"diq"), 	tag: ({ let Ok(value) = Tag::new(b"ZZA "); value }) },
    LangTag { language: lang(b"diw"), 	tag: ({ let Ok(value) = Tag::new(b"DNK "); value }) },
    LangTag { language: lang(b"dje"), 	tag: ({ let Ok(value) = Tag::new(b"DJR "); value }) },
    LangTag { language: lang(b"djk"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"djr"), 	tag: ({ let Ok(value) = Tag::new(b"DJR0"); value }) },
    LangTag { language: lang(b"dks"), 	tag: ({ let Ok(value) = Tag::new(b"DNK "); value }) },
    LangTag { language: lang(b"dng"), 	tag: ({ let Ok(value) = Tag::new(b"DUN "); value }) },

    LangTag { language: lang(b"dnk"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"doi"), 	tag: ({ let Ok(value) = Tag::new(b"DGR "); value }) },
    LangTag { language: lang(b"drh"), 	tag: ({ let Ok(value) = Tag::new(b"MNG "); value }) },
    LangTag { language: lang(b"dri"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"drw"), 	tag: ({ let Ok(value) = Tag::new(b"DRI "); value }) },
    LangTag { language: lang(b"drw"), 	tag: ({ let Ok(value) = Tag::new(b"FAR "); value }) },
    LangTag { language: lang(b"dsb"), 	tag: ({ let Ok(value) = Tag::new(b"LSB "); value }) },
    LangTag { language: lang(b"dty"), 	tag: ({ let Ok(value) = Tag::new(b"NEP "); value }) },

    LangTag { language: lang(b"dun"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"dup"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"dv"), 	tag: ({ let Ok(value) = Tag::new(b"DIV "); value }) },
    LangTag { language: lang(b"dv"), 	tag: ({ let Ok(value) = Tag::new(b"DHV "); value }) },
    LangTag { language: lang(b"dwk"), 	tag: ({ let Ok(value) = Tag::new(b"KUI "); value }) },
    LangTag { language: lang(b"dwu"), 	tag: ({ let Ok(value) = Tag::new(b"DUJ "); value }) },
    LangTag { language: lang(b"dwy"), 	tag: ({ let Ok(value) = Tag::new(b"DUJ "); value }) },
    LangTag { language: lang(b"dyu"), 	tag: ({ let Ok(value) = Tag::new(b"JUL "); value }) },
    LangTag { language: lang(b"dz"), 	tag: ({ let Ok(value) = Tag::new(b"DZN "); value }) },
    LangTag { language: lang(b"dzn"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ecr"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ee"), 	tag: ({ let Ok(value) = Tag::new(b"EWE "); value }) },

    LangTag { language: lang(b"ekk"), 	tag: ({ let Ok(value) = Tag::new(b"ETI "); value }) },
    LangTag { language: lang(b"eky"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"el"), 	tag: ({ let Ok(value) = Tag::new(b"ELL "); value }) },
    LangTag { language: lang(b"emk"), 	tag: ({ let Ok(value) = Tag::new(b"EMK "); value }) },
    LangTag { language: lang(b"emk"), 	tag: ({ let Ok(value) = Tag::new(b"MNK "); value }) },

    LangTag { language: lang(b"emy"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"en"), 	tag: ({ let Ok(value) = Tag::new(b"ENG "); value }) },
    LangTag { language: lang(b"enb"), 	tag: ({ let Ok(value) = Tag::new(b"KAL "); value }) },
    LangTag { language: lang(b"enf"), 	tag: ({ let Ok(value) = Tag::new(b"FNE "); value }) },
    LangTag { language: lang(b"enh"), 	tag: ({ let Ok(value) = Tag::new(b"TNE "); value }) },
    LangTag { language: lang(b"eo"), 	tag: ({ let Ok(value) = Tag::new(b"NTO "); value }) },
    LangTag { language: lang(b"es"), 	tag: ({ let Ok(value) = Tag::new(b"ESP "); value }) },
    LangTag { language: lang(b"esg"), 	tag: ({ let Ok(value) = Tag::new(b"GON "); value }) },
    LangTag { language: lang(b"esi"), 	tag: ({ let Ok(value) = Tag::new(b"IPK "); value }) },
    LangTag { language: lang(b"esk"), 	tag: ({ let Ok(value) = Tag::new(b"IPK "); value }) },

    LangTag { language: lang(b"et"), 	tag: ({ let Ok(value) = Tag::new(b"ETI "); value }) },
    LangTag { language: lang(b"eto"), 	tag: ({ let Ok(value) = Tag::new(b"BTI "); value }) },
    LangTag { language: lang(b"eu"), 	tag: ({ let Ok(value) = Tag::new(b"EUQ "); value }) },
    LangTag { language: lang(b"euq"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"eve"), 	tag: ({ let Ok(value) = Tag::new(b"EVN "); value }) },
    LangTag { language: lang(b"evn"), 	tag: ({ let Ok(value) = Tag::new(b"EVK "); value }) },
    LangTag { language: lang(b"ewo"), 	tag: ({ let Ok(value) = Tag::new(b"BTI "); value }) },
    LangTag { language: lang(b"eyo"), 	tag: ({ let Ok(value) = Tag::new(b"KAL "); value }) },
    LangTag { language: lang(b"fa"), 	tag: ({ let Ok(value) = Tag::new(b"FAR "); value }) },
    LangTag { language: lang(b"fab"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"fan"), 	tag: ({ let Ok(value) = Tag::new(b"FAN0"); value }) },
    LangTag { language: lang(b"fan"), 	tag: ({ let Ok(value) = Tag::new(b"BTI "); value }) },
    LangTag { language: lang(b"far"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"fat"), 	tag: ({ let Ok(value) = Tag::new(b"FAT "); value }) },
    LangTag { language: lang(b"fat"), 	tag: ({ let Ok(value) = Tag::new(b"AKA "); value }) },
    LangTag { language: lang(b"fbl"), 	tag: ({ let Ok(value) = Tag::new(b"BIK "); value }) },
    LangTag { language: lang(b"ff"), 	tag: ({ let Ok(value) = Tag::new(b"FUL "); value }) },
    LangTag { language: lang(b"ffm"), 	tag: ({ let Ok(value) = Tag::new(b"FUL "); value }) },
    LangTag { language: lang(b"fi"), 	tag: ({ let Ok(value) = Tag::new(b"FIN "); value }) },
    LangTag { language: lang(b"fil"), 	tag: ({ let Ok(value) = Tag::new(b"PIL "); value }) },
    LangTag { language: lang(b"fj"), 	tag: ({ let Ok(value) = Tag::new(b"FJI "); value }) },
    LangTag { language: lang(b"flm"), 	tag: ({ let Ok(value) = Tag::new(b"HAL "); value }) },
    LangTag { language: lang(b"flm"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"fmp"), 	tag: ({ let Ok(value) = Tag::new(b"FMP "); value }) },
    LangTag { language: lang(b"fmp"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"fng"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"fo"), 	tag: ({ let Ok(value) = Tag::new(b"FOS "); value }) },

    LangTag { language: lang(b"fos"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"fpe"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"fr"), 	tag: ({ let Ok(value) = Tag::new(b"FRA "); value }) },


    LangTag { language: lang(b"fub"), 	tag: ({ let Ok(value) = Tag::new(b"FUL "); value }) },
    LangTag { language: lang(b"fuc"), 	tag: ({ let Ok(value) = Tag::new(b"FUL "); value }) },
    LangTag { language: lang(b"fue"), 	tag: ({ let Ok(value) = Tag::new(b"FUL "); value }) },
    LangTag { language: lang(b"fuf"), 	tag: ({ let Ok(value) = Tag::new(b"FTA "); value }) },
    LangTag { language: lang(b"fuf"), 	tag: ({ let Ok(value) = Tag::new(b"FUL "); value }) },
    LangTag { language: lang(b"fuh"), 	tag: ({ let Ok(value) = Tag::new(b"FUL "); value }) },
    LangTag { language: lang(b"fui"), 	tag: ({ let Ok(value) = Tag::new(b"FUL "); value }) },
    LangTag { language: lang(b"fuq"), 	tag: ({ let Ok(value) = Tag::new(b"FUL "); value }) },
    LangTag { language: lang(b"fur"), 	tag: ({ let Ok(value) = Tag::new(b"FRL "); value }) },
    LangTag { language: lang(b"fuv"), 	tag: ({ let Ok(value) = Tag::new(b"FUV "); value }) },
    LangTag { language: lang(b"fuv"), 	tag: ({ let Ok(value) = Tag::new(b"FUL "); value }) },
    LangTag { language: lang(b"fy"), 	tag: ({ let Ok(value) = Tag::new(b"FRI "); value }) },
    LangTag { language: lang(b"ga"), 	tag: ({ let Ok(value) = Tag::new(b"IRI "); value }) },
    LangTag { language: lang(b"ga"), 	tag: ({ let Ok(value) = Tag::new(b"IRT "); value }) },
    LangTag { language: lang(b"gaa"), 	tag: ({ let Ok(value) = Tag::new(b"GAD "); value }) },
    LangTag { language: lang(b"gac"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"gad"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"gae"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"gal"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"gan"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"gar"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"gaw"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"gax"), 	tag: ({ let Ok(value) = Tag::new(b"ORO "); value }) },
    LangTag { language: lang(b"gaz"), 	tag: ({ let Ok(value) = Tag::new(b"ORO "); value }) },
    LangTag { language: lang(b"gbm"), 	tag: ({ let Ok(value) = Tag::new(b"GAW "); value }) },
    LangTag { language: lang(b"gce"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"gcf"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"gcl"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"gcr"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"gd"), 	tag: ({ let Ok(value) = Tag::new(b"GAE "); value }) },
    LangTag { language: lang(b"gda"), 	tag: ({ let Ok(value) = Tag::new(b"RAJ "); value }) },

    LangTag { language: lang(b"ggo"), 	tag: ({ let Ok(value) = Tag::new(b"GON "); value }) },
    LangTag { language: lang(b"gha"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"ghc"), 	tag: ({ let Ok(value) = Tag::new(b"IRT "); value }) },
    LangTag { language: lang(b"ghk"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"gho"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"gib"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"gil"), 	tag: ({ let Ok(value) = Tag::new(b"GIL0"); value }) },
    LangTag { language: lang(b"gju"), 	tag: ({ let Ok(value) = Tag::new(b"RAJ "); value }) },
    LangTag { language: lang(b"gkp"), 	tag: ({ let Ok(value) = Tag::new(b"GKP "); value }) },
    LangTag { language: lang(b"gkp"), 	tag: ({ let Ok(value) = Tag::new(b"KPL "); value }) },
    LangTag { language: lang(b"gl"), 	tag: ({ let Ok(value) = Tag::new(b"GAL "); value }) },
    LangTag { language: lang(b"gld"), 	tag: ({ let Ok(value) = Tag::new(b"NAN "); value }) },

    LangTag { language: lang(b"gmz"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"gn"), 	tag: ({ let Ok(value) = Tag::new(b"GUA "); value }) },
    LangTag { language: lang(b"gnb"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },

    LangTag { language: lang(b"gno"), 	tag: ({ let Ok(value) = Tag::new(b"GON "); value }) },
    LangTag { language: lang(b"gnw"), 	tag: ({ let Ok(value) = Tag::new(b"GUA "); value }) },

    LangTag { language: lang(b"gom"), 	tag: ({ let Ok(value) = Tag::new(b"KOK "); value }) },

    LangTag { language: lang(b"goq"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"gox"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"gpe"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"gro"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"grr"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"grt"), 	tag: ({ let Ok(value) = Tag::new(b"GRO "); value }) },
    LangTag { language: lang(b"gru"), 	tag: ({ let Ok(value) = Tag::new(b"SOG "); value }) },
    LangTag { language: lang(b"gsw"), 	tag: ({ let Ok(value) = Tag::new(b"ALS "); value }) },
    LangTag { language: lang(b"gu"), 	tag: ({ let Ok(value) = Tag::new(b"GUJ "); value }) },
    LangTag { language: lang(b"gua"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },


    LangTag { language: lang(b"gug"), 	tag: ({ let Ok(value) = Tag::new(b"GUA "); value }) },
    LangTag { language: lang(b"gui"), 	tag: ({ let Ok(value) = Tag::new(b"GUA "); value }) },
    LangTag { language: lang(b"guk"), 	tag: ({ let Ok(value) = Tag::new(b"GMZ "); value }) },
    LangTag { language: lang(b"gul"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"gun"), 	tag: ({ let Ok(value) = Tag::new(b"GUA "); value }) },

    LangTag { language: lang(b"gv"), 	tag: ({ let Ok(value) = Tag::new(b"MNX "); value }) },
    LangTag { language: lang(b"gwi"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"gyn"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"ha"), 	tag: ({ let Ok(value) = Tag::new(b"HAU "); value }) },
    LangTag { language: lang(b"haa"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"hae"), 	tag: ({ let Ok(value) = Tag::new(b"ORO "); value }) },
    LangTag { language: lang(b"hai"), 	tag: ({ let Ok(value) = Tag::new(b"HAI0"); value }) },
    LangTag { language: lang(b"hak"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"hal"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"har"), 	tag: ({ let Ok(value) = Tag::new(b"HRI "); value }) },

    LangTag { language: lang(b"hax"), 	tag: ({ let Ok(value) = Tag::new(b"HAI0"); value }) },


    LangTag { language: lang(b"hbn"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"hca"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"hdn"), 	tag: ({ let Ok(value) = Tag::new(b"HAI0"); value }) },
    LangTag { language: lang(b"he"), 	tag: ({ let Ok(value) = Tag::new(b"IWR "); value }) },
    LangTag { language: lang(b"hea"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },

    LangTag { language: lang(b"hi"), 	tag: ({ let Ok(value) = Tag::new(b"HIN "); value }) },

    LangTag { language: lang(b"hji"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"hlt"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"hma"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmc"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmd"), 	tag: ({ let Ok(value) = Tag::new(b"HMD "); value }) },
    LangTag { language: lang(b"hmd"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hme"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmg"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmh"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmi"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmj"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hml"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmm"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },

    LangTag { language: lang(b"hmp"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmq"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmr"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"hms"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmw"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmy"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hmz"), 	tag: ({ let Ok(value) = Tag::new(b"HMZ "); value }) },
    LangTag { language: lang(b"hmz"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },

    LangTag { language: lang(b"hne"), 	tag: ({ let Ok(value) = Tag::new(b"CHH "); value }) },
    LangTag { language: lang(b"hnj"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hnm"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"hno"), 	tag: ({ let Ok(value) = Tag::new(b"HND "); value }) },
    LangTag { language: lang(b"ho"), 	tag: ({ let Ok(value) = Tag::new(b"HMO "); value }) },
    LangTag { language: lang(b"ho"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"hoc"), 	tag: ({ let Ok(value) = Tag::new(b"HO  "); value }) },
    LangTag { language: lang(b"hoi"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"hoj"), 	tag: ({ let Ok(value) = Tag::new(b"HAR "); value }) },
    LangTag { language: lang(b"hoj"), 	tag: ({ let Ok(value) = Tag::new(b"RAJ "); value }) },
    LangTag { language: lang(b"hr"), 	tag: ({ let Ok(value) = Tag::new(b"HRV "); value }) },
    LangTag { language: lang(b"hra"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"hrm"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hsb"), 	tag: ({ let Ok(value) = Tag::new(b"USB "); value }) },
    LangTag { language: lang(b"hsn"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"ht"), 	tag: ({ let Ok(value) = Tag::new(b"HAI "); value }) },
    LangTag { language: lang(b"ht"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"hu"), 	tag: ({ let Ok(value) = Tag::new(b"HUN "); value }) },
    LangTag { language: lang(b"huj"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"hup"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },

    LangTag { language: lang(b"hus"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"hwc"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"hy"), 	tag: ({ let Ok(value) = Tag::new(b"HYE0"); value }) },
    LangTag { language: lang(b"hy"), 	tag: ({ let Ok(value) = Tag::new(b"HYE "); value }) },
    LangTag { language: lang(b"hyw"), 	tag: ({ let Ok(value) = Tag::new(b"HYE "); value }) },
    LangTag { language: lang(b"hz"), 	tag: ({ let Ok(value) = Tag::new(b"HER "); value }) },
    LangTag { language: lang(b"ia"), 	tag: ({ let Ok(value) = Tag::new(b"INA "); value }) },


    LangTag { language: lang(b"iby"), 	tag: ({ let Ok(value) = Tag::new(b"IJO "); value }) },
    LangTag { language: lang(b"icr"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"id"), 	tag: ({ let Ok(value) = Tag::new(b"IND "); value }) },
    LangTag { language: lang(b"id"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"ida"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"idb"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"ie"), 	tag: ({ let Ok(value) = Tag::new(b"ILE "); value }) },
    LangTag { language: lang(b"ig"), 	tag: ({ let Ok(value) = Tag::new(b"IBO "); value }) },
    LangTag { language: lang(b"igb"), 	tag: ({ let Ok(value) = Tag::new(b"EBI "); value }) },
    LangTag { language: lang(b"ihb"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"ii"), 	tag: ({ let Ok(value) = Tag::new(b"YIM "); value }) },
    LangTag { language: lang(b"ijc"), 	tag: ({ let Ok(value) = Tag::new(b"IJO "); value }) },
    LangTag { language: lang(b"ije"), 	tag: ({ let Ok(value) = Tag::new(b"IJO "); value }) },
    LangTag { language: lang(b"ijn"), 	tag: ({ let Ok(value) = Tag::new(b"IJO "); value }) },

    LangTag { language: lang(b"ijs"), 	tag: ({ let Ok(value) = Tag::new(b"IJO "); value }) },
    LangTag { language: lang(b"ik"), 	tag: ({ let Ok(value) = Tag::new(b"IPK "); value }) },
    LangTag { language: lang(b"ike"), 	tag: ({ let Ok(value) = Tag::new(b"INU "); value }) },
    LangTag { language: lang(b"ike"), 	tag: ({ let Ok(value) = Tag::new(b"INUK"); value }) },
    LangTag { language: lang(b"ikt"), 	tag: ({ let Ok(value) = Tag::new(b"INU "); value }) },

    LangTag { language: lang(b"in"), 	tag: ({ let Ok(value) = Tag::new(b"IND "); value }) },
    LangTag { language: lang(b"in"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"ing"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"inh"), 	tag: ({ let Ok(value) = Tag::new(b"ING "); value }) },
    LangTag { language: lang(b"io"), 	tag: ({ let Ok(value) = Tag::new(b"IDO "); value }) },
    LangTag { language: lang(b"iri"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"is"), 	tag: ({ let Ok(value) = Tag::new(b"ISL "); value }) },
    LangTag { language: lang(b"ism"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"it"), 	tag: ({ let Ok(value) = Tag::new(b"ITA "); value }) },
    LangTag { language: lang(b"itz"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"iu"), 	tag: ({ let Ok(value) = Tag::new(b"INU "); value }) },
    LangTag { language: lang(b"iu"), 	tag: ({ let Ok(value) = Tag::new(b"INUK"); value }) },
    LangTag { language: lang(b"iw"), 	tag: ({ let Ok(value) = Tag::new(b"IWR "); value }) },
    LangTag { language: lang(b"ixl"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"ja"), 	tag: ({ let Ok(value) = Tag::new(b"JAN "); value }) },
    LangTag { language: lang(b"jac"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"jak"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"jam"), 	tag: ({ let Ok(value) = Tag::new(b"JAM "); value }) },
    LangTag { language: lang(b"jam"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"jan"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"jax"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"jbe"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"jbn"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },



    LangTag { language: lang(b"jgo"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"ji"), 	tag: ({ let Ok(value) = Tag::new(b"JII "); value }) },
    LangTag { language: lang(b"jii"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"jkm"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"jkp"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"jud"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"jul"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"jv"), 	tag: ({ let Ok(value) = Tag::new(b"JAV "); value }) },
    LangTag { language: lang(b"jvd"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"jw"), 	tag: ({ let Ok(value) = Tag::new(b"JAV "); value }) },
    LangTag { language: lang(b"ka"), 	tag: ({ let Ok(value) = Tag::new(b"KAT "); value }) },
    LangTag { language: lang(b"kaa"), 	tag: ({ let Ok(value) = Tag::new(b"KRK "); value }) },
    LangTag { language: lang(b"kab"), 	tag: ({ let Ok(value) = Tag::new(b"KAB0"); value }) },
    LangTag { language: lang(b"kab"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"kac"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kam"), 	tag: ({ let Ok(value) = Tag::new(b"KMB "); value }) },
    LangTag { language: lang(b"kar"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },


    LangTag { language: lang(b"kbd"), 	tag: ({ let Ok(value) = Tag::new(b"KAB "); value }) },
    LangTag { language: lang(b"kby"), 	tag: ({ let Ok(value) = Tag::new(b"KNR "); value }) },
    LangTag { language: lang(b"kca"), 	tag: ({ let Ok(value) = Tag::new(b"KHK "); value }) },
    LangTag { language: lang(b"kca"), 	tag: ({ let Ok(value) = Tag::new(b"KHS "); value }) },
    LangTag { language: lang(b"kca"), 	tag: ({ let Ok(value) = Tag::new(b"KHV "); value }) },
    LangTag { language: lang(b"kcn"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"kdr"), 	tag: ({ let Ok(value) = Tag::new(b"KRM "); value }) },
    LangTag { language: lang(b"kdt"), 	tag: ({ let Ok(value) = Tag::new(b"KUY "); value }) },
    LangTag { language: lang(b"kea"), 	tag: ({ let Ok(value) = Tag::new(b"KEA "); value }) },
    LangTag { language: lang(b"kea"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"keb"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kek"), 	tag: ({ let Ok(value) = Tag::new(b"KEK "); value }) },
    LangTag { language: lang(b"kek"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"kex"), 	tag: ({ let Ok(value) = Tag::new(b"KKN "); value }) },
    LangTag { language: lang(b"kfa"), 	tag: ({ let Ok(value) = Tag::new(b"KOD "); value }) },
    LangTag { language: lang(b"kfr"), 	tag: ({ let Ok(value) = Tag::new(b"KAC "); value }) },
    LangTag { language: lang(b"kfx"), 	tag: ({ let Ok(value) = Tag::new(b"KUL "); value }) },
    LangTag { language: lang(b"kfy"), 	tag: ({ let Ok(value) = Tag::new(b"KMN "); value }) },
    LangTag { language: lang(b"kg"), 	tag: ({ let Ok(value) = Tag::new(b"KON0"); value }) },
    LangTag { language: lang(b"kge"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"kha"), 	tag: ({ let Ok(value) = Tag::new(b"KSI "); value }) },
    LangTag { language: lang(b"khb"), 	tag: ({ let Ok(value) = Tag::new(b"XBD "); value }) },
    LangTag { language: lang(b"khk"), 	tag: ({ let Ok(value) = Tag::new(b"MNG "); value }) },
    LangTag { language: lang(b"khn"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"khs"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kht"), 	tag: ({ let Ok(value) = Tag::new(b"KHT "); value }) },
    LangTag { language: lang(b"kht"), 	tag: ({ let Ok(value) = Tag::new(b"KHN "); value }) },
    LangTag { language: lang(b"khv"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"ki"), 	tag: ({ let Ok(value) = Tag::new(b"KIK "); value }) },
    LangTag { language: lang(b"kis"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kiu"), 	tag: ({ let Ok(value) = Tag::new(b"KIU "); value }) },
    LangTag { language: lang(b"kiu"), 	tag: ({ let Ok(value) = Tag::new(b"ZZA "); value }) },
    LangTag { language: lang(b"kj"), 	tag: ({ let Ok(value) = Tag::new(b"KUA "); value }) },
    LangTag { language: lang(b"kjb"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },

    LangTag { language: lang(b"kjh"), 	tag: ({ let Ok(value) = Tag::new(b"KHA "); value }) },

    LangTag { language: lang(b"kjp"), 	tag: ({ let Ok(value) = Tag::new(b"KJP "); value }) },
    LangTag { language: lang(b"kjp"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"kjt"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },

    LangTag { language: lang(b"kk"), 	tag: ({ let Ok(value) = Tag::new(b"KAZ "); value }) },
    LangTag { language: lang(b"kkn"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kkz"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"kl"), 	tag: ({ let Ok(value) = Tag::new(b"GRN "); value }) },
    LangTag { language: lang(b"klm"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kln"), 	tag: ({ let Ok(value) = Tag::new(b"KAL "); value }) },
    LangTag { language: lang(b"km"), 	tag: ({ let Ok(value) = Tag::new(b"KHM "); value }) },
    LangTag { language: lang(b"kmb"), 	tag: ({ let Ok(value) = Tag::new(b"MBN "); value }) },

    LangTag { language: lang(b"kmn"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kmo"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kmr"), 	tag: ({ let Ok(value) = Tag::new(b"KUR "); value }) },
    LangTag { language: lang(b"kms"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kmv"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"kmw"), 	tag: ({ let Ok(value) = Tag::new(b"KMO "); value }) },

    LangTag { language: lang(b"kn"), 	tag: ({ let Ok(value) = Tag::new(b"KAN "); value }) },
    LangTag { language: lang(b"knc"), 	tag: ({ let Ok(value) = Tag::new(b"KNR "); value }) },
    LangTag { language: lang(b"kng"), 	tag: ({ let Ok(value) = Tag::new(b"KON0"); value }) },
    LangTag { language: lang(b"knj"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"knn"), 	tag: ({ let Ok(value) = Tag::new(b"KOK "); value }) },
    LangTag { language: lang(b"knr"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ko"), 	tag: ({ let Ok(value) = Tag::new(b"KOR "); value }) },
    LangTag { language: lang(b"ko"), 	tag: ({ let Ok(value) = Tag::new(b"KOH "); value }) },
    LangTag { language: lang(b"kod"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"koh"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"koi"), 	tag: ({ let Ok(value) = Tag::new(b"KOP "); value }) },
    LangTag { language: lang(b"koi"), 	tag: ({ let Ok(value) = Tag::new(b"KOM "); value }) },

    LangTag { language: lang(b"kop"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"koy"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"koz"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kpe"), 	tag: ({ let Ok(value) = Tag::new(b"KPL "); value }) },
    LangTag { language: lang(b"kpl"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kpp"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"kpv"), 	tag: ({ let Ok(value) = Tag::new(b"KOZ "); value }) },
    LangTag { language: lang(b"kpv"), 	tag: ({ let Ok(value) = Tag::new(b"KOM "); value }) },
    LangTag { language: lang(b"kpy"), 	tag: ({ let Ok(value) = Tag::new(b"KYK "); value }) },
    LangTag { language: lang(b"kqs"), 	tag: ({ let Ok(value) = Tag::new(b"KIS "); value }) },
    LangTag { language: lang(b"kqy"), 	tag: ({ let Ok(value) = Tag::new(b"KRT "); value }) },
    LangTag { language: lang(b"kr"), 	tag: ({ let Ok(value) = Tag::new(b"KNR "); value }) },
    LangTag { language: lang(b"krc"), 	tag: ({ let Ok(value) = Tag::new(b"KAR "); value }) },
    LangTag { language: lang(b"krc"), 	tag: ({ let Ok(value) = Tag::new(b"BAL "); value }) },
    LangTag { language: lang(b"kri"), 	tag: ({ let Ok(value) = Tag::new(b"KRI "); value }) },
    LangTag { language: lang(b"kri"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"krk"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"krm"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"krn"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"krt"), 	tag: ({ let Ok(value) = Tag::new(b"KNR "); value }) },
    LangTag { language: lang(b"kru"), 	tag: ({ let Ok(value) = Tag::new(b"KUU "); value }) },
    LangTag { language: lang(b"ks"), 	tag: ({ let Ok(value) = Tag::new(b"KSH "); value }) },
    LangTag { language: lang(b"ksh"), 	tag: ({ let Ok(value) = Tag::new(b"KSH0"); value }) },
    LangTag { language: lang(b"ksi"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ksm"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kss"), 	tag: ({ let Ok(value) = Tag::new(b"KIS "); value }) },

    LangTag { language: lang(b"ksw"), 	tag: ({ let Ok(value) = Tag::new(b"KSW "); value }) },
    LangTag { language: lang(b"ksw"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"ktb"), 	tag: ({ let Ok(value) = Tag::new(b"KEB "); value }) },
    LangTag { language: lang(b"ktu"), 	tag: ({ let Ok(value) = Tag::new(b"KON "); value }) },
    LangTag { language: lang(b"ktw"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"ku"), 	tag: ({ let Ok(value) = Tag::new(b"KUR "); value }) },
    LangTag { language: lang(b"kui"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kul"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"kuu"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"kuw"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"kuy"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kv"), 	tag: ({ let Ok(value) = Tag::new(b"KOM "); value }) },
    LangTag { language: lang(b"kvb"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"kvl"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"kvq"), 	tag: ({ let Ok(value) = Tag::new(b"KVQ "); value }) },
    LangTag { language: lang(b"kvq"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"kvr"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"kvt"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"kvu"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"kvy"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"kw"), 	tag: ({ let Ok(value) = Tag::new(b"COR "); value }) },

    LangTag { language: lang(b"kww"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"kwy"), 	tag: ({ let Ok(value) = Tag::new(b"KON0"); value }) },
    LangTag { language: lang(b"kxc"), 	tag: ({ let Ok(value) = Tag::new(b"KMS "); value }) },
    LangTag { language: lang(b"kxd"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"kxf"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"kxk"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"kxl"), 	tag: ({ let Ok(value) = Tag::new(b"KUU "); value }) },
    LangTag { language: lang(b"kxu"), 	tag: ({ let Ok(value) = Tag::new(b"KUI "); value }) },
    LangTag { language: lang(b"ky"), 	tag: ({ let Ok(value) = Tag::new(b"KIR "); value }) },
    LangTag { language: lang(b"kyk"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"kyu"), 	tag: ({ let Ok(value) = Tag::new(b"KYU "); value }) },
    LangTag { language: lang(b"kyu"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"la"), 	tag: ({ let Ok(value) = Tag::new(b"LAT "); value }) },
    LangTag { language: lang(b"lac"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"lad"), 	tag: ({ let Ok(value) = Tag::new(b"JUD "); value }) },
    LangTag { language: lang(b"lah"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"lak"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"lam"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"laz"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"lb"), 	tag: ({ let Ok(value) = Tag::new(b"LTZ "); value }) },
    LangTag { language: lang(b"lbe"), 	tag: ({ let Ok(value) = Tag::new(b"LAK "); value }) },
    LangTag { language: lang(b"lbj"), 	tag: ({ let Ok(value) = Tag::new(b"LDK "); value }) },
    LangTag { language: lang(b"lbl"), 	tag: ({ let Ok(value) = Tag::new(b"BIK "); value }) },
    LangTag { language: lang(b"lce"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"lcf"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"ldi"), 	tag: ({ let Ok(value) = Tag::new(b"KON0"); value }) },
    LangTag { language: lang(b"ldk"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },


    LangTag { language: lang(b"lg"), 	tag: ({ let Ok(value) = Tag::new(b"LUG "); value }) },
    LangTag { language: lang(b"li"), 	tag: ({ let Ok(value) = Tag::new(b"LIM "); value }) },
    LangTag { language: lang(b"lif"), 	tag: ({ let Ok(value) = Tag::new(b"LMB "); value }) },

    LangTag { language: lang(b"lir"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },


    LangTag { language: lang(b"liw"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"liy"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },

    LangTag { language: lang(b"lkb"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },

    LangTag { language: lang(b"lko"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"lks"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"lld"), 	tag: ({ let Ok(value) = Tag::new(b"LAD "); value }) },
    LangTag { language: lang(b"lma"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"lmb"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"lmn"), 	tag: ({ let Ok(value) = Tag::new(b"LAM "); value }) },

    LangTag { language: lang(b"lmw"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ln"), 	tag: ({ let Ok(value) = Tag::new(b"LIN "); value }) },
    LangTag { language: lang(b"lna"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"lnl"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"lo"), 	tag: ({ let Ok(value) = Tag::new(b"LAO "); value }) },

    LangTag { language: lang(b"lou"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },


    LangTag { language: lang(b"lri"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"lrm"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"lrt"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"lsb"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"lsm"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"lt"), 	tag: ({ let Ok(value) = Tag::new(b"LTH "); value }) },
    LangTag { language: lang(b"ltg"), 	tag: ({ let Ok(value) = Tag::new(b"LVI "); value }) },
    LangTag { language: lang(b"lth"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"lto"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"lts"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"lu"), 	tag: ({ let Ok(value) = Tag::new(b"LUB "); value }) },

    LangTag { language: lang(b"luh"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },

    LangTag { language: lang(b"lus"), 	tag: ({ let Ok(value) = Tag::new(b"MIZ "); value }) },
    LangTag { language: lang(b"lus"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },

    LangTag { language: lang(b"luy"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"luz"), 	tag: ({ let Ok(value) = Tag::new(b"LRC "); value }) },
    LangTag { language: lang(b"lv"), 	tag: ({ let Ok(value) = Tag::new(b"LVI "); value }) },
    LangTag { language: lang(b"lvi"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"lvs"), 	tag: ({ let Ok(value) = Tag::new(b"LVI "); value }) },
    LangTag { language: lang(b"lwg"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"lzh"), 	tag: ({ let Ok(value) = Tag::new(b"ZHT "); value }) },
    LangTag { language: lang(b"lzz"), 	tag: ({ let Ok(value) = Tag::new(b"LAZ "); value }) },


    LangTag { language: lang(b"mai"), 	tag: ({ let Ok(value) = Tag::new(b"MTH "); value }) },
    LangTag { language: lang(b"maj"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mak"), 	tag: ({ let Ok(value) = Tag::new(b"MKR "); value }) },
    LangTag { language: lang(b"mam"), 	tag: ({ let Ok(value) = Tag::new(b"MAM "); value }) },
    LangTag { language: lang(b"mam"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"man"), 	tag: ({ let Ok(value) = Tag::new(b"MNK "); value }) },
    LangTag { language: lang(b"map"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"maw"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"max"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"max"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"mbf"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"mbn"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"mch"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mcm"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"mcr"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mct"), 	tag: ({ let Ok(value) = Tag::new(b"BTI "); value }) },
    LangTag { language: lang(b"mde"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mdf"), 	tag: ({ let Ok(value) = Tag::new(b"MOK "); value }) },

    LangTag { language: lang(b"mdy"), 	tag: ({ let Ok(value) = Tag::new(b"MLE "); value }) },
    LangTag { language: lang(b"men"), 	tag: ({ let Ok(value) = Tag::new(b"MDE "); value }) },
    LangTag { language: lang(b"meo"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },


    LangTag { language: lang(b"mfa"), 	tag: ({ let Ok(value) = Tag::new(b"MFA "); value }) },
    LangTag { language: lang(b"mfa"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"mfb"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"mfe"), 	tag: ({ let Ok(value) = Tag::new(b"MFE "); value }) },
    LangTag { language: lang(b"mfe"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"mfp"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"mg"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"mga"), 	tag: ({ let Ok(value) = Tag::new(b"SGA "); value }) },
    LangTag { language: lang(b"mh"), 	tag: ({ let Ok(value) = Tag::new(b"MAH "); value }) },
    LangTag { language: lang(b"mhc"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"mhr"), 	tag: ({ let Ok(value) = Tag::new(b"LMA "); value }) },
    LangTag { language: lang(b"mhv"), 	tag: ({ let Ok(value) = Tag::new(b"ARK "); value }) },
    LangTag { language: lang(b"mi"), 	tag: ({ let Ok(value) = Tag::new(b"MRI "); value }) },
    LangTag { language: lang(b"min"), 	tag: ({ let Ok(value) = Tag::new(b"MIN "); value }) },
    LangTag { language: lang(b"min"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"miz"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mk"), 	tag: ({ let Ok(value) = Tag::new(b"MKD "); value }) },
    LangTag { language: lang(b"mkn"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"mkr"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mku"), 	tag: ({ let Ok(value) = Tag::new(b"MNK "); value }) },

    LangTag { language: lang(b"ml"), 	tag: ({ let Ok(value) = Tag::new(b"MAL "); value }) },
    LangTag { language: lang(b"ml"), 	tag: ({ let Ok(value) = Tag::new(b"MLR "); value }) },
    LangTag { language: lang(b"mle"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mln"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mlq"), 	tag: ({ let Ok(value) = Tag::new(b"MLN "); value }) },
    LangTag { language: lang(b"mlq"), 	tag: ({ let Ok(value) = Tag::new(b"MNK "); value }) },
    LangTag { language: lang(b"mlr"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mmr"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"mn"), 	tag: ({ let Ok(value) = Tag::new(b"MNG "); value }) },
    LangTag { language: lang(b"mnc"), 	tag: ({ let Ok(value) = Tag::new(b"MCH "); value }) },
    LangTag { language: lang(b"mnd"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mng"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mnh"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },

    LangTag { language: lang(b"mnk"), 	tag: ({ let Ok(value) = Tag::new(b"MND "); value }) },
    LangTag { language: lang(b"mnk"), 	tag: ({ let Ok(value) = Tag::new(b"MNK "); value }) },
    LangTag { language: lang(b"mnp"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"mns"), 	tag: ({ let Ok(value) = Tag::new(b"MAN "); value }) },
    LangTag { language: lang(b"mnw"), 	tag: ({ let Ok(value) = Tag::new(b"MON "); value }) },
    LangTag { language: lang(b"mnw"), 	tag: ({ let Ok(value) = Tag::new(b"MONT"); value }) },
    LangTag { language: lang(b"mnx"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mo"), 	tag: ({ let Ok(value) = Tag::new(b"MOL "); value }) },
    LangTag { language: lang(b"mo"), 	tag: ({ let Ok(value) = Tag::new(b"ROM "); value }) },
    LangTag { language: lang(b"mod"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"mok"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mop"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"mor"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"mpe"), 	tag: ({ let Ok(value) = Tag::new(b"MAJ "); value }) },
    LangTag { language: lang(b"mqg"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"mr"), 	tag: ({ let Ok(value) = Tag::new(b"MAR "); value }) },
    LangTag { language: lang(b"mrh"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"mrj"), 	tag: ({ let Ok(value) = Tag::new(b"HMA "); value }) },
    LangTag { language: lang(b"ms"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"msc"), 	tag: ({ let Ok(value) = Tag::new(b"MNK "); value }) },
    LangTag { language: lang(b"msh"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"msi"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"msi"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"mt"), 	tag: ({ let Ok(value) = Tag::new(b"MTS "); value }) },
    LangTag { language: lang(b"mth"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mtr"), 	tag: ({ let Ok(value) = Tag::new(b"MAW "); value }) },
    LangTag { language: lang(b"mts"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mud"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"mui"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"mun"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"mup"), 	tag: ({ let Ok(value) = Tag::new(b"RAJ "); value }) },
    LangTag { language: lang(b"muq"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },

    LangTag { language: lang(b"mvb"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"mve"), 	tag: ({ let Ok(value) = Tag::new(b"MAW "); value }) },
    LangTag { language: lang(b"mvf"), 	tag: ({ let Ok(value) = Tag::new(b"MNG "); value }) },
    LangTag { language: lang(b"mwk"), 	tag: ({ let Ok(value) = Tag::new(b"MNK "); value }) },

    LangTag { language: lang(b"mwq"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"mwr"), 	tag: ({ let Ok(value) = Tag::new(b"MAW "); value }) },
    LangTag { language: lang(b"mww"), 	tag: ({ let Ok(value) = Tag::new(b"MWW "); value }) },
    LangTag { language: lang(b"mww"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"my"), 	tag: ({ let Ok(value) = Tag::new(b"BRM "); value }) },
    LangTag { language: lang(b"mym"), 	tag: ({ let Ok(value) = Tag::new(b"MEN "); value }) },

    LangTag { language: lang(b"myq"), 	tag: ({ let Ok(value) = Tag::new(b"MNK "); value }) },
    LangTag { language: lang(b"myv"), 	tag: ({ let Ok(value) = Tag::new(b"ERZ "); value }) },
    LangTag { language: lang(b"mzb"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },

    LangTag { language: lang(b"mzs"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"na"), 	tag: ({ let Ok(value) = Tag::new(b"NAU "); value }) },
    LangTag { language: lang(b"nag"), 	tag: ({ let Ok(value) = Tag::new(b"NAG "); value }) },
    LangTag { language: lang(b"nag"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"nan"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },

    LangTag { language: lang(b"nas"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"naz"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nb"), 	tag: ({ let Ok(value) = Tag::new(b"NOR "); value }) },
    LangTag { language: lang(b"nch"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nci"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"ncj"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"ncl"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"ncr"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ncx"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nd"), 	tag: ({ let Ok(value) = Tag::new(b"NDB "); value }) },
    LangTag { language: lang(b"ndb"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"ndg"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"ne"), 	tag: ({ let Ok(value) = Tag::new(b"NEP "); value }) },
    LangTag { language: lang(b"nef"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"ng"), 	tag: ({ let Ok(value) = Tag::new(b"NDG "); value }) },

    LangTag { language: lang(b"ngl"), 	tag: ({ let Ok(value) = Tag::new(b"LMW "); value }) },
    LangTag { language: lang(b"ngm"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"ngo"), 	tag: ({ let Ok(value) = Tag::new(b"SXT "); value }) },
    LangTag { language: lang(b"ngr"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ngu"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhc"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhd"), 	tag: ({ let Ok(value) = Tag::new(b"GUA "); value }) },
    LangTag { language: lang(b"nhe"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhg"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhi"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhk"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhm"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhn"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhp"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhq"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nht"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhv"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhw"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhx"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhy"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nhz"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"niq"), 	tag: ({ let Ok(value) = Tag::new(b"KAL "); value }) },
    LangTag { language: lang(b"nis"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"niv"), 	tag: ({ let Ok(value) = Tag::new(b"GIL "); value }) },
    LangTag { language: lang(b"njt"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"njz"), 	tag: ({ let Ok(value) = Tag::new(b"NIS "); value }) },
    LangTag { language: lang(b"nko"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"nkx"), 	tag: ({ let Ok(value) = Tag::new(b"IJO "); value }) },
    LangTag { language: lang(b"nl"), 	tag: ({ let Ok(value) = Tag::new(b"NLD "); value }) },
    LangTag { language: lang(b"nla"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"nle"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },
    LangTag { language: lang(b"nln"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nlv"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nn"), 	tag: ({ let Ok(value) = Tag::new(b"NYN "); value }) },
    LangTag { language: lang(b"nnh"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"nnz"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"no"), 	tag: ({ let Ok(value) = Tag::new(b"NOR "); value }) },
    LangTag { language: lang(b"nod"), 	tag: ({ let Ok(value) = Tag::new(b"NTA "); value }) },




    LangTag { language: lang(b"npi"), 	tag: ({ let Ok(value) = Tag::new(b"NEP "); value }) },
    LangTag { language: lang(b"npl"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nqo"), 	tag: ({ let Ok(value) = Tag::new(b"NKO "); value }) },
    LangTag { language: lang(b"nr"), 	tag: ({ let Ok(value) = Tag::new(b"NDB "); value }) },
    LangTag { language: lang(b"nsk"), 	tag: ({ let Ok(value) = Tag::new(b"NAS "); value }) },
    LangTag { language: lang(b"nsm"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"nsu"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nto"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"nue"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },

    LangTag { language: lang(b"nuu"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"nuz"), 	tag: ({ let Ok(value) = Tag::new(b"NAH "); value }) },
    LangTag { language: lang(b"nv"), 	tag: ({ let Ok(value) = Tag::new(b"NAV "); value }) },
    LangTag { language: lang(b"nv"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"nwe"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"ny"), 	tag: ({ let Ok(value) = Tag::new(b"CHI "); value }) },
    LangTag { language: lang(b"nyd"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },

    LangTag { language: lang(b"nyn"), 	tag: ({ let Ok(value) = Tag::new(b"NKL "); value }) },

    LangTag { language: lang(b"oc"), 	tag: ({ let Ok(value) = Tag::new(b"OCI "); value }) },
    LangTag { language: lang(b"oj"), 	tag: ({ let Ok(value) = Tag::new(b"OJB "); value }) },

    LangTag { language: lang(b"ojc"), 	tag: ({ let Ok(value) = Tag::new(b"OJB "); value }) },
    LangTag { language: lang(b"ojg"), 	tag: ({ let Ok(value) = Tag::new(b"OJB "); value }) },
    LangTag { language: lang(b"ojs"), 	tag: ({ let Ok(value) = Tag::new(b"OCR "); value }) },
    LangTag { language: lang(b"ojs"), 	tag: ({ let Ok(value) = Tag::new(b"OJB "); value }) },
    LangTag { language: lang(b"ojw"), 	tag: ({ let Ok(value) = Tag::new(b"OJB "); value }) },
    LangTag { language: lang(b"okd"), 	tag: ({ let Ok(value) = Tag::new(b"IJO "); value }) },
    LangTag { language: lang(b"oki"), 	tag: ({ let Ok(value) = Tag::new(b"KAL "); value }) },
    LangTag { language: lang(b"okm"), 	tag: ({ let Ok(value) = Tag::new(b"KOH "); value }) },
    LangTag { language: lang(b"okr"), 	tag: ({ let Ok(value) = Tag::new(b"IJO "); value }) },
    LangTag { language: lang(b"om"), 	tag: ({ let Ok(value) = Tag::new(b"ORO "); value }) },


    LangTag { language: lang(b"onx"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"oor"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"or"), 	tag: ({ let Ok(value) = Tag::new(b"ORI "); value }) },
    LangTag { language: lang(b"orc"), 	tag: ({ let Ok(value) = Tag::new(b"ORO "); value }) },
    LangTag { language: lang(b"orn"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"oro"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"orr"), 	tag: ({ let Ok(value) = Tag::new(b"IJO "); value }) },
    LangTag { language: lang(b"ors"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"ory"), 	tag: ({ let Ok(value) = Tag::new(b"ORI "); value }) },
    LangTag { language: lang(b"os"), 	tag: ({ let Ok(value) = Tag::new(b"OSS "); value }) },
    LangTag { language: lang(b"otw"), 	tag: ({ let Ok(value) = Tag::new(b"OJB "); value }) },
    LangTag { language: lang(b"oua"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"pa"), 	tag: ({ let Ok(value) = Tag::new(b"PAN "); value }) },
    LangTag { language: lang(b"paa"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"pal"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"pap"), 	tag: ({ let Ok(value) = Tag::new(b"PAP0"); value }) },
    LangTag { language: lang(b"pap"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"pas"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"pbt"), 	tag: ({ let Ok(value) = Tag::new(b"PAS "); value }) },
    LangTag { language: lang(b"pbu"), 	tag: ({ let Ok(value) = Tag::new(b"PAS "); value }) },


    LangTag { language: lang(b"pce"), 	tag: ({ let Ok(value) = Tag::new(b"PLG "); value }) },
    LangTag { language: lang(b"pck"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"pcm"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"pdu"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"pea"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"pel"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"pes"), 	tag: ({ let Ok(value) = Tag::new(b"FAR "); value }) },
    LangTag { language: lang(b"pey"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"pga"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"pga"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"pi"), 	tag: ({ let Ok(value) = Tag::new(b"PAL "); value }) },
    LangTag { language: lang(b"pih"), 	tag: ({ let Ok(value) = Tag::new(b"PIH "); value }) },
    LangTag { language: lang(b"pih"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"pil"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"pis"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"pkh"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"pko"), 	tag: ({ let Ok(value) = Tag::new(b"KAL "); value }) },
    LangTag { language: lang(b"pl"), 	tag: ({ let Ok(value) = Tag::new(b"PLK "); value }) },
    LangTag { language: lang(b"plg"), 	tag: ({ let Ok(value) = Tag::new(b"PLG0"); value }) },
    LangTag { language: lang(b"plk"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"pll"), 	tag: ({ let Ok(value) = Tag::new(b"PLG "); value }) },
    LangTag { language: lang(b"pln"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"plp"), 	tag: ({ let Ok(value) = Tag::new(b"PAP "); value }) },
    LangTag { language: lang(b"plt"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"pml"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"pmy"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"poc"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"poh"), 	tag: ({ let Ok(value) = Tag::new(b"POH "); value }) },
    LangTag { language: lang(b"poh"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },

    LangTag { language: lang(b"pov"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"ppa"), 	tag: ({ let Ok(value) = Tag::new(b"BAG "); value }) },
    LangTag { language: lang(b"pre"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"prp"), 	tag: ({ let Ok(value) = Tag::new(b"GUJ "); value }) },
    LangTag { language: lang(b"prs"), 	tag: ({ let Ok(value) = Tag::new(b"DRI "); value }) },
    LangTag { language: lang(b"prs"), 	tag: ({ let Ok(value) = Tag::new(b"FAR "); value }) },
    LangTag { language: lang(b"ps"), 	tag: ({ let Ok(value) = Tag::new(b"PAS "); value }) },
    LangTag { language: lang(b"pse"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"pst"), 	tag: ({ let Ok(value) = Tag::new(b"PAS "); value }) },
    LangTag { language: lang(b"pt"), 	tag: ({ let Ok(value) = Tag::new(b"PTG "); value }) },
    LangTag { language: lang(b"pub"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"puz"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"pwo"), 	tag: ({ let Ok(value) = Tag::new(b"PWO "); value }) },
    LangTag { language: lang(b"pwo"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"pww"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"qu"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qub"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qub"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"quc"), 	tag: ({ let Ok(value) = Tag::new(b"QUC "); value }) },
    LangTag { language: lang(b"quc"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"qud"), 	tag: ({ let Ok(value) = Tag::new(b"QVI "); value }) },
    LangTag { language: lang(b"qud"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"quf"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qug"), 	tag: ({ let Ok(value) = Tag::new(b"QVI "); value }) },
    LangTag { language: lang(b"qug"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"quh"), 	tag: ({ let Ok(value) = Tag::new(b"QUH "); value }) },
    LangTag { language: lang(b"quh"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"quk"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qul"), 	tag: ({ let Ok(value) = Tag::new(b"QUH "); value }) },
    LangTag { language: lang(b"qul"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qum"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"qup"), 	tag: ({ let Ok(value) = Tag::new(b"QVI "); value }) },
    LangTag { language: lang(b"qup"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qur"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qur"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qus"), 	tag: ({ let Ok(value) = Tag::new(b"QUH "); value }) },
    LangTag { language: lang(b"qus"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"quv"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"quw"), 	tag: ({ let Ok(value) = Tag::new(b"QVI "); value }) },
    LangTag { language: lang(b"quw"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qux"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qux"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"quy"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },

    LangTag { language: lang(b"qva"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qva"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvc"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qve"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvh"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qvh"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvi"), 	tag: ({ let Ok(value) = Tag::new(b"QVI "); value }) },
    LangTag { language: lang(b"qvi"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvj"), 	tag: ({ let Ok(value) = Tag::new(b"QVI "); value }) },
    LangTag { language: lang(b"qvj"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvl"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qvl"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvm"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qvm"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvn"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qvn"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvo"), 	tag: ({ let Ok(value) = Tag::new(b"QVI "); value }) },
    LangTag { language: lang(b"qvo"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvp"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qvp"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvs"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvw"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qvw"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qvz"), 	tag: ({ let Ok(value) = Tag::new(b"QVI "); value }) },
    LangTag { language: lang(b"qvz"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qwa"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qwa"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qwc"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qwh"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qwh"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qws"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qws"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qwt"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"qxa"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qxa"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qxc"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qxc"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qxh"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qxh"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qxl"), 	tag: ({ let Ok(value) = Tag::new(b"QVI "); value }) },
    LangTag { language: lang(b"qxl"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qxn"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qxn"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qxo"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qxo"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qxp"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qxr"), 	tag: ({ let Ok(value) = Tag::new(b"QVI "); value }) },
    LangTag { language: lang(b"qxr"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qxt"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qxt"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qxu"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"qxw"), 	tag: ({ let Ok(value) = Tag::new(b"QWH "); value }) },
    LangTag { language: lang(b"qxw"), 	tag: ({ let Ok(value) = Tag::new(b"QUZ "); value }) },
    LangTag { language: lang(b"rag"), 	tag: ({ let Ok(value) = Tag::new(b"LUH "); value }) },

    LangTag { language: lang(b"ral"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },

    LangTag { language: lang(b"rbb"), 	tag: ({ let Ok(value) = Tag::new(b"PLG "); value }) },
    LangTag { language: lang(b"rbl"), 	tag: ({ let Ok(value) = Tag::new(b"BIK "); value }) },
    LangTag { language: lang(b"rcf"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },



    LangTag { language: lang(b"rif"), 	tag: ({ let Ok(value) = Tag::new(b"RIF "); value }) },
    LangTag { language: lang(b"rif"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },

    LangTag { language: lang(b"rki"), 	tag: ({ let Ok(value) = Tag::new(b"ARK "); value }) },

    LangTag { language: lang(b"rm"), 	tag: ({ let Ok(value) = Tag::new(b"RMS "); value }) },
    LangTag { language: lang(b"rmc"), 	tag: ({ let Ok(value) = Tag::new(b"ROY "); value }) },
    LangTag { language: lang(b"rmf"), 	tag: ({ let Ok(value) = Tag::new(b"ROY "); value }) },
    LangTag { language: lang(b"rml"), 	tag: ({ let Ok(value) = Tag::new(b"ROY "); value }) },
    LangTag { language: lang(b"rmn"), 	tag: ({ let Ok(value) = Tag::new(b"ROY "); value }) },
    LangTag { language: lang(b"rmo"), 	tag: ({ let Ok(value) = Tag::new(b"ROY "); value }) },
    LangTag { language: lang(b"rms"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"rmw"), 	tag: ({ let Ok(value) = Tag::new(b"ROY "); value }) },
    LangTag { language: lang(b"rmy"), 	tag: ({ let Ok(value) = Tag::new(b"RMY "); value }) },
    LangTag { language: lang(b"rmy"), 	tag: ({ let Ok(value) = Tag::new(b"ROY "); value }) },
    LangTag { language: lang(b"rmz"), 	tag: ({ let Ok(value) = Tag::new(b"ARK "); value }) },
    LangTag { language: lang(b"rn"), 	tag: ({ let Ok(value) = Tag::new(b"RUN "); value }) },
    LangTag { language: lang(b"ro"), 	tag: ({ let Ok(value) = Tag::new(b"ROM "); value }) },
    LangTag { language: lang(b"rom"), 	tag: ({ let Ok(value) = Tag::new(b"ROY "); value }) },
    LangTag { language: lang(b"rop"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"rtc"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },

    LangTag { language: lang(b"ru"), 	tag: ({ let Ok(value) = Tag::new(b"RUS "); value }) },
    LangTag { language: lang(b"rue"), 	tag: ({ let Ok(value) = Tag::new(b"RSY "); value }) },

    LangTag { language: lang(b"rw"), 	tag: ({ let Ok(value) = Tag::new(b"RUA "); value }) },
    LangTag { language: lang(b"rwr"), 	tag: ({ let Ok(value) = Tag::new(b"MAW "); value }) },
    LangTag { language: lang(b"sa"), 	tag: ({ let Ok(value) = Tag::new(b"SAN "); value }) },
    LangTag { language: lang(b"sad"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"sah"), 	tag: ({ let Ok(value) = Tag::new(b"YAK "); value }) },
    LangTag { language: lang(b"sam"), 	tag: ({ let Ok(value) = Tag::new(b"PAA "); value }) },


    LangTag { language: lang(b"say"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"sc"), 	tag: ({ let Ok(value) = Tag::new(b"SRD "); value }) },
    LangTag { language: lang(b"scf"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"sch"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"sci"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"sck"), 	tag: ({ let Ok(value) = Tag::new(b"SAD "); value }) },


    LangTag { language: lang(b"scs"), 	tag: ({ let Ok(value) = Tag::new(b"SCS "); value }) },
    LangTag { language: lang(b"scs"), 	tag: ({ let Ok(value) = Tag::new(b"SLA "); value }) },
    LangTag { language: lang(b"scs"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"sd"), 	tag: ({ let Ok(value) = Tag::new(b"SND "); value }) },
    LangTag { language: lang(b"sdc"), 	tag: ({ let Ok(value) = Tag::new(b"SRD "); value }) },
    LangTag { language: lang(b"sdh"), 	tag: ({ let Ok(value) = Tag::new(b"KUR "); value }) },
    LangTag { language: lang(b"sdn"), 	tag: ({ let Ok(value) = Tag::new(b"SRD "); value }) },
    LangTag { language: lang(b"sds"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"se"), 	tag: ({ let Ok(value) = Tag::new(b"NSM "); value }) },

    LangTag { language: lang(b"seh"), 	tag: ({ let Ok(value) = Tag::new(b"SNA "); value }) },
    LangTag { language: lang(b"sek"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },

    LangTag { language: lang(b"sez"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"sfm"), 	tag: ({ let Ok(value) = Tag::new(b"SFM "); value }) },
    LangTag { language: lang(b"sfm"), 	tag: ({ let Ok(value) = Tag::new(b"HMN "); value }) },
    LangTag { language: lang(b"sg"), 	tag: ({ let Ok(value) = Tag::new(b"SGO "); value }) },

    LangTag { language: lang(b"sgc"), 	tag: ({ let Ok(value) = Tag::new(b"KAL "); value }) },
    LangTag { language: lang(b"sgo"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"sgw"), 	tag: ({ let Ok(value) = Tag::new(b"CHG "); value }) },
    LangTag { language: lang(b"sh"), 	tag: ({ let Ok(value) = Tag::new(b"BOS "); value }) },
    LangTag { language: lang(b"sh"), 	tag: ({ let Ok(value) = Tag::new(b"HRV "); value }) },
    LangTag { language: lang(b"sh"), 	tag: ({ let Ok(value) = Tag::new(b"SRB "); value }) },
    LangTag { language: lang(b"shi"), 	tag: ({ let Ok(value) = Tag::new(b"SHI "); value }) },
    LangTag { language: lang(b"shi"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"shl"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },

    LangTag { language: lang(b"shu"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"shy"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"si"), 	tag: ({ let Ok(value) = Tag::new(b"SNH "); value }) },
    LangTag { language: lang(b"sib"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"sig"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"siz"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },

    LangTag { language: lang(b"sjc"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"sjd"), 	tag: ({ let Ok(value) = Tag::new(b"KSM "); value }) },

    LangTag { language: lang(b"sjo"), 	tag: ({ let Ok(value) = Tag::new(b"SIB "); value }) },
    LangTag { language: lang(b"sjs"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },

    LangTag { language: lang(b"sk"), 	tag: ({ let Ok(value) = Tag::new(b"SKY "); value }) },
    LangTag { language: lang(b"skg"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"skr"), 	tag: ({ let Ok(value) = Tag::new(b"SRK "); value }) },
    LangTag { language: lang(b"sks"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"skw"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"sky"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"sl"), 	tag: ({ let Ok(value) = Tag::new(b"SLV "); value }) },
    LangTag { language: lang(b"sla"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"sm"), 	tag: ({ let Ok(value) = Tag::new(b"SMO "); value }) },
    LangTag { language: lang(b"sma"), 	tag: ({ let Ok(value) = Tag::new(b"SSM "); value }) },
    LangTag { language: lang(b"smd"), 	tag: ({ let Ok(value) = Tag::new(b"MBN "); value }) },
    LangTag { language: lang(b"smj"), 	tag: ({ let Ok(value) = Tag::new(b"LSM "); value }) },
    LangTag { language: lang(b"sml"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"smn"), 	tag: ({ let Ok(value) = Tag::new(b"ISM "); value }) },
    LangTag { language: lang(b"sms"), 	tag: ({ let Ok(value) = Tag::new(b"SKS "); value }) },
    LangTag { language: lang(b"smt"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"sn"), 	tag: ({ let Ok(value) = Tag::new(b"SNA0"); value }) },
    LangTag { language: lang(b"snb"), 	tag: ({ let Ok(value) = Tag::new(b"IBA "); value }) },
    LangTag { language: lang(b"snh"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"so"), 	tag: ({ let Ok(value) = Tag::new(b"SML "); value }) },
    LangTag { language: lang(b"sog"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"spv"), 	tag: ({ let Ok(value) = Tag::new(b"ORI "); value }) },
    LangTag { language: lang(b"spy"), 	tag: ({ let Ok(value) = Tag::new(b"KAL "); value }) },
    LangTag { language: lang(b"sq"), 	tag: ({ let Ok(value) = Tag::new(b"SQI "); value }) },
    LangTag { language: lang(b"sr"), 	tag: ({ let Ok(value) = Tag::new(b"SRB "); value }) },
    LangTag { language: lang(b"srb"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"src"), 	tag: ({ let Ok(value) = Tag::new(b"SRD "); value }) },
    LangTag { language: lang(b"srk"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"srm"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"srn"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"sro"), 	tag: ({ let Ok(value) = Tag::new(b"SRD "); value }) },

    LangTag { language: lang(b"srs"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"ss"), 	tag: ({ let Ok(value) = Tag::new(b"SWZ "); value }) },
    LangTag { language: lang(b"ssh"), 	tag: ({ let Ok(value) = Tag::new(b"ARA "); value }) },
    LangTag { language: lang(b"ssl"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ssm"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"st"), 	tag: ({ let Ok(value) = Tag::new(b"SOT "); value }) },
    LangTag { language: lang(b"sta"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },


    LangTag { language: lang(b"stv"), 	tag: ({ let Ok(value) = Tag::new(b"SIG "); value }) },
    LangTag { language: lang(b"su"), 	tag: ({ let Ok(value) = Tag::new(b"SUN "); value }) },

    LangTag { language: lang(b"suq"), 	tag: ({ let Ok(value) = Tag::new(b"SUR "); value }) },
    LangTag { language: lang(b"sur"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"sv"), 	tag: ({ let Ok(value) = Tag::new(b"SVE "); value }) },

    LangTag { language: lang(b"svc"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"sve"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"sw"), 	tag: ({ let Ok(value) = Tag::new(b"SWK "); value }) },
    LangTag { language: lang(b"swb"), 	tag: ({ let Ok(value) = Tag::new(b"CMR "); value }) },
    LangTag { language: lang(b"swc"), 	tag: ({ let Ok(value) = Tag::new(b"SWK "); value }) },
    LangTag { language: lang(b"swh"), 	tag: ({ let Ok(value) = Tag::new(b"SWK "); value }) },
    LangTag { language: lang(b"swk"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"swn"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"swv"), 	tag: ({ let Ok(value) = Tag::new(b"MAW "); value }) },

    LangTag { language: lang(b"syc"), 	tag: ({ let Ok(value) = Tag::new(b"SYR "); value }) },



    LangTag { language: lang(b"ta"), 	tag: ({ let Ok(value) = Tag::new(b"TAM "); value }) },
    LangTag { language: lang(b"taa"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },

    LangTag { language: lang(b"taj"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"taq"), 	tag: ({ let Ok(value) = Tag::new(b"TAQ "); value }) },
    LangTag { language: lang(b"taq"), 	tag: ({ let Ok(value) = Tag::new(b"TMH "); value }) },
    LangTag { language: lang(b"taq"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"tas"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"tau"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },

    LangTag { language: lang(b"tcb"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"tce"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"tch"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"tcp"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"tcs"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"tcy"), 	tag: ({ let Ok(value) = Tag::new(b"TUL "); value }) },
    LangTag { language: lang(b"tcz"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },


    LangTag { language: lang(b"tdx"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"te"), 	tag: ({ let Ok(value) = Tag::new(b"TEL "); value }) },
    LangTag { language: lang(b"tec"), 	tag: ({ let Ok(value) = Tag::new(b"KAL "); value }) },
    LangTag { language: lang(b"tem"), 	tag: ({ let Ok(value) = Tag::new(b"TMN "); value }) },

    LangTag { language: lang(b"tez"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"tfn"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"tg"), 	tag: ({ let Ok(value) = Tag::new(b"TAJ "); value }) },
    LangTag { language: lang(b"tgh"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"tgj"), 	tag: ({ let Ok(value) = Tag::new(b"NIS "); value }) },
    LangTag { language: lang(b"tgn"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"tgr"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"tgx"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"tgy"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"th"), 	tag: ({ let Ok(value) = Tag::new(b"THA "); value }) },

    LangTag { language: lang(b"tht"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"thv"), 	tag: ({ let Ok(value) = Tag::new(b"THV "); value }) },
    LangTag { language: lang(b"thv"), 	tag: ({ let Ok(value) = Tag::new(b"TMH "); value }) },
    LangTag { language: lang(b"thv"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"thz"), 	tag: ({ let Ok(value) = Tag::new(b"THZ "); value }) },
    LangTag { language: lang(b"thz"), 	tag: ({ let Ok(value) = Tag::new(b"TMH "); value }) },
    LangTag { language: lang(b"thz"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"ti"), 	tag: ({ let Ok(value) = Tag::new(b"TGY "); value }) },
    LangTag { language: lang(b"tia"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"tig"), 	tag: ({ let Ok(value) = Tag::new(b"TGR "); value }) },


    LangTag { language: lang(b"tjo"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"tk"), 	tag: ({ let Ok(value) = Tag::new(b"TKM "); value }) },
    LangTag { language: lang(b"tkg"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"tkm"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"tl"), 	tag: ({ let Ok(value) = Tag::new(b"TGL "); value }) },


    LangTag { language: lang(b"tmg"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"tmh"), 	tag: ({ let Ok(value) = Tag::new(b"TMH "); value }) },
    LangTag { language: lang(b"tmh"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"tmn"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"tmw"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"tn"), 	tag: ({ let Ok(value) = Tag::new(b"TNA "); value }) },
    LangTag { language: lang(b"tna"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"tne"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"tnf"), 	tag: ({ let Ok(value) = Tag::new(b"DRI "); value }) },
    LangTag { language: lang(b"tnf"), 	tag: ({ let Ok(value) = Tag::new(b"FAR "); value }) },
    LangTag { language: lang(b"tng"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"to"), 	tag: ({ let Ok(value) = Tag::new(b"TGN "); value }) },
    LangTag { language: lang(b"tod"), 	tag: ({ let Ok(value) = Tag::new(b"TOD0"); value }) },
    LangTag { language: lang(b"toi"), 	tag: ({ let Ok(value) = Tag::new(b"TNG "); value }) },
    LangTag { language: lang(b"toj"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"tol"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"tor"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"tpi"), 	tag: ({ let Ok(value) = Tag::new(b"TPI "); value }) },
    LangTag { language: lang(b"tpi"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"tr"), 	tag: ({ let Ok(value) = Tag::new(b"TRK "); value }) },
    LangTag { language: lang(b"trf"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"trk"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"tru"), 	tag: ({ let Ok(value) = Tag::new(b"TUA "); value }) },
    LangTag { language: lang(b"tru"), 	tag: ({ let Ok(value) = Tag::new(b"SYR "); value }) },
    LangTag { language: lang(b"ts"), 	tag: ({ let Ok(value) = Tag::new(b"TSG "); value }) },
    LangTag { language: lang(b"tsg"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"tt"), 	tag: ({ let Ok(value) = Tag::new(b"TAT "); value }) },
    LangTag { language: lang(b"ttc"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"ttm"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"ttq"), 	tag: ({ let Ok(value) = Tag::new(b"TTQ "); value }) },
    LangTag { language: lang(b"ttq"), 	tag: ({ let Ok(value) = Tag::new(b"TMH "); value }) },
    LangTag { language: lang(b"ttq"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"tua"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"tul"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },


    LangTag { language: lang(b"tuu"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"tuv"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"tuy"), 	tag: ({ let Ok(value) = Tag::new(b"KAL "); value }) },

    LangTag { language: lang(b"tvy"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"tw"), 	tag: ({ let Ok(value) = Tag::new(b"TWI "); value }) },
    LangTag { language: lang(b"tw"), 	tag: ({ let Ok(value) = Tag::new(b"AKA "); value }) },
    LangTag { language: lang(b"txc"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"txy"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"ty"), 	tag: ({ let Ok(value) = Tag::new(b"THT "); value }) },
    LangTag { language: lang(b"tyv"), 	tag: ({ let Ok(value) = Tag::new(b"TUV "); value }) },

    LangTag { language: lang(b"tzh"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"tzj"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"tzm"), 	tag: ({ let Ok(value) = Tag::new(b"TZM "); value }) },
    LangTag { language: lang(b"tzm"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"tzo"), 	tag: ({ let Ok(value) = Tag::new(b"TZO "); value }) },
    LangTag { language: lang(b"tzo"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"ubl"), 	tag: ({ let Ok(value) = Tag::new(b"BIK "); value }) },


    LangTag { language: lang(b"ug"), 	tag: ({ let Ok(value) = Tag::new(b"UYG "); value }) },
    LangTag { language: lang(b"uk"), 	tag: ({ let Ok(value) = Tag::new(b"UKR "); value }) },
    LangTag { language: lang(b"uki"), 	tag: ({ let Ok(value) = Tag::new(b"KUI "); value }) },
    LangTag { language: lang(b"uln"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },

    LangTag { language: lang(b"unr"), 	tag: ({ let Ok(value) = Tag::new(b"MUN "); value }) },
    LangTag { language: lang(b"ur"), 	tag: ({ let Ok(value) = Tag::new(b"URD "); value }) },
    LangTag { language: lang(b"urk"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"usp"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"uz"), 	tag: ({ let Ok(value) = Tag::new(b"UZB "); value }) },
    LangTag { language: lang(b"uzn"), 	tag: ({ let Ok(value) = Tag::new(b"UZB "); value }) },
    LangTag { language: lang(b"uzs"), 	tag: ({ let Ok(value) = Tag::new(b"UZB "); value }) },
    LangTag { language: lang(b"vap"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"ve"), 	tag: ({ let Ok(value) = Tag::new(b"VEN "); value }) },

    LangTag { language: lang(b"vi"), 	tag: ({ let Ok(value) = Tag::new(b"VIT "); value }) },
    LangTag { language: lang(b"vic"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"vit"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"vkk"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"vkp"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"vkt"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"vls"), 	tag: ({ let Ok(value) = Tag::new(b"FLE "); value }) },
    LangTag { language: lang(b"vmw"), 	tag: ({ let Ok(value) = Tag::new(b"MAK "); value }) },
    LangTag { language: lang(b"vo"), 	tag: ({ let Ok(value) = Tag::new(b"VOL "); value }) },
    LangTag { language: lang(b"vro"), 	tag: ({ let Ok(value) = Tag::new(b"VRO "); value }) },
    LangTag { language: lang(b"vro"), 	tag: ({ let Ok(value) = Tag::new(b"ETI "); value }) },
    LangTag { language: lang(b"vsn"), 	tag: ({ let Ok(value) = Tag::new(b"SAN "); value }) },
    LangTag { language: lang(b"wa"), 	tag: ({ let Ok(value) = Tag::new(b"WLN "); value }) },
    LangTag { language: lang(b"wag"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },


    LangTag { language: lang(b"wbm"), 	tag: ({ let Ok(value) = Tag::new(b"WA  "); value }) },
    LangTag { language: lang(b"wbr"), 	tag: ({ let Ok(value) = Tag::new(b"WAG "); value }) },
    LangTag { language: lang(b"wbr"), 	tag: ({ let Ok(value) = Tag::new(b"RAJ "); value }) },


    LangTag { language: lang(b"wea"), 	tag: ({ let Ok(value) = Tag::new(b"KRN "); value }) },
    LangTag { language: lang(b"wes"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"weu"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"wlc"), 	tag: ({ let Ok(value) = Tag::new(b"CMR "); value }) },
    LangTag { language: lang(b"wle"), 	tag: ({ let Ok(value) = Tag::new(b"SIG "); value }) },
    LangTag { language: lang(b"wlk"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"wni"), 	tag: ({ let Ok(value) = Tag::new(b"CMR "); value }) },
    LangTag { language: lang(b"wo"), 	tag: ({ let Ok(value) = Tag::new(b"WLF "); value }) },
    LangTag { language: lang(b"wry"), 	tag: ({ let Ok(value) = Tag::new(b"MAW "); value }) },
    LangTag { language: lang(b"wsg"), 	tag: ({ let Ok(value) = Tag::new(b"GON "); value }) },

    LangTag { language: lang(b"wuu"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"wya"), 	tag: ({ let Ok(value) = Tag::new(b"WDT "); value }) },
    LangTag { language: lang(b"wya"), 	tag: ({ let Ok(value) = Tag::new(b"WYN "); value }) },

    LangTag { language: lang(b"xal"), 	tag: ({ let Ok(value) = Tag::new(b"KLM "); value }) },
    LangTag { language: lang(b"xal"), 	tag: ({ let Ok(value) = Tag::new(b"TOD "); value }) },
    LangTag { language: lang(b"xan"), 	tag: ({ let Ok(value) = Tag::new(b"SEK "); value }) },
    LangTag { language: lang(b"xbd"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"xh"), 	tag: ({ let Ok(value) = Tag::new(b"XHS "); value }) },


    LangTag { language: lang(b"xmg"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"xmm"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"xmm"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"xmv"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"xmw"), 	tag: ({ let Ok(value) = Tag::new(b"MLG "); value }) },
    LangTag { language: lang(b"xnj"), 	tag: ({ let Ok(value) = Tag::new(b"SXT "); value }) },
    LangTag { language: lang(b"xnq"), 	tag: ({ let Ok(value) = Tag::new(b"SXT "); value }) },
    LangTag { language: lang(b"xnr"), 	tag: ({ let Ok(value) = Tag::new(b"DGR "); value }) },

    LangTag { language: lang(b"xpe"), 	tag: ({ let Ok(value) = Tag::new(b"XPE "); value }) },
    LangTag { language: lang(b"xpe"), 	tag: ({ let Ok(value) = Tag::new(b"KPL "); value }) },
    LangTag { language: lang(b"xsl"), 	tag: ({ let Ok(value) = Tag::new(b"SSL "); value }) },
    LangTag { language: lang(b"xsl"), 	tag: ({ let Ok(value) = Tag::new(b"SLA "); value }) },
    LangTag { language: lang(b"xsl"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"xst"), 	tag: ({ let Ok(value) = Tag::new(b"SIG "); value }) },


    LangTag { language: lang(b"xup"), 	tag: ({ let Ok(value) = Tag::new(b"ATH "); value }) },
    LangTag { language: lang(b"xwo"), 	tag: ({ let Ok(value) = Tag::new(b"TOD "); value }) },
    LangTag { language: lang(b"yaj"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"yak"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },


    LangTag { language: lang(b"yba"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"ybb"), 	tag: ({ let Ok(value) = Tag::new(b"BML "); value }) },
    LangTag { language: lang(b"ybd"), 	tag: ({ let Ok(value) = Tag::new(b"ARK "); value }) },
    LangTag { language: lang(b"ycr"), 	tag: ({ let Ok(value) = Tag::new(b"CPP "); value }) },
    LangTag { language: lang(b"ydd"), 	tag: ({ let Ok(value) = Tag::new(b"JII "); value }) },

    LangTag { language: lang(b"yi"), 	tag: ({ let Ok(value) = Tag::new(b"JII "); value }) },
    LangTag { language: lang(b"yih"), 	tag: ({ let Ok(value) = Tag::new(b"JII "); value }) },
    LangTag { language: lang(b"yim"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },

    LangTag { language: lang(b"yo"), 	tag: ({ let Ok(value) = Tag::new(b"YBA "); value }) },
    LangTag { language: lang(b"yos"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"yua"), 	tag: ({ let Ok(value) = Tag::new(b"MYN "); value }) },
    LangTag { language: lang(b"yue"), 	tag: ({ let Ok(value) = Tag::new(b"ZHH "); value }) },


    LangTag { language: lang(b"za"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zch"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zdj"), 	tag: ({ let Ok(value) = Tag::new(b"CMR "); value }) },

    LangTag { language: lang(b"zeh"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zen"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"zgb"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zgh"), 	tag: ({ let Ok(value) = Tag::new(b"ZGH "); value }) },
    LangTag { language: lang(b"zgh"), 	tag: ({ let Ok(value) = Tag::new(b"BBR "); value }) },
    LangTag { language: lang(b"zgm"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zgn"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zh"), 	tag: ({ let Ok(value) = Tag::new(b"ZHS "); value }) },
    LangTag { language: lang(b"zhd"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zhn"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zkb"), 	tag: ({ let Ok(value) = Tag::new(b"KHA "); value }) },
    LangTag { language: lang(b"zlj"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zlm"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"zln"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zlq"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zmi"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"zmz"), 	tag: ({ let Ok(value) = Tag::new(b"BAD0"); value }) },
    LangTag { language: lang(b"znd"), 	tag: ({ let Ok(value) = Tag::new(&[0; 4]); value }) },
    LangTag { language: lang(b"zne"), 	tag: ({ let Ok(value) = Tag::new(b"ZND "); value }) },
    LangTag { language: lang(b"zom"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },
    LangTag { language: lang(b"zqe"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zsm"), 	tag: ({ let Ok(value) = Tag::new(b"MLY "); value }) },
    LangTag { language: lang(b"zu"), 	tag: ({ let Ok(value) = Tag::new(b"ZUL "); value }) },
    LangTag { language: lang(b"zum"), 	tag: ({ let Ok(value) = Tag::new(b"LRC "); value }) },
    LangTag { language: lang(b"zyb"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zyg"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zyj"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zyn"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
    LangTag { language: lang(b"zyp"), 	tag: ({ let Ok(value) = Tag::new(b"QIN "); value }) },

    LangTag { language: lang(b"zzj"), 	tag: ({ let Ok(value) = Tag::new(b"ZHA "); value }) },
];

/// Converts a multi-subtag BCP 47 language tag to language tags.
pub fn tags_from_complex_language(language: &str, tags: &mut Vec<Tag>) -> bool {
  match subtag_matches(language, "-fonnapa") {
    true => {
      tags.push({
        let Ok(value) = Tag::new(b"APPH");
        value
      });
      return true;
    }
    false => {}
  }
  match subtag_matches(language, "-polyton") {
    true => {
      tags.push({
        let Ok(value) = Tag::new(b"PGR ");
        value
      });
      return true;
    }
    false => {}
  }
  match subtag_matches(language, "-arevmda") {
    true => {
      tags.push({
        let Ok(value) = Tag::new(b"HYE ");
        value
      });
      return true;
    }
    false => {}
  }
  match subtag_matches(language, "-provenc") {
    true => {
      tags.push({
        let Ok(value) = Tag::new(b"PRO ");
        value
      });
      return true;
    }
    false => {}
  }
  match subtag_matches(language, "-fonipa") {
    true => {
      tags.push({
        let Ok(value) = Tag::new(b"IPPH");
        value
      });
      return true;
    }
    false => {}
  }
  match subtag_matches(language, "-geok") {
    true => {
      tags.push({
        let Ok(value) = Tag::new(b"KGE ");
        value
      });
      return true;
    }
    false => {}
  }
  match subtag_matches(language, "-syre") {
    true => {
      tags.push({
        let Ok(value) = Tag::new(b"SYRE");
        value
      });
      return true;
    }
    false => {}
  }
  match subtag_matches(language, "-syrj") {
    true => {
      tags.push({
        let Ok(value) = Tag::new(b"SYRJ");
        value
      });
      return true;
    }
    false => {}
  }
  match subtag_matches(language, "-syrn") {
    true => {
      tags.push({
        let Ok(value) = Tag::new(b"SYRN");
        value
      });
      return true;
    }
    false => {}
  }
  match language.as_bytes()[0] {
    b'a' => match &language[1..] == "rt-lojban" {
      true => {
        tags.push({
          let Ok(value) = Tag::new(b"JBO ");
          value
        });
        return true;
      }
      false => {}
    },
    b'c' => {
      match lang_matches(&language[1..], "do-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "do-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "jy-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "jy-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "mn-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "mn-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "np-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "np-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "px-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "px-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "sp-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "sp-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "zh-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "zh-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "zo-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "zo-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "do-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "do-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "jy-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "jy-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "mn-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "mn-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "np-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "np-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "px-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "px-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "sp-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "sp-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "zh-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "zh-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "zo-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "zo-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "do-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "do-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "do-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "jy-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "jy-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "jy-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "mn-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "mn-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "mn-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "np-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "np-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "np-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "px-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "px-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "px-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "sp-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "sp-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "sp-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "zh-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "zh-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "zh-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "zo-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "zo-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "zo-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
    }
    b'g' => {
      match lang_matches(&language[1..], "an-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "an-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "an-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "an-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "a-latg") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"IRT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "an-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "an-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "an-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
    }
    b'h' => {
      match lang_matches(&language[1..], "ak-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "ak-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "nm-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "nm-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "sn-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "sn-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "ak-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "ak-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "nm-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "nm-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "sn-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "sn-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "ak-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "ak-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "ak-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "nm-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "nm-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "nm-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "sn-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "sn-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "sn-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
    }
    b'i' => {
      match &language[1..] == "-navajo" {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"NAV ");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ATH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match &language[1..] == "-hak" {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match &language[1..] == "-lux" {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"LTZ ");
            value
          });
          return true;
        }
        false => {}
      }
    }
    b'l' => {
      match lang_matches(&language[1..], "uh-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "uh-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "uh-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "uh-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "zh-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "uh-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "uh-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "uh-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
    }
    b'm' => {
      match lang_matches(&language[1..], "np-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "np-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "np-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "np-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "np-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "np-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "np-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "nw-", 3) && subtag_matches(language, "-th") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"MONT");
            value
          });
          return true;
        }
        false => {}
      }
    }
    b'n' => {
      match lang_matches(&language[1..], "an-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "an-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "an-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "an-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "an-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "an-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "an-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match &language[1..] == "o-bok" {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"NOR ");
            value
          });
          return true;
        }
        false => {}
      }
      match &language[1..] == "o-nyn" {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"NYN ");
            value
          });
          return true;
        }
        false => {}
      }
    }
    b'r' => match strncmp(&language[1..], "o-", 2) && subtag_matches(language, "-md") {
      true => {
        let possible_tags = &[
          ({
            let Ok(value) = Tag::new(b"MOL ");
            value
          }),
          ({
            let Ok(value) = Tag::new(b"ROM ");
            value
          }),
        ];
        tags.extend_from_slice(possible_tags);
        return true;
      }
      false => {}
    },
    b's' => {
      match lang_matches(&language[1..], "jc-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "jc-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "jc-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "jc-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "jc-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "jc-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "jc-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
    }
    b'w' => {
      match lang_matches(&language[1..], "uu-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "uu-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "uu-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "uu-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "uu-", 3) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "uu-", 3) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "uu-", 3) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
    }
    b'y' => match lang_matches(&language[1..], "ue-hans") {
      true => {
        tags.push({
          let Ok(value) = Tag::new(b"ZHS ");
          value
        });
        return true;
      }
      false => {}
    },
    b'z' => {
      match lang_matches(&language[1..], "h-hant-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "h-hant-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match &language[1..] == "h-min-nan" {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "h-hans") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match lang_matches(&language[1..], "h-hant") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
      match &language[1..] == "h-min" {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHS ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "h-", 2) && subtag_matches(language, "-hk") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHH ");
            value
          });
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "h-", 2) && subtag_matches(language, "-mo") {
        true => {
          let possible_tags = &[
            ({
              let Ok(value) = Tag::new(b"ZHTM");
              value
            }),
            ({
              let Ok(value) = Tag::new(b"ZHH ");
              value
            }),
          ];
          tags.extend_from_slice(possible_tags);
          return true;
        }
        false => {}
      }
      match strncmp(&language[1..], "h-", 2) && subtag_matches(language, "-tw") {
        true => {
          tags.push({
            let Ok(value) = Tag::new(b"ZHT ");
            value
          });
          return true;
        }
        false => {}
      }
    }
    _ => {}
  }
  false
}
