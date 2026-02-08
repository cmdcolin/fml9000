## fml9000

A music player written in Rust with GTK4-rs

## Features/concepts

- Not MPD based (could be a good of bad thing, depending on your point of view)
- Inspired by foobar2000
- Implemented in gtk4-rs
- Plays youtube videos embedded in app
- Play audio with rust `rodio` library
- Add all videos from a youtube channel your library (motivated by https://cmdcolin.github.io/ytshuffle/)
- Recently added auto playlist
- Recently played auto playlist
- Playback queue auto playlist
- Show embededed art or folder art
- Keep track metadata in sqlite database with diesel
- Four 'quadrant' view
- Optionally auto-scan one or more folders
- Also has experimental TUI mode for the terminal, reads some diesal database and plays youtube videos with mpv,




## Screenshot

![](img/1.png)
GUI with GTK

![](img/2.png)
TUI for the terminal, uses same database


## Usage

```
git clone https://github.com/cmdcolin/fml9000
cd fml9000
cargo run -p fml9000-tui  # terminal mode
cargo run -p fml9000-gtk  # gui mode
```


## Notes

Still a very alpha quality software work in progress


