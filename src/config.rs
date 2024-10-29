#![allow(clippy::unreadable_literal)]
use poise::serenity_prelude::{self as serenity, Embed, GuildId, RoleId};
use std::fmt;

pub const MEDITATION_MIND: GuildId = GuildId::new(244917432383176705);
pub const EMBED_COLOR: u32 = 0xFDAC2E;
pub const MIN_STARS: u64 = 5;

/// Sensible defaults for use within our application.
pub struct BloomBotEmbed {}

impl BloomBotEmbed {
  #[allow(clippy::new_ret_no_self)]
  pub fn new() -> serenity::CreateEmbed {
    serenity::CreateEmbed::default().color(EMBED_COLOR)
  }

  pub fn from(embed: Embed) -> serenity::CreateEmbed {
    serenity::CreateEmbed::from(embed).color(EMBED_COLOR)
  }
}

pub struct EntriesPerPage {
  pub default: usize,
  pub bookmarks: usize,
  pub glossary: usize,
}

pub const ENTRIES_PER_PAGE: EntriesPerPage = EntriesPerPage {
  default: 10,
  bookmarks: 5,
  glossary: 15,
};

pub struct Roles {
  pub welcome_team: u64,
  pub meditation_challenger: u64,
  pub meditation_challenger_365: u64,
  pub patreon: u64,
  pub kofi: u64,
  pub staff: u64,
  pub community_sit_helper: u64,
}

pub const ROLES: Roles = Roles {
  welcome_team: 828291690917265418,
  meditation_challenger: 796821826369617970,
  meditation_challenger_365: 516750476268666880,
  patreon: 543900027928444935,
  kofi: 1083219974509826048,
  staff: 788760128010059786,
  community_sit_helper: 1285275266549158050,
};

pub struct Channels {
  pub welcome: u64,
  pub announcement: u64,
  pub logs: u64,
  pub bloomlogs: u64,
  pub starchannel: u64,
  pub reportchannel: u64,
  pub donators: u64,
  pub suggestion: u64,
  pub tracking: u64,
  pub private_thread_default: u64,
}

pub const CHANNELS: Channels = Channels {
  welcome: 493402917001494539,
  announcement: 244917519477899264,
  logs: 441207765357035541,
  bloomlogs: 1161911290915209297,
  starchannel: 856865368098078720,
  reportchannel: 855894610001395743,
  donators: 551895169532952578,
  suggestion: 553676378621476887,
  tracking: 440556997364940801,
  private_thread_default: 501464482996944909,
};

pub struct Emotes<'a> {
  pub star: &'a str,
  pub report: u64,
}

pub const EMOTES: Emotes = Emotes {
  star: "⭐",
  report: 852463521894629376,
};

pub struct SimpleEmoji<'a> {
  pub animated: bool,
  pub id: u64,
  pub name: &'a str,
}

#[allow(dead_code)]
pub struct BloomEmoji<'a> {
  pub pepeglow: SimpleEmoji<'a>,
  pub aww: SimpleEmoji<'a>,
  pub loveit: SimpleEmoji<'a>,
  pub mminfo: SimpleEmoji<'a>,
  pub mmx: SimpleEmoji<'a>,
  pub mmcheck: SimpleEmoji<'a>,
}

pub const EMOJI: BloomEmoji = BloomEmoji {
  pepeglow: SimpleEmoji {
    animated: false,
    id: 1279541855150673991,
    name: "pepeglow",
  },
  aww: SimpleEmoji {
    animated: false,
    id: 1279541172049678438,
    name: "aww",
  },
  loveit: SimpleEmoji {
    animated: false,
    id: 1279540710747672689,
    name: "loveit",
  },
  mminfo: SimpleEmoji {
    animated: false,
    id: 1279517292455264359,
    name: "mminfo",
  },
  mmx: SimpleEmoji {
    animated: false,
    id: 1279517275749089290,
    name: "mmx",
  },
  mmcheck: SimpleEmoji {
    animated: false,
    id: 1279517233877483601,
    name: "mmcheck",
  },
};

impl fmt::Display for SimpleEmoji<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.animated {
      f.write_str("<a:")?;
    } else {
      f.write_str("<:")?;
    }
    f.write_str(self.name)?;
    fmt::Write::write_char(f, ':')?;
    fmt::Display::fmt(&self.id, f)?;
    fmt::Write::write_char(f, '>')
  }
}

#[derive(Debug, Eq, PartialEq)]
pub enum TimeSumRoles {
  One,
  Two,
  Three,
  Four,
  Five,
  Six,
  Seven,
  Eight,
  Nine,
  Ten,
  Eleven,
  Twelve,
  Thirteen,
  Fourteen,
  Fifteen,
}

impl TimeSumRoles {
  pub fn to_role_id(&self) -> serenity::RoleId {
    serenity::RoleId::new(match self {
      TimeSumRoles::One => 504641899890475018,
      TimeSumRoles::Two => 504641945596067851,
      TimeSumRoles::Three => 504642088760115241,
      TimeSumRoles::Four => 504641974486302751,
      TimeSumRoles::Five => 504642451898630164,
      TimeSumRoles::Six => 504642479459532810,
      TimeSumRoles::Seven => 504642975519866881,
      TimeSumRoles::Eight => 504643005479649280,
      TimeSumRoles::Nine => 504643037515874317,
      TimeSumRoles::Ten => 504645771464015893,
      TimeSumRoles::Eleven => 504645799821574144,
      TimeSumRoles::Twelve => 504645823888621568,
      TimeSumRoles::Thirteen => 1224667049175941120,
      TimeSumRoles::Fourteen => 1224671462657359972,
      TimeSumRoles::Fifteen => 1224678890161573969,
    })
  }

  fn from_role_id(id: serenity::RoleId) -> Option<TimeSumRoles> {
    match <u64>::from(id) {
      504641899890475018 => Some(TimeSumRoles::One),
      504641945596067851 => Some(TimeSumRoles::Two),
      504642088760115241 => Some(TimeSumRoles::Three),
      504641974486302751 => Some(TimeSumRoles::Four),
      504642451898630164 => Some(TimeSumRoles::Five),
      504642479459532810 => Some(TimeSumRoles::Six),
      504642975519866881 => Some(TimeSumRoles::Seven),
      504643005479649280 => Some(TimeSumRoles::Eight),
      504643037515874317 => Some(TimeSumRoles::Nine),
      504645771464015893 => Some(TimeSumRoles::Ten),
      504645799821574144 => Some(TimeSumRoles::Eleven),
      504645823888621568 => Some(TimeSumRoles::Twelve),
      1224667049175941120 => Some(TimeSumRoles::Thirteen),
      1224671462657359972 => Some(TimeSumRoles::Fourteen),
      1224678890161573969 => Some(TimeSumRoles::Fifteen),
      _ => None,
    }
  }

  pub fn to_role_icon<'a>(&self) -> &'a str {
    match self {
      TimeSumRoles::One => "⭐",
      TimeSumRoles::Two => "⭐⭐",
      TimeSumRoles::Three => "⭐⭐⭐",
      TimeSumRoles::Four => "🌟",
      TimeSumRoles::Five => "🌟🌟",
      TimeSumRoles::Six => "🌟🌟🌟",
      TimeSumRoles::Seven => "✨",
      TimeSumRoles::Eight => "✨✨",
      TimeSumRoles::Nine => "✨✨✨",
      TimeSumRoles::Ten => "💫",
      TimeSumRoles::Eleven => "💫💫",
      TimeSumRoles::Twelve => "💫💫💫",
      TimeSumRoles::Thirteen => "🔥",
      TimeSumRoles::Fourteen => "🔥🔥",
      TimeSumRoles::Fifteen => "🔥🔥🔥",
    }
  }

  pub fn get_users_current_roles(member_roles: &Vec<RoleId>) -> Vec<RoleId> {
    let mut roles = Vec::new();

    for user_role in member_roles {
      let Some(matching_role_id) = TimeSumRoles::from_role_id(*user_role) else {
        continue;
      };

      roles.push(matching_role_id.to_role_id());
    }

    roles
  }

  pub fn from_sum(sum: i64) -> Option<TimeSumRoles> {
    match sum {
      i64::MIN..=49 => None,
      50..=99 => Some(TimeSumRoles::One),
      100..=149 => Some(TimeSumRoles::Two),
      150..=249 => Some(TimeSumRoles::Three),
      250..=499 => Some(TimeSumRoles::Four),
      500..=999 => Some(TimeSumRoles::Five),
      1000..=1999 => Some(TimeSumRoles::Six),
      2000..=4999 => Some(TimeSumRoles::Seven),
      5000..=9999 => Some(TimeSumRoles::Eight),
      10000..=19999 => Some(TimeSumRoles::Nine),
      20000..=49999 => Some(TimeSumRoles::Ten),
      50000..=99999 => Some(TimeSumRoles::Eleven),
      100000..=119999 => Some(TimeSumRoles::Twelve),
      120000..=149999 => Some(TimeSumRoles::Thirteen),
      150000..=199999 => Some(TimeSumRoles::Fourteen),
      200000..=i64::MAX => Some(TimeSumRoles::Fifteen),
    }
  }
}

#[derive(Debug, Eq, PartialEq)]
pub enum StreakRoles {
  Egg,
  HatchingChick,
  BabyChick,
  Chicken,
  Dove,
  Owl,
  Eagle,
  Dragon,
  Alien,
  SpaceInvader,
}

impl StreakRoles {
  pub fn to_role_id(&self) -> serenity::RoleId {
    serenity::RoleId::new(match self {
      StreakRoles::Egg => 857242224390832158,
      StreakRoles::HatchingChick => 857242222529347584,
      StreakRoles::BabyChick => 857242220675465227,
      StreakRoles::Chicken => 857242218695229450,
      StreakRoles::Dove => 857242216493219862,
      StreakRoles::Owl => 857242214588612629,
      StreakRoles::Eagle => 857242212991762463,
      StreakRoles::Dragon => 857242210302427186,
      StreakRoles::Alien => 857242155784863754,
      StreakRoles::SpaceInvader => 1226730813190836367,
    })
  }

  pub fn to_role_icon<'a>(&self) -> &'a str {
    match self {
      StreakRoles::Egg => "🥚",
      StreakRoles::HatchingChick => "🐣",
      StreakRoles::BabyChick => "🐤",
      StreakRoles::Chicken => "🐔",
      StreakRoles::Dove => "🕊️",
      StreakRoles::Owl => "🦉",
      StreakRoles::Eagle => "🦅",
      StreakRoles::Dragon => "🐉",
      StreakRoles::Alien => "👽",
      StreakRoles::SpaceInvader => "👾",
    }
  }

  pub fn from_streak(streak: u64) -> Option<StreakRoles> {
    match streak {
      0..=6 => None,
      7..=13 => Some(StreakRoles::Egg),
      14..=27 => Some(StreakRoles::HatchingChick),
      28..=34 => Some(StreakRoles::BabyChick),
      35..=55 => Some(StreakRoles::Chicken),
      56..=69 => Some(StreakRoles::Dove),
      70..=139 => Some(StreakRoles::Owl),
      140..=364 => Some(StreakRoles::Eagle),
      365..=729 => Some(StreakRoles::Dragon),
      730..=1824 => Some(StreakRoles::Alien),
      1825..=u64::MAX => Some(StreakRoles::SpaceInvader),
    }
  }

  pub fn get_users_current_roles(member_roles: &Vec<RoleId>) -> Vec<RoleId> {
    let mut roles = Vec::new();

    for user_role in member_roles {
      let Some(matching_role_id) = StreakRoles::from_role_id(*user_role) else {
        continue;
      };

      roles.push(matching_role_id.to_role_id());
    }

    roles
  }

  fn from_role_id(id: serenity::RoleId) -> Option<StreakRoles> {
    match <u64>::from(id) {
      857242224390832158 => Some(StreakRoles::Egg),
      857242222529347584 => Some(StreakRoles::HatchingChick),
      857242220675465227 => Some(StreakRoles::BabyChick),
      857242218695229450 => Some(StreakRoles::Chicken),
      857242216493219862 => Some(StreakRoles::Dove),
      857242214588612629 => Some(StreakRoles::Owl),
      857242212991762463 => Some(StreakRoles::Eagle),
      857242210302427186 => Some(StreakRoles::Dragon),
      857242155784863754 => Some(StreakRoles::Alien),
      1226730813190836367 => Some(StreakRoles::SpaceInvader),
      _ => None,
    }
  }
}
