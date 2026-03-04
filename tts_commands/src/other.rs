use aformat::{aformat, astr};
use anyhow::Error;

use poise::{
    CreateReply,
    serenity_prelude::{self as serenity, builder::*},
};

use aformat::ToArrayString;
use tts_core::{
    common::{fetch_audio, prepare_url},
    opt_ext::OptionTryUnwrap,
    structs::{Command, CommandResult, Context, IsPremium, TTSMode},
    traits::PoiseContextExt as _,
};

/// Generates TTS and sends it in the current text channel!
#[poise::command(
    category = "Extra Commands",
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES | ATTACH_FILES"
)]
pub async fn tts(
    ctx: Context<'_>,
    #[description = "The text to TTS"]
    #[rest]
    message: String,
) -> CommandResult {
    let is_unnecessary_command_invoke = async {
        if !matches!(ctx, poise::Context::Prefix(_)) {
            return Ok(false);
        }

        let (guild_id, author_voice_cid, bot_voice_cid) = {
            if let Some(guild) = ctx.guild() {
                (
                    guild.id,
                    guild
                        .voice_states
                        .get(&ctx.author().id)
                        .and_then(|vc| vc.channel_id),
                    guild
                        .voice_states
                        .get(&ctx.cache().current_user().id)
                        .and_then(|vc| vc.channel_id),
                )
            } else {
                return Ok(false);
            }
        };

        if author_voice_cid.is_some() && author_voice_cid == bot_voice_cid {
            let setup_channel = ctx.data().guilds_db.get(guild_id.into()).await?.channel;
            if setup_channel == Some(ctx.channel_id().expect_channel()) {
                return Ok(true);
            }
        }

        Ok::<_, Error>(false)
    };

    if is_unnecessary_command_invoke.await? {
        ctx.say("You don't need to include the `/tts` for messages to be said!")
            .await?;
        Ok(())
    } else {
        tts_(ctx, ctx.author(), &message).await
    }
}

async fn tts_(ctx: Context<'_>, author: &serenity::User, message: &str) -> CommandResult {
    let attachment = {
        let data = ctx.data();
        let http = ctx.http();
        let guild_info = if let Some(guild_id) = ctx.guild_id() {
            Some((guild_id, data.is_premium_simple(http, guild_id).await?))
        } else {
            None
        };

        let (voice, mode) = data
            .parse_user_or_guild_with_premium(author.id, guild_info)
            .await?;

        let guild_row;
        let translation_lang = if let Some((guild_id, is_premium)) = guild_info {
            guild_row = data.guilds_db.get(guild_id.into()).await?;
            guild_row.target_lang(IsPremium::from(is_premium))
        } else {
            None
        };

        let author_name: String = author
            .name
            .chars()
            .filter(|char| char.is_alphanumeric())
            .collect();
        let speaking_rate = data.speaking_rate(author.id, mode).await?;

        let url = prepare_url(
            data.config.tts_service.clone(),
            message,
            &voice,
            mode,
            &speaking_rate,
            &u64::MAX.to_arraystring(),
            translation_lang,
        );

        let auth_key = data.config.tts_service_auth_key.as_deref();
        let audio = fetch_audio(&data.reqwest, url, auth_key)
            .await?
            .try_unwrap()?
            .bytes()
            .await?;

        let mut file_name = author_name;
        file_name.push_str(&aformat!(
            "-{}.{}",
            ctx.id(),
            match mode {
                TTSMode::gTTS | TTSMode::gCloud | TTSMode::Polly => astr!("mp3"),
                TTSMode::eSpeak => astr!("wav"),
            }
        ));

        serenity::CreateAttachment::bytes(audio.to_vec(), file_name)
    };

    ctx.send(
        CreateReply::default()
            .content("Generated some TTS!")
            .attachment(attachment),
    )
    .await?;

    Ok(())
}

pub fn commands() -> [Command; 1] {
    [tts()]
}
