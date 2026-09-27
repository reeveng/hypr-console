use console_music::player::local;
use console_music_player::answers::{Reply, Song, metadata};
use std::error::Error;

#[test]
fn the_cover_the_player_names_is_the_file_the_panel_draws() -> Result<(), Box<dyn Error>> {
    let folder = console_core_temporary_directories::fresh("music-cover-address")?;
    let cover = folder.join("100%41 Beyonc\u{e9} [x9].jpg");

    console_core_atomic_writes::whole(&cover, b"a cover")?;

    let Ok(said) = metadata(&Song { art: Some(cover.clone()), ..Song::default() });
    let named = said
        .iter()
        .find_map(|(key, reply)| match (key.as_str(), reply) {
            ("mpris:artUrl", Reply::Word(url)) => Some(url.clone()),
            (_, Reply::Track(_) | Reply::Word(_) | Reply::Strings(_) | Reply::Long(_)) => None,
        })
        .ok_or("the player to name its cover")?;

    assert_eq!(local(&named), Ok(Some(cover)), "the panel read {named} as some other file");

    Ok(())
}
