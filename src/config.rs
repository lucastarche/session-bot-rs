use std::{fs::File, io::Read, path::Path, sync::Arc};

use serde::Deserialize;
use serenity::{
    model::id::{ApplicationId, ChannelId, GuildId, RoleId, UserId},
    prelude::{RwLock, TypeMapKey},
};
use tracing::log::error;

use crate::{commands::help::HelpPage, embed::Embed};

#[derive(Deserialize, Clone)]
pub struct Config {
    pub application_id: ApplicationId,
    pub discord_token: String,
    pub guild_id: GuildId,
    pub vc_channel: ChannelId,
    pub default_user_role: Option<RoleId>,
    pub default_time: String,
    pub idle_text: String,
    pub timezone_text: String,
    pub ip_embed: Embed,
    pub default_help: Embed,
    pub help: Vec<HelpPage>,
    pub admins: Vec<UserId>,
    pub games: Vec<Game>,
    pub colors: Vec<ColorRole>,
}

#[derive(Deserialize, Clone)]
pub struct Game {
    pub name: String,
    pub channel_id: Option<ChannelId>,
    pub role_id: RoleId,
    pub all_roles_exception: Option<bool>,
}

#[derive(Deserialize, Clone)]
pub struct ColorRole {
    pub name: String,
    pub role_id: RoleId,
}

impl TypeMapKey for Config {
    type Value = Arc<RwLock<Config>>;
}

impl Config {
    pub fn read_from(path: &Path) -> Option<Self> {
        let mut config_file = match File::open(path) {
            Ok(f) => f,
            Err(why) => {
                error!("Error opening {:?}: {}", path, why);
                return None;
            }
        };

        let mut config_str = String::new();
        if let Err(why) = config_file.read_to_string(&mut config_str) {
            error!("Error reading {:?}: {}", path, why);
            return None;
        }

        match toml::from_str(&config_str) {
            Ok(config) => Some(config),
            Err(why) => {
                error!("Error parsing {:?} to config: {}", path, why);
                None
            }
        }
    }
}
