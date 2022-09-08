## fml9000

A music player written in Rust with GTK4-rs

## Usage

```
git clone https://github.com/cmdcolin/fml9000
cd fml9000
cargo run
```

## Troubleshooting

Usage with Linuxbrew may not work, I had to completely uninstall linuxbrew to
make the development work on my computer. A minimal homebrew/linuxbrew install
may work but certain pacakages may confuse pkg-config too much. See
https://github.com/tauri-apps/tauri/issues/3856

## License

Most code is MIT except files specifically marked as otherwise (some code from
symphonia, marked MPL 2.0)
