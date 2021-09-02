use std::sync::Arc;

use crate::{
    context_ext::ContextExt,
    session::{Session, UserState},
};

use super::{
    interaction_handler::{CommandHandler, InteractionHandler},
    prelude::interaction_respond_with_private_message,
    status::get_status_embed,
};
use chrono::{DateTime, Local, NaiveDateTime, NaiveTime, TimeZone};
use serenity::{
    async_trait,
    client::Context,
    model::{
        channel::Message,
        id::{ChannelId, RoleId},
        interactions::{
            application_command::{
                ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue,
                ApplicationCommandOptionType,
            },
            message_component::ButtonStyle,
            Interaction, InteractionResponseType,
        },
    },
    prelude::RwLock,
};
use tracing::warn;

#[derive(Clone, Copy)]
pub struct HostGame;

async fn ping_all_not_in_vc(ctx: Context, channel_id: u64) {
    let user_map = ctx.session().await.read().await.users.clone();
    let members = ChannelId(ctx.config().await.vc_channel)
        .to_channel(&ctx.http)
        .await
        .expect("Could not convert to Channel")
        .guild()
        .expect("Could not convert to GuildChannel")
        .members(ctx.cache)
        .await
        .expect("Could not retrieve Member list");

    let pings = user_map
        .iter()
        .filter(|(u, s)| **s == UserState::WillJoin && !members.iter().any(|m| m.user.id == **u))
        .fold(String::new(), |lhs, (rhs, _)| {
            lhs + format!("<@{}> ", rhs).as_str()
        });

    if pings.is_empty() {
        return;
    }

    let content = format!("{}you're late, get in the VC!", pings);
    if let Err(why) = ChannelId(channel_id)
        .send_message(&ctx.http, |message| message.content(content))
        .await
    {
        warn!("Error sending message to text channel: {}", why);
    }
}

async fn start_session(
    ctx: Context,
    interaction: ApplicationCommandInteraction,
    time: &str,
    description: &str,
) -> bool {
    let channel_id = interaction.channel_id.as_u64().to_owned();
    let guild_id = interaction.guild_id.unwrap_or_default().as_u64().to_owned();

    let session_time =
        NaiveTime::parse_from_str(time, "%H:%M").expect("Error parsing default time to string");
    let now = Local::now();
    let today = Local::today();
    let session_time = Local
        .from_local_datetime(&NaiveDateTime::new(today.naive_local(), session_time))
        .earliest()
        .expect("Error parsing time to DateTime");

    let session_time = if (session_time - now) < chrono::Duration::zero() {
        session_time
            .date()
            .succ()
            .and_time(session_time.time())
            .unwrap()
    } else {
        session_time
    };

    let ctx2 = ctx.clone();
    let handle = tokio::task::spawn(async move {
        let ctx = ctx2.clone();
        let ten_minutes_before =
            session_time.signed_duration_since(now) - chrono::Duration::minutes(10);

        tokio::time::sleep(
            ten_minutes_before
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(60)),
        )
        .await;

        let game = ctx.session().await.read().await.game.clone();
        let embed = get_status_embed(ctx.clone(), guild_id).await;

        ChannelId(channel_id)
            .send_message(&ctx.http, |message| {
                message.set_embed(embed).content(format!(
                    "<@&{}> Game starting in 10 minutes!",
                    RoleId(game.role_id).to_string()
                ))
            })
            .await
            .expect("Error sending message to channel");

        tokio::time::sleep(
            session_time
                .signed_duration_since(now)
                .to_std()
                .unwrap_or_default(),
        )
        .await;

        let member_amount = ctx
            .session()
            .await
            .read()
            .await
            .users
            .iter()
            .filter(|(_, s)| **s == UserState::WillJoin)
            .count();

        let embed = get_status_embed(ctx.clone(), guild_id).await;
        ChannelId(channel_id)
            .send_message(&ctx.http, |message| {
                message.set_embed(embed).content(format!(
                    "{} Session has started! {} people said Yes!",
                    game.name, member_amount
                ))
            })
            .await
            .expect("Error sending message to channel");

        tokio::time::sleep(std::time::Duration::from_secs(60 * 10)).await;
        // ping users who said yes but not in VC
        ping_all_not_in_vc(ctx, channel_id).await;
    });

    let game = match ctx
        .config()
        .await
        .games
        .iter()
        .find(|g| g.channel_id == channel_id)
    {
        Some(g) => g,
        None => {
            handle.abort();

            return false;
        }
    }
    .clone();

    let message = send_session_message(
        ctx.clone(),
        &interaction,
        session_time,
        description,
        game.role_id,
    )
    .await;

    ctx.data
        .write()
        .await
        .insert::<Session>(Arc::new(RwLock::new(Session::new(
            game,
            handle,
            session_time,
            message.id,
            interaction.user.id,
        ))));
    true
}

async fn send_session_message(
    ctx: Context,
    interaction: &ApplicationCommandInteraction,
    time: DateTime<Local>,
    description: &str,
    role_id: u64,
) -> Message {
    let description = if description.is_empty() {
        description.to_string()
    } else {
        format!("Description: {}", description)
    };

    interaction
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message
                        .content(format!(
                            "<@&{}> A session is planned!\nTime: <t:{}>\n{}",
                            role_id,
                            time.timestamp(),
                            description
                        ))
                        .components(|component| {
                            component.create_action_row(|row| {
                                row.create_button(|button| {
                                    button
                                        .custom_id("button-yes")
                                        .label("Yes")
                                        .style(ButtonStyle::Success)
                                })
                                .create_button(|button| {
                                    button
                                        .custom_id("button-maybe")
                                        .label("Maybe")
                                        .style(ButtonStyle::Secondary)
                                })
                                .create_button(|button| {
                                    button
                                        .custom_id("button-no")
                                        .label("No")
                                        .style(ButtonStyle::Danger)
                                })
                            })
                        })
                })
        })
        .await
        .expect("Error responding to interaction");

    let message = interaction
        .get_interaction_response(&ctx.http)
        .await
        .expect("Error retrieving interaction response");

    message.pin(&ctx.http).await.expect("Error pinning message");
    message
}

impl InteractionHandler for HostGame {
    fn name(&self) -> &'static str {
        "hostgame"
    }
}

#[async_trait]
impl CommandHandler for HostGame {
    async fn invoke(&self, ctx: Context, interaction: ApplicationCommandInteraction) {
        if ctx.is_session_running().await {
            interaction_respond_with_private_message(
                ctx,
                Interaction::ApplicationCommand(interaction),
                "Error creating session: Session already running",
            )
            .await;
            return;
        }

        let config = ctx.config().await;
        let mut time = config.default_time;
        let mut description = String::new();

        for option in &interaction.data.options {
            match option.name.as_ref() {
                "time" => {
                    if let ApplicationCommandInteractionDataOptionValue::String(s) =
                        option.resolved.as_ref().unwrap()
                    {
                        time = s.clone();
                    }
                }
                "description" => {
                    if let ApplicationCommandInteractionDataOptionValue::String(s) =
                        option.resolved.as_ref().unwrap()
                    {
                        description = s.clone();
                    }
                }
                _ => {}
            }
        }

        if !start_session(ctx.clone(), interaction.clone(), &time, &description).await {
            interaction_respond_with_private_message(
                ctx,
                Interaction::ApplicationCommand(interaction),
                "Error creating session: No game registered to this channel",
            )
            .await;
        }
    }

    fn create_command(
        self,
        command: &mut serenity::builder::CreateApplicationCommand,
    ) -> &mut serenity::builder::CreateApplicationCommand {
        command
            .name(self.name())
            .description("Hosts a new game")
            .create_option(|option| {
                option
                    .kind(ApplicationCommandOptionType::String)
                    .name("time")
                    .description("Time to host the session")
            })
            .create_option(|option| {
                option
                    .kind(ApplicationCommandOptionType::String)
                    .name("description")
                    .description("Sets the session description")
            })
    }
}
