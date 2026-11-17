//! Now Playing: whatever app Windows lists as the current media session (Spotify, a YouTube tab in
//! Firefox, the Media Player...), read and controlled through Windows' global media controls.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// The app's id, e.g. "Spotify.exe" or Firefox's; shown as a short name.
    pub app: String,
    pub playing: bool,
    /// Seconds, when the app reports a timeline.
    pub position: Option<f64>,
    pub duration: Option<f64>,
    /// Changes when the track changes, so the UI knows to fetch new art.
    pub key: String,
    pub can_prev: bool,
    pub can_next: bool,
}

/// A readable app name from an AppUserModelID: "Spotify.exe" -> "Spotify",
/// "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify" -> "Spotify", "308046B0AF4A39CB" (Firefox) -> "Firefox".
pub fn app_name(aumid: &str) -> String {
    let lower = aumid.to_ascii_lowercase();
    for (needle, name) in [
        ("spotify", "Spotify"),
        ("firefox", "Firefox"),
        ("308046b0af4a39cb", "Firefox"),
        ("chrome", "Chrome"),
        ("msedge", "Edge"),
        ("zunemusic", "Media Player"),
        ("zunevideo", "Movies & TV"),
        ("vlc", "VLC"),
        ("applemusic", "Apple Music"),
        ("discord", "Discord"),
    ] {
        if lower.contains(needle) {
            return name.into();
        }
    }
    let base = aumid.rsplit(['!', '\\']).next().unwrap_or(aumid);
    base.trim_end_matches(".exe")
        .trim_end_matches(".EXE")
        .to_string()
}

#[cfg(windows)]
pub mod win {
    use super::*;
    use std::sync::Mutex;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Storage::Streams::{DataReader, InputStreamOptions};

    static MANAGER: Mutex<Option<Manager>> = Mutex::new(None);

    fn session() -> Option<Session> {
        let mut g = MANAGER.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            *g = Manager::RequestAsync().and_then(|a| a.join()).ok();
        }
        g.as_ref()?.GetCurrentSession().ok()
    }

    pub fn now() -> Option<NowPlaying> {
        let s = session()?;
        let props = s.TryGetMediaPropertiesAsync().and_then(|a| a.join()).ok()?;
        let info = s.GetPlaybackInfo().ok()?;
        let controls = info.Controls().ok();
        let timeline = s.GetTimelineProperties().ok();
        let secs = |t: windows::Foundation::TimeSpan| t.Duration as f64 / 10_000_000.0;
        let duration = timeline
            .as_ref()
            .and_then(|t| t.EndTime().ok())
            .map(secs)
            .filter(|d| *d > 0.0);
        let position = duration.and(timeline.as_ref().and_then(|t| t.Position().ok()).map(secs));
        let title = props.Title().map(|h| h.to_string()).unwrap_or_default();
        let artist = props.Artist().map(|h| h.to_string()).unwrap_or_default();
        if title.is_empty() && artist.is_empty() {
            return None;
        }
        let aumid = s
            .SourceAppUserModelId()
            .map(|h| h.to_string())
            .unwrap_or_default();
        Some(NowPlaying {
            key: format!("{aumid}\u{1}{title}\u{1}{artist}"),
            title,
            artist,
            album: props
                .AlbumTitle()
                .map(|h| h.to_string())
                .unwrap_or_default(),
            app: app_name(&aumid),
            playing: info.PlaybackStatus().ok() == Some(Status::Playing),
            position,
            duration,
            can_prev: controls
                .as_ref()
                .and_then(|c| c.IsPreviousEnabled().ok())
                .unwrap_or(true),
            can_next: controls
                .as_ref()
                .and_then(|c| c.IsNextEnabled().ok())
                .unwrap_or(true),
        })
    }

    /// The current track's cover as a data URL (apps give PNG or JPEG), or None.
    pub fn art() -> Option<String> {
        use base64::Engine;
        let s = session()?;
        let props = s.TryGetMediaPropertiesAsync().and_then(|a| a.join()).ok()?;
        let stream = props
            .Thumbnail()
            .ok()?
            .OpenReadAsync()
            .and_then(|a| a.join())
            .ok()?;
        let size = stream.Size().ok()?.min(4 << 20) as u32;
        if size == 0 {
            return None;
        }
        let reader = DataReader::CreateDataReader(&stream).ok()?;
        reader
            .SetInputStreamOptions(InputStreamOptions::ReadAhead)
            .ok()?;
        let got = reader.LoadAsync(size).and_then(|a| a.join()).ok()?;
        let mut buf = vec![0u8; got as usize];
        reader.ReadBytes(&mut buf).ok()?;
        let mime = if buf.starts_with(&[0x89, b'P', b'N', b'G']) {
            "image/png"
        } else {
            "image/jpeg"
        };
        Some(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&buf)
        ))
    }

    pub fn control(action: &str) -> Result<(), String> {
        let s = session().ok_or("nothing is playing")?;
        let op = match action {
            "toggle" => s.TryTogglePlayPauseAsync(),
            // Only ever pauses (the meeting automation must never start music).
            "pause" => s.TryPauseAsync(),
            "next" => s.TrySkipNextAsync(),
            "prev" => s.TrySkipPreviousAsync(),
            _ => return Err("unknown action".into()),
        };
        op.and_then(|a| a.join())
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn seek(seconds: f64) -> Result<(), String> {
        let s = session().ok_or("nothing is playing")?;
        s.TryChangePlaybackPositionAsync((seconds.max(0.0) * 10_000_000.0) as i64)
            .and_then(|a| a.join())
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_names() {
        assert_eq!(app_name("Spotify.exe"), "Spotify");
        assert_eq!(
            app_name("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
            "Spotify"
        );
        assert_eq!(app_name("308046B0AF4A39CB"), "Firefox");
        assert_eq!(
            app_name("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"),
            "Media Player"
        );
        assert_eq!(app_name("foobar2000.exe"), "foobar2000");
    }
}
