export interface TrackItem {
  kind?: string;
  f?: string;
  video_id?: string;
  video_db_id?: number;
  t?: string;
  ar?: string;
  al?: string;
  aa?: string;
  tr?: string;
  g?: string;
  d?: number;
  pc: number;
  lp?: string;
  ad?: string;
}

export interface PlaybackState {
  playing: boolean;
  paused: boolean;
  position_secs: number;
  duration_secs: number | null;
  current_index: number | null;
  current_track: {
    title: string;
    artist: string;
    album: string;
    duration_str: string;
  } | null;
  shuffle_enabled: boolean;
  repeat_mode: string;
  volume: number;
}

export interface NavItem {
  id: string;
  db_id: number | null;
  label: string;
  kind: string;
}

export interface SidebarData {
  auto_playlists: NavItem[];
  user_playlists: NavItem[];
  youtube_channels: NavItem[];
}

export interface AlbumItem {
  artist: string;
  album: string;
  representative_filename: string;
}

export interface PlaylistInfo {
  id: number;
  name: string;
  created_at: string;
}
