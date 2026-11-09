//! What Lidarr knows that the library may not have: its catalogue, an artist's albums, an album's
//! tracks. Virgil's albums and Lidarr's are matched by path, since Lidarr files the music into the
//! folder MPD plays from.

use std::collections::HashMap;

use anyhow::{Context, anyhow};
use virgil_core::{ArtistCatalog, CatalogAlbum, CatalogArtist, CatalogResults, CatalogTrack};
use virgil_fetch::lidarr::{Album, Track};
use virgil_fetch::{paths, pick};

use crate::downloads::{Fetcher, Host};

/// Artists and albums in Lidarr's catalogue matching `term`, albums in the library linked to Virgil's.
pub async fn search(fetcher: &Fetcher, host: &impl Host, term: &str) -> anyhow::Result<CatalogResults> {
    let (artists, albums) = tokio::join!(fetcher.lidarr.lookup_artists(term), albums(fetcher, host, term));
    let artists = artists?
        .into_iter()
        .map(|a| CatalogArtist {
            id: a.foreign_artist_id,
            name: a.artist_name,
            disambiguation: a.disambiguation,
            known: a.id > 0,
        })
        .collect();
    Ok(CatalogResults { artists, albums: albums? })
}

/// Every album Lidarr has of an artist (MusicBrainz id), newest first. With `add`, an artist Lidarr
/// doesn't follow is added first, wanting nothing, so that it reads their albums; that takes it a
/// moment, so the list may come back empty at first.
pub async fn discography(fetcher: &Fetcher, host: &impl Host, id: &str, add: bool) -> anyhow::Result<ArtistCatalog> {
    let mut artist = fetcher.artists().await?.into_iter().find(|a| a.foreign_artist_id == id);
    if artist.is_none() && add {
        let lookup =
            fetcher.lidarr.lookup_artist(id).await?.ok_or_else(|| anyhow!("Lidarr doesn't know that artist"))?;
        let placement = fetcher.placement().await?;
        artist = Some(fetcher.lidarr.add_artist(&virgil_fetch::lidarr::new_artist(&lookup, &placement)).await?);
        fetcher.artists_changed().await;
    }
    let Some(artist) = artist else { return Ok(ArtistCatalog::default()) };
    let mut found = fetcher.lidarr.albums_of(artist.id).await?;
    found.sort_by(|a, b| b.release_date.cmp(&a.release_date));
    let mut albums: Vec<CatalogAlbum> = found
        .iter()
        .map(|a| {
            let mut entry = entry(a, a.statistics.map_or((0, 0), |s| (s.track_file_count, s.track_count)));
            entry.artist = artist.artist_name.clone();
            entry
        })
        .collect();
    link(fetcher, host, &mut albums, &found).await?;
    Ok(ArtistCatalog { known: true, albums })
}

/// Albums in Lidarr's catalogue matching `term`, those in the library linked to Virgil's.
async fn albums(fetcher: &Fetcher, host: &impl Host, term: &str) -> anyhow::Result<Vec<CatalogAlbum>> {
    let found = fetcher.lidarr.lookup(term).await?;
    let ids: Vec<u32> = found.iter().map(|a| a.id).filter(|&id| id > 0).collect();
    let known: HashMap<u32, Album> = fetcher.lidarr.albums(&ids).await?.into_iter().map(|a| (a.id, a)).collect();
    let mut albums: Vec<CatalogAlbum> = found
        .iter()
        .map(|album| {
            // Lookups carry no statistics; Lidarr's own copy does.
            let stats = known.get(&album.id).and_then(|a| a.statistics);
            entry(album, stats.map_or((0, 0), |s| (s.track_file_count, s.track_count)))
        })
        .collect();
    link(fetcher, host, &mut albums, &found).await?;
    Ok(albums)
}

/// The albums Lidarr has of an artist that the library has none of, newest first. `folder` is the
/// artist's folder in the music directory, when Virgil has any of their music.
pub async fn artist(fetcher: &Fetcher, name: &str, folder: Option<&str>) -> anyhow::Result<ArtistCatalog> {
    let root = fetcher.placement().await?.root;
    let artists = fetcher.artists().await?;
    let wanted = pick::words(name);
    let by_path = folder.map(|f| format!("{root}/{f}"));
    let artist = artists
        .iter()
        .find(|a| by_path.as_deref().is_some_and(|p| a.path.trim_end_matches('/') == p))
        .or_else(|| artists.iter().find(|a| pick::words(&a.artist_name) == wanted));
    let Some(artist) = artist else { return Ok(ArtistCatalog::default()) };
    let mut albums: Vec<Album> = fetcher
        .lidarr
        .albums_of(artist.id)
        .await?
        .into_iter()
        .filter(|a| a.statistics.is_some_and(|s| s.track_file_count == 0))
        .collect();
    albums.sort_by(|a, b| b.release_date.cmp(&a.release_date));
    let albums = albums
        .iter()
        .map(|a| {
            let mut entry = entry(a, a.statistics.map_or((0, 0), |s| (0, s.track_count)));
            entry.artist = artist.artist_name.clone();
            entry
        })
        .collect();
    Ok(ArtistCatalog { known: true, albums })
}

/// One album with its tracks. Lidarr only knows the tracks of albums it has, so with `add` an album
/// it doesn't have is added (and its artist, if need be) without wanting anything, just to read them.
pub async fn album(fetcher: &Fetcher, host: &impl Host, id: &str, add: bool) -> anyhow::Result<CatalogAlbum> {
    let lookup = fetcher.lidarr.lookup_album(id).await?.ok_or_else(|| anyhow!("Lidarr doesn't know that album"))?;
    let mut album: Album = serde_json::from_value(lookup.clone()).context("reading Lidarr's album")?;
    if album.id == 0 && add {
        let placement = fetcher.placement().await?;
        let added = fetcher.lidarr.add_album(&virgil_fetch::lidarr::new_album(&lookup, &placement, false)).await?;
        fetcher.artists_changed().await;
        album.id = added.id;
    }
    with_tracks(fetcher, host, &album).await
}

/// The album of Virgil's that `uri` is in, as Lidarr has it, with every track and which are missing.
pub async fn album_of(fetcher: &Fetcher, host: &impl Host, uri: &str) -> anyhow::Result<Option<CatalogAlbum>> {
    let root = fetcher.placement().await?.root;
    let path = format!("{root}/{uri}");
    let artists = fetcher.artists().await?;
    let Some(artist) = artists.iter().find(|a| path.starts_with(&format!("{}/", a.path.trim_end_matches('/')))) else {
        return Ok(None);
    };
    let files = fetcher.lidarr.artist_files(artist.id).await?;
    let Some(file) = files.iter().find(|f| f.path == path) else { return Ok(None) };
    let mut album = fetcher.lidarr.album(file.album_id).await?;
    album.artist.get_or_insert_with(|| artist.clone());
    with_tracks(fetcher, host, &album).await.map(Some)
}

async fn with_tracks(fetcher: &Fetcher, host: &impl Host, album: &Album) -> anyhow::Result<CatalogAlbum> {
    if album.id == 0 {
        return Ok(entry(album, (0, 0)));
    }
    let tracks = fetcher.lidarr.tracks(album.id).await?;
    let have = tracks.iter().filter(|t| t.has_file).count() as u32;
    let mut entry = entry(album, (have, tracks.len() as u32));
    entry.tracks = tracks.iter().map(track).collect();
    entry.tracks.sort_by_key(|t| (t.disc, t.number));
    let found = [album.clone()];
    link(fetcher, host, std::slice::from_mut(&mut entry), &found).await?;
    Ok(entry)
}

/// Links albums with music in the library to Virgil's album, through one of their files.
async fn link(
    fetcher: &Fetcher,
    host: &impl Host,
    entries: &mut [CatalogAlbum],
    albums: &[Album],
) -> anyhow::Result<()> {
    let ids: Vec<u32> =
        entries.iter().zip(albums).filter(|(e, _)| e.have > 0).map(|(_, a)| a.id).filter(|&id| id > 0).collect();
    if ids.is_empty() {
        return Ok(());
    }
    let root = fetcher.placement().await?.root;
    let files = fetcher.lidarr.track_files(&ids).await?;
    for (entry, album) in entries.iter_mut().zip(albums) {
        let file = files.iter().find(|f| f.album_id == album.id && album.id > 0);
        entry.album_id = file.and_then(|f| paths::in_library(&root, &f.path)).and_then(|uri| host.album_of(&uri));
    }
    Ok(())
}

fn entry(album: &Album, (have, total): (u32, u32)) -> CatalogAlbum {
    CatalogAlbum {
        id: album.foreign_album_id.clone(),
        title: album.title.clone(),
        artist: album.artist_name().to_string(),
        year: album.year(),
        kind: album.kind(),
        cover: album.cover(),
        have,
        total,
        wanted: album.id > 0 && album.monitored && have < total,
        album_id: None,
        tracks: Vec::new(),
    }
}

fn track(track: &Track) -> CatalogTrack {
    CatalogTrack {
        id: track.id,
        title: track.title.clone(),
        disc: track.medium_number.max(1),
        number: track.number(),
        duration: track.duration as f64 / 1000.0,
        have: track.has_file,
    }
}
