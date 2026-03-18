use crate::settings::RepeatMode;
use rand::Rng;

pub enum NextTrackResult {
  PlayIndex(usize),
  Stop,
}

pub fn compute_next_index(
  current_index: Option<usize>,
  playlist_len: usize,
  shuffle_enabled: bool,
  repeat_mode: RepeatMode,
) -> NextTrackResult {
  if playlist_len == 0 {
    return NextTrackResult::Stop;
  }

  if repeat_mode == RepeatMode::One {
    if let Some(idx) = current_index {
      return NextTrackResult::PlayIndex(idx);
    }
  }

  if shuffle_enabled {
    let mut rng = rand::rng();
    if playlist_len == 1 {
      return NextTrackResult::PlayIndex(0);
    }
    loop {
      let idx = rng.random_range(0..playlist_len);
      if Some(idx) != current_index {
        return NextTrackResult::PlayIndex(idx);
      }
    }
  }

  match current_index {
    Some(idx) => {
      if idx + 1 < playlist_len {
        NextTrackResult::PlayIndex(idx + 1)
      } else if repeat_mode == RepeatMode::All {
        NextTrackResult::PlayIndex(0)
      } else {
        NextTrackResult::Stop
      }
    }
    None => NextTrackResult::PlayIndex(0),
  }
}

pub fn compute_prev_index(
  current_index: Option<usize>,
  playlist_len: usize,
) -> NextTrackResult {
  if playlist_len == 0 {
    return NextTrackResult::Stop;
  }

  match current_index {
    Some(idx) => {
      if idx > 0 {
        NextTrackResult::PlayIndex(idx - 1)
      } else {
        NextTrackResult::PlayIndex(playlist_len - 1)
      }
    }
    None => NextTrackResult::PlayIndex(0),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn next_empty_playlist_stops() {
    match compute_next_index(None, 0, false, RepeatMode::Off) {
      NextTrackResult::Stop => {}
      NextTrackResult::PlayIndex(_) => panic!("expected Stop"),
    }
  }

  #[test]
  fn next_sequential_advances() {
    match compute_next_index(Some(2), 10, false, RepeatMode::Off) {
      NextTrackResult::PlayIndex(3) => {}
      other => panic!("expected PlayIndex(3), got {:?}", match other {
        NextTrackResult::PlayIndex(i) => format!("PlayIndex({i})"),
        NextTrackResult::Stop => "Stop".to_string(),
      }),
    }
  }

  #[test]
  fn next_at_end_stops_when_repeat_off() {
    match compute_next_index(Some(9), 10, false, RepeatMode::Off) {
      NextTrackResult::Stop => {}
      NextTrackResult::PlayIndex(i) => panic!("expected Stop, got PlayIndex({i})"),
    }
  }

  #[test]
  fn next_at_end_wraps_when_repeat_all() {
    match compute_next_index(Some(9), 10, false, RepeatMode::All) {
      NextTrackResult::PlayIndex(0) => {}
      other => panic!("expected PlayIndex(0), got {:?}", match other {
        NextTrackResult::PlayIndex(i) => format!("PlayIndex({i})"),
        NextTrackResult::Stop => "Stop".to_string(),
      }),
    }
  }

  #[test]
  fn next_repeat_one_replays() {
    match compute_next_index(Some(5), 10, false, RepeatMode::One) {
      NextTrackResult::PlayIndex(5) => {}
      other => panic!("expected PlayIndex(5), got {:?}", match other {
        NextTrackResult::PlayIndex(i) => format!("PlayIndex({i})"),
        NextTrackResult::Stop => "Stop".to_string(),
      }),
    }
  }

  #[test]
  fn next_shuffle_picks_different_index() {
    for _ in 0..20 {
      match compute_next_index(Some(3), 10, true, RepeatMode::Off) {
        NextTrackResult::PlayIndex(idx) => assert_ne!(idx, 3),
        NextTrackResult::Stop => panic!("expected PlayIndex"),
      }
    }
  }

  #[test]
  fn next_shuffle_single_item() {
    match compute_next_index(Some(0), 1, true, RepeatMode::Off) {
      NextTrackResult::PlayIndex(0) => {}
      other => panic!("expected PlayIndex(0), got {:?}", match other {
        NextTrackResult::PlayIndex(i) => format!("PlayIndex({i})"),
        NextTrackResult::Stop => "Stop".to_string(),
      }),
    }
  }

  #[test]
  fn prev_wraps_at_start() {
    match compute_prev_index(Some(0), 10) {
      NextTrackResult::PlayIndex(9) => {}
      other => panic!("expected PlayIndex(9), got {:?}", match other {
        NextTrackResult::PlayIndex(i) => format!("PlayIndex({i})"),
        NextTrackResult::Stop => "Stop".to_string(),
      }),
    }
  }

  #[test]
  fn prev_goes_back() {
    match compute_prev_index(Some(5), 10) {
      NextTrackResult::PlayIndex(4) => {}
      other => panic!("expected PlayIndex(4), got {:?}", match other {
        NextTrackResult::PlayIndex(i) => format!("PlayIndex({i})"),
        NextTrackResult::Stop => "Stop".to_string(),
      }),
    }
  }

  #[test]
  fn prev_empty_playlist_stops() {
    match compute_prev_index(None, 0) {
      NextTrackResult::Stop => {}
      NextTrackResult::PlayIndex(_) => panic!("expected Stop"),
    }
  }
}
