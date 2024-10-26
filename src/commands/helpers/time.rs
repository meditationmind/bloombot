#[derive(poise::ChoiceParameter)]
pub enum PlusOffsetChoice {
  #[name = "UTC+1 (BST, CET, IST, WAT, WEST)"]
  UTCPlus1,
  #[name = "UTC+2 (CAT, CEST, EET, IST, SAST, WAST)"]
  UTCPlus2,
  #[name = "UTC+3 (AST, EAT, EEST, FET, IDT, IOT, MSK, USZ1)"]
  UTCPlus3,
  #[name = "UTC+3:30 (IRST)"]
  UTCPlus3_30,
  #[name = "UTC+4 (AMT, AZT, GET, GST, MUT, RET, SAMT, SCT, VOLT)"]
  UTCPlus4,
  #[name = "UTC+4:30 (AFT, IRDT)"]
  UTCPlus4_30,
  #[name = "UTC+5 (HMT, MAWT, MVT, ORAT, PKT, TFT, TJT, TMT, UZT, YEKT)"]
  UTCPlus5,
  #[name = "UTC+5:30 (IST, SLST)"]
  UTCPlus5_30,
  #[name = "UTC+5:45 (NPT)"]
  UTCPlus5_45,
  #[name = "UTC+6 (BDT, BIOT, BST, BTT, KGT, OMST, VOST)"]
  UTCPlus6,
  #[name = "UTC+6:30 (CCT, MMT, MST)"]
  UTCPlus6_30,
  #[name = "UTC+7 (CXT, DAVT, HOVT, ICT, KRAT, THA, WIT)"]
  UTCPlus7,
  #[name = "UTC+8 (ACT, AWST, BDT, CHOT, CIT, CST, CT, HKT, IRKT, MST, MYT, PST, SGT, SST, ULAT, WST)"]
  UTCPlus8,
  #[name = "UTC+8:45 (CWST)"]
  UTCPlus8_45,
  #[name = "UTC+9 (AWDT, EIT, JST, KST, TLT, YAKT)"]
  UTCPlus9,
  #[name = "UTC+9:30 (ACST, CST)"]
  UTCPlus9_30,
  #[name = "UTC+10 (AEST, ChST, CHUT, DDUT, EST, PGT, VLAT)"]
  UTCPlus10,
  #[name = "UTC+10:30 (ACDT, CST, LHST)"]
  UTCPlus10_30,
  #[name = "UTC+11 (AEDT, BST, KOST, LHST, MIST, NCT, PONT, SAKT, SBT, SRET, VUT, NFT)"]
  UTCPlus11,
  #[name = "UTC+12 (FJT, GILT, MAGT, MHT, NZST, PETT, TVT, WAKT)"]
  UTCPlus12,
  #[name = "UTC+12:45 (CHAST)"]
  UTCPlus12_45,
  #[name = "UTC+13 (NZDT, PHOT, TKT, TOT)"]
  UTCPlus13,
  #[name = "UTC+13:45 (CHADT)"]
  UTCPlus13_45,
  #[name = "UTC+14 (LINT)"]
  UTCPlus14,
}

#[derive(poise::ChoiceParameter)]
pub enum MinusOffsetChoice {
  #[name = "UTC-12 (BIT)"]
  UTCMinus12,
  #[name = "UTC-11 (NUT, SST)"]
  UTCMinus11,
  #[name = "UTC-10 (CKT, HAST, HST, TAHT)"]
  UTCMinus10,
  #[name = "UTC-9:30 (MART, MIT)"]
  UTCMinus9_30,
  #[name = "UTC-9 (AKST, GAMT, GIT, HADT)"]
  UTCMinus9,
  #[name = "UTC-8 (AKDT, CIST, PST)"]
  UTCMinus8,
  #[name = "UTC-7 (MST, PDT)"]
  UTCMinus7,
  #[name = "UTC-6 (CST, EAST, GALT, MDT)"]
  UTCMinus6,
  #[name = "UTC-5 (ACT, CDT, COT, CST, EASST, ECT, EST, PET)"]
  UTCMinus5,
  #[name = "UTC-4:30 (VET)"]
  UTCMinus4_30,
  #[name = "UTC-4 (AMT, AST, BOT, CDT, CLT, COST, ECT, EDT, FKT, GYT, PYT)"]
  UTCMinus4,
  #[name = "UTC-3:30 (NST, NT)"]
  UTCMinus3_30,
  #[name = "UTC-3 (ADT, AMST, ART, BRT, CLST, FKST, GFT, PMST, PYST, ROTT, SRT, UYT)"]
  UTCMinus3,
  #[name = "UTC-2:30 (NDT)"]
  UTCMinus2_30,
  #[name = "UTC-2 (BRST, FNT, GST, PMDT, UYST)"]
  UTCMinus2,
  #[name = "UTC-1 (AZOST, CVT, EGT)"]
  UTCMinus1,
}

pub fn choice_from_offset(offset: i16) -> (Option<MinusOffsetChoice>, Option<PlusOffsetChoice>) {
  match offset {
    -720 => (Some(MinusOffsetChoice::UTCMinus12), None),
    -660 => (Some(MinusOffsetChoice::UTCMinus11), None),
    -600 => (Some(MinusOffsetChoice::UTCMinus10), None),
    -570 => (Some(MinusOffsetChoice::UTCMinus9_30), None),
    -540 => (Some(MinusOffsetChoice::UTCMinus9), None),
    -480 => (Some(MinusOffsetChoice::UTCMinus8), None),
    -420 => (Some(MinusOffsetChoice::UTCMinus7), None),
    -360 => (Some(MinusOffsetChoice::UTCMinus6), None),
    -300 => (Some(MinusOffsetChoice::UTCMinus5), None),
    -270 => (Some(MinusOffsetChoice::UTCMinus4_30), None),
    -240 => (Some(MinusOffsetChoice::UTCMinus4), None),
    -210 => (Some(MinusOffsetChoice::UTCMinus3_30), None),
    -180 => (Some(MinusOffsetChoice::UTCMinus3), None),
    -150 => (Some(MinusOffsetChoice::UTCMinus2_30), None),
    -120 => (Some(MinusOffsetChoice::UTCMinus2), None),
    -60 => (Some(MinusOffsetChoice::UTCMinus1), None),
    60 => (None, Some(PlusOffsetChoice::UTCPlus1)),
    120 => (None, Some(PlusOffsetChoice::UTCPlus2)),
    180 => (None, Some(PlusOffsetChoice::UTCPlus3)),
    210 => (None, Some(PlusOffsetChoice::UTCPlus3_30)),
    240 => (None, Some(PlusOffsetChoice::UTCPlus4)),
    270 => (None, Some(PlusOffsetChoice::UTCPlus4_30)),
    300 => (None, Some(PlusOffsetChoice::UTCPlus5)),
    330 => (None, Some(PlusOffsetChoice::UTCPlus5_30)),
    345 => (None, Some(PlusOffsetChoice::UTCPlus5_45)),
    360 => (None, Some(PlusOffsetChoice::UTCPlus6)),
    390 => (None, Some(PlusOffsetChoice::UTCPlus6_30)),
    420 => (None, Some(PlusOffsetChoice::UTCPlus7)),
    480 => (None, Some(PlusOffsetChoice::UTCPlus8)),
    525 => (None, Some(PlusOffsetChoice::UTCPlus8_45)),
    540 => (None, Some(PlusOffsetChoice::UTCPlus9)),
    570 => (None, Some(PlusOffsetChoice::UTCPlus9_30)),
    600 => (None, Some(PlusOffsetChoice::UTCPlus10)),
    630 => (None, Some(PlusOffsetChoice::UTCPlus10_30)),
    660 => (None, Some(PlusOffsetChoice::UTCPlus11)),
    720 => (None, Some(PlusOffsetChoice::UTCPlus12)),
    765 => (None, Some(PlusOffsetChoice::UTCPlus12_45)),
    780 => (None, Some(PlusOffsetChoice::UTCPlus13)),
    825 => (None, Some(PlusOffsetChoice::UTCPlus13_45)),
    840 => (None, Some(PlusOffsetChoice::UTCPlus14)),
    _ => (None, None),
  }
}

pub fn offset_from_choice(
  minus_offset: Option<MinusOffsetChoice>,
  plus_offset: Option<PlusOffsetChoice>,
  default: i16,
) -> Result<i16, String> {
  match (minus_offset, plus_offset) {
    (None, None) => Ok(default),
    (Some(_), Some(_)) => Err(String::from("Cannot have both minus and plus offsets")),
    (Some(minus_offset), None) => Ok(match minus_offset {
      MinusOffsetChoice::UTCMinus12 => -720,
      MinusOffsetChoice::UTCMinus11 => -660,
      MinusOffsetChoice::UTCMinus10 => -600,
      MinusOffsetChoice::UTCMinus9_30 => -570,
      MinusOffsetChoice::UTCMinus9 => -540,
      MinusOffsetChoice::UTCMinus8 => -480,
      MinusOffsetChoice::UTCMinus7 => -420,
      MinusOffsetChoice::UTCMinus6 => -360,
      MinusOffsetChoice::UTCMinus5 => -300,
      MinusOffsetChoice::UTCMinus4_30 => -270,
      MinusOffsetChoice::UTCMinus4 => -240,
      MinusOffsetChoice::UTCMinus3_30 => -210,
      MinusOffsetChoice::UTCMinus3 => -180,
      MinusOffsetChoice::UTCMinus2_30 => -150,
      MinusOffsetChoice::UTCMinus2 => -120,
      MinusOffsetChoice::UTCMinus1 => -60,
    }),
    (None, Some(plus_offset)) => Ok(match plus_offset {
      PlusOffsetChoice::UTCPlus1 => 60,
      PlusOffsetChoice::UTCPlus2 => 120,
      PlusOffsetChoice::UTCPlus3 => 180,
      PlusOffsetChoice::UTCPlus3_30 => 210,
      PlusOffsetChoice::UTCPlus4 => 240,
      PlusOffsetChoice::UTCPlus4_30 => 270,
      PlusOffsetChoice::UTCPlus5 => 300,
      PlusOffsetChoice::UTCPlus5_30 => 330,
      PlusOffsetChoice::UTCPlus5_45 => 345,
      PlusOffsetChoice::UTCPlus6 => 360,
      PlusOffsetChoice::UTCPlus6_30 => 390,
      PlusOffsetChoice::UTCPlus7 => 420,
      PlusOffsetChoice::UTCPlus8 => 480,
      PlusOffsetChoice::UTCPlus8_45 => 525,
      PlusOffsetChoice::UTCPlus9 => 540,
      PlusOffsetChoice::UTCPlus9_30 => 570,
      PlusOffsetChoice::UTCPlus10 => 600,
      PlusOffsetChoice::UTCPlus10_30 => 630,
      PlusOffsetChoice::UTCPlus11 => 660,
      PlusOffsetChoice::UTCPlus12 => 720,
      PlusOffsetChoice::UTCPlus12_45 => 765,
      PlusOffsetChoice::UTCPlus13 => 780,
      PlusOffsetChoice::UTCPlus13_45 => 825,
      PlusOffsetChoice::UTCPlus14 => 840,
    }),
  }
}

#[derive(poise::ChoiceParameter)]
pub enum Timeframe {
  Yearly,
  Monthly,
  Weekly,
  Daily,
}

#[derive(poise::ChoiceParameter, PartialEq)]
pub enum ChallengeTimeframe {
  #[name = "Monthly Challenge"]
  Monthly,
  #[name = "365-Day Challenge"]
  YearRound,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_choice_from_offset() {
    matches!(
      choice_from_offset(-420),
      (Some(MinusOffsetChoice::UTCMinus7), None)
    );
    matches!(
      choice_from_offset(345),
      (None, Some(PlusOffsetChoice::UTCPlus5_45))
    );
    matches!(choice_from_offset(0), (None, None));
    matches!(choice_from_offset(777), (None, None));
  }

  #[test]
  fn test_offset_from_choice() {
    assert_eq!(
      offset_from_choice(Some(MinusOffsetChoice::UTCMinus7), None, 0).unwrap_or(0),
      -420
    );
    assert_eq!(
      offset_from_choice(None, Some(PlusOffsetChoice::UTCPlus5_45), 0).unwrap_or(0),
      345
    );
    assert_eq!(offset_from_choice(None, None, 0).unwrap_or(111), 0);
    assert_eq!(offset_from_choice(None, None, 777).unwrap_or(0), 777);
    let Err(err) = offset_from_choice(
      Some(MinusOffsetChoice::UTCMinus7),
      Some(PlusOffsetChoice::UTCPlus5_45),
      0,
    ) else {
      panic!("Expected Err, got Ok");
    };
    assert_eq!(err, "Cannot have both minus and plus offsets");
  }
}
