use youtube3::YouTube;

pub async fn request_playlists(
    client: &YouTube,
    playlist_id: &str,
) -> Vec<youtube3::api::PlaylistItem> {
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

    playlist_items
}
