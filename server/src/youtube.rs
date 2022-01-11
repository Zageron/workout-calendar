use serde::{Deserialize, Serialize};
use youtube3::YouTube;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaylistWrapper {
    pub playlist: youtube3::api::Playlist,
    pub items: Vec<youtube3::api::PlaylistItem>,
}

pub async fn request_playlist(client: &YouTube, playlist_id: &str) -> Option<PlaylistWrapper> {
    let mut possible_playlist: Option<youtube3::api::Playlist> = None;

    // Get Playlist
    match client
        .playlists()
        .list(&["snippet".to_string()].to_vec())
        .max_results(1)
        .add_scope(youtube3::api::Scope::Readonly)
        .add_id(playlist_id)
        .doit()
        .await
    {
        Ok((_response, result)) => {
            if let Some(item) = result.items {
                if !item.is_empty() {
                    possible_playlist = Some(item[0].clone());
                }
            }
        }
        Err(e) => println!("Error: {:?}", e),
    }

    // Get Playlist Items
    let mut curr_page_token = String::new();
    let mut playlist_items = Vec::new();

    loop {
        match client
            .playlist_items()
            .list(&["snippet".to_string()].to_vec())
            .max_results(100)
            .page_token(&curr_page_token)
            .add_scope(youtube3::api::Scope::Readonly)
            .playlist_id(playlist_id)
            .doit()
            .await
        {
            Ok(result) => {
                playlist_items.append(&mut result.1.items.unwrap());
                match result.1.next_page_token {
                    Some(token) => curr_page_token = token,
                    None => break,
                }
            }
            Err(e) => println!("Error: {:?}", e),
        }
    }

    possible_playlist.map(|playlist| PlaylistWrapper {
        playlist,
        items: playlist_items,
    })
}
