//! Explicit playback and setlist state machines. Playback is never modelled as scattered booleans.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlaybackState {
    Stopped,
    Loading,
    Ready,
    CountIn,
    Playing,
    Paused,
    Nexting,
    Error,
    Panic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackEvent {
    Load,
    Loaded,
    LoadFailed,
    Play,
    CountInDone,
    Pause,
    Stop,
    Ended,
    Next,
    Panic,
    Unload,
}

impl PlaybackState {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns the next state, or `None` if the transition is not allowed.
    pub fn transition(self, ev: PlaybackEvent) -> Option<PlaybackState> {
        use PlaybackEvent as E;
        use PlaybackState as S;
        match (self, ev) {
            // Panic is always allowed and always wins.
            (_, E::Panic) => Some(S::Panic),
            (_, E::Unload) => Some(S::Stopped),
            (S::Stopped | S::Ready | S::Paused | S::Error | S::Panic, E::Load) => Some(S::Loading),
            (S::Playing | S::CountIn, E::Next) => Some(S::Nexting),
            (S::Stopped | S::Ready | S::Paused | S::Error | S::Panic, E::Next) => Some(S::Nexting),
            (S::Nexting, E::Load) => Some(S::Loading),
            (S::Loading | S::Nexting, E::Loaded) => Some(S::Ready),
            (S::Loading | S::Nexting, E::LoadFailed) => Some(S::Error),
            (S::Ready | S::Paused | S::Stopped | S::Panic, E::Play) => Some(S::Playing),
            (S::CountIn, E::CountInDone) => Some(S::Playing),
            (S::Playing | S::CountIn, E::Pause) => Some(S::Paused),
            (S::Playing | S::CountIn | S::Paused | S::Ready, E::Stop) => Some(S::Ready),
            (S::Playing, E::Ended) => Some(S::Ready),
            (s, E::Stop) if s == S::Stopped || s == S::Panic || s == S::Error => Some(s),
            _ => None,
        }
    }

    pub fn is_audible(self) -> bool {
        matches!(self, PlaybackState::Playing | PlaybackState::CountIn)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SetlistState {
    SetlistReady,
    SongLoading,
    SongReady,
    SongPlaying,
    SongCompleted,
    Transitioning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetlistEvent {
    LoadSong,
    SongLoaded,
    Play,
    Stop,
    SongEnded,
    Advance,
}

impl SetlistState {
    pub fn transition(self, ev: SetlistEvent) -> SetlistState {
        use SetlistEvent as E;
        use SetlistState as S;
        match (self, ev) {
            (_, E::LoadSong) => S::SongLoading,
            (S::SongLoading | S::Transitioning, E::SongLoaded) => S::SongReady,
            (S::SongReady | S::SongCompleted, E::Play) => S::SongPlaying,
            (S::SongPlaying, E::Stop) => S::SongReady,
            (S::SongPlaying, E::SongEnded) => S::SongCompleted,
            (_, E::Advance) => S::Transitioning,
            (s, _) => s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PlaybackEvent as E;
    use super::PlaybackState as S;
    use super::*;

    #[test]
    fn happy_path() {
        let s = S::Stopped.transition(E::Load).unwrap();
        assert_eq!(s, S::Loading);
        let s = s.transition(E::Loaded).unwrap();
        assert_eq!(s, S::Ready);
        let s = s.transition(E::Play).unwrap();
        assert_eq!(s, S::Playing);
        let s = s.transition(E::Pause).unwrap();
        assert_eq!(s, S::Paused);
        let s = s.transition(E::Play).unwrap();
        assert_eq!(s.transition(E::Ended), Some(S::Ready));
    }

    #[test]
    fn panic_always_allowed_and_recoverable() {
        for st in [S::Stopped, S::Loading, S::Ready, S::Playing, S::Paused, S::Nexting, S::Error, S::CountIn] {
            assert_eq!(st.transition(E::Panic), Some(S::Panic));
        }
        assert_eq!(S::Panic.transition(E::Play), Some(S::Playing));
    }

    #[test]
    fn invalid_transitions_rejected() {
        assert_eq!(S::Loading.transition(E::Play), None);
        assert_eq!(S::Stopped.transition(E::Pause), None);
        assert_eq!(S::Stopped.transition(E::Loaded), None);
    }

    #[test]
    fn setlist_flow() {
        use SetlistEvent as SE;
        use SetlistState as SS;
        let s = SS::SetlistReady.transition(SE::LoadSong);
        let s = s.transition(SE::SongLoaded);
        assert_eq!(s, SS::SongReady);
        let s = s.transition(SE::Play).transition(SE::SongEnded);
        assert_eq!(s, SS::SongCompleted);
        assert_eq!(s.transition(SE::Advance), SS::Transitioning);
    }
}
