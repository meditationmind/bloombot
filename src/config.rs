#![allow(clippy::unreadable_literal)]
use std::fmt::{Display, Formatter, Result, Write};

use poise::serenity_prelude::{ChannelId, CreateEmbed, Embed, GuildId, RoleId};

pub const MEDITATION_MIND: GuildId = GuildId::new(244917432383176705);
pub const SECRET_CATEGORY: &str = "Secret";
pub const EMBED_COLOR: u32 = 0xFDAC2E;
pub const MIN_STARS: u64 = 4;

/// Sensible defaults for use within our application.
pub struct BloomBotEmbed {}

impl BloomBotEmbed {
  #[allow(clippy::new_ret_no_self)]
  pub fn new() -> CreateEmbed {
    CreateEmbed::default().color(EMBED_COLOR)
  }

  pub fn from(embed: Embed) -> CreateEmbed {
    CreateEmbed::from(embed).color(EMBED_COLOR)
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

#[derive(Debug, Copy, Clone)]
pub struct Role(u64);

impl From<Role> for RoleId {
  fn from(val: Role) -> Self {
    RoleId::new(val.0)
  }
}

impl Display for Role {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result {
    write!(f, "<@&{}>", self.0)
  }
}

pub struct Roles {
  pub welcome_team: Role,
  pub meditation_challenger: Role,
  pub meditation_challenger_365: Role,
  pub patreon: Role,
  pub kofi: Role,
  pub staff: Role,
  pub community_sit_helper: Role,
  pub community_book_club_host: Role,
  pub no_pings: Role,
}

pub const ROLES: Roles = Roles {
  welcome_team: Role(828291690917265418),
  meditation_challenger: Role(796821826369617970),
  meditation_challenger_365: Role(516750476268666880),
  patreon: Role(543900027928444935),
  kofi: Role(1083219974509826048),
  staff: Role(788760128010059786),
  community_sit_helper: Role(1285275266549158050),
  community_book_club_host: Role(1355086929229647990),
  no_pings: Role(1128156466273058906),
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Channel(u64);

impl Channel {
  pub fn id(self) -> u64 {
    self.0
  }
}

impl PartialEq<Channel> for ChannelId {
  fn eq(&self, other: &Channel) -> bool {
    self.get() == other.0
  }
}

impl From<Channel> for ChannelId {
  fn from(val: Channel) -> Self {
    ChannelId::new(val.0)
  }
}

impl Display for Channel {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result {
    write!(f, "<#{}>", self.0)
  }
}

pub struct Channels {
  pub welcome: Channel,
  pub announcement: Channel,
  pub logs: Channel,
  pub bloomlogs: Channel,
  pub starchannel: Channel,
  pub reportchannel: Channel,
  pub donators: Channel,
  pub suggestion: Channel,
  pub tracking: Channel,
  pub private_thread_default: Channel,
  pub group_meditation: Channel,
  pub meditate_with_me_1: Channel,
  pub meditate_with_me_2: Channel,
  pub meditation_hall: Channel,
}

pub const CHANNELS: Channels = Channels {
  welcome: Channel(493402917001494539),
  announcement: Channel(244917519477899264),
  logs: Channel(441207765357035541),
  bloomlogs: Channel(1161911290915209297),
  starchannel: Channel(856865368098078720),
  reportchannel: Channel(855894610001395743),
  donators: Channel(551895169532952578),
  suggestion: Channel(553676378621476887),
  tracking: Channel(440556997364940801),
  private_thread_default: Channel(501464482996944909),
  group_meditation: Channel(462964692856602624),
  meditate_with_me_1: Channel(1062108971558776872),
  meditate_with_me_2: Channel(1143607063226875984),
  meditation_hall: Channel(909856372378722324),
};

#[derive(Debug, Copy, Clone)]
pub struct Command<'a>(&'a str, u64);

#[allow(clippy::struct_field_names)]
pub struct Commands<'a> {
  pub glossary_info: Command<'a>,
  pub glossary_search: Command<'a>,
  pub glossary_suggest: Command<'a>,
}

pub const COMMANDS: Commands = Commands {
  glossary_info: Command("glossary info", 1135659962308243479),
  glossary_search: Command("glossary search", 1135659962308243479),
  glossary_suggest: Command("glossary suggest", 1135659962308243479),
};

impl Display for Command<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result {
    write!(f, "</{}:{}>", self.0, self.1)
  }
}

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
  pub derpman: SimpleEmoji<'a>,
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
  derpman: SimpleEmoji {
    animated: false,
    id: 1309907402958700615,
    name: "derpman",
  },
};

impl Display for SimpleEmoji<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result {
    if self.animated {
      f.write_str("<a:")?;
    } else {
      f.write_str("<:")?;
    }
    f.write_str(self.name)?;
    Write::write_char(f, ':')?;
    Display::fmt(&self.id, f)?;
    Write::write_char(f, '>')
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
  pub fn to_role_id(&self) -> RoleId {
    RoleId::new(match self {
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

  fn from_role_id(id: RoleId) -> Option<TimeSumRoles> {
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

  pub fn current(member_roles: &[RoleId]) -> Vec<RoleId> {
    member_roles
      .iter()
      .filter_map(|role| TimeSumRoles::from_role_id(*role))
      .map(|role| role.to_role_id())
      .collect::<Vec<RoleId>>()
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
  pub fn to_role_id(&self) -> RoleId {
    RoleId::new(match self {
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

  pub fn current(member_roles: &[RoleId]) -> Vec<RoleId> {
    member_roles
      .iter()
      .filter_map(|role| StreakRoles::from_role_id(*role))
      .map(|role| role.to_role_id())
      .collect::<Vec<RoleId>>()
  }

  fn from_role_id(id: RoleId) -> Option<StreakRoles> {
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
