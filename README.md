## fml9000

A music player written in Rust with GTK4-rs

## Screenshot

![](img/1.png)

Note: not all features pictured work yet, but it does look like this :)

## Usage

```
git clone https://github.com/cmdcolin/fml9000
cd fml9000
cargo run
```

## Troubleshooting

### Linuxbrew can mess with PKG_CONFIG_PATH

Usage with Linuxbrew may cause issues with PKG_CONFIG_PATH, I had to completely
uninstall linuxbrew to make the development work on my computer.

A minimal homebrew/linuxbrew install may work but certain packages may confuse
pkg-config. See https://github.com/tauri-apps/tauri/issues/3856

You may be able to set PKG_CONFIG_PATH env variable to avoid error

```
error: failed to run custom build command for `alsa-sys v0.3.1`
 pkg-config exited with status code 1
  > PKG_CONFIG_ALLOW_SYSTEM_LIBS=1 PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1 pkg-config --libs --cflags alsa

  The system library `alsa` required by crate `alsa-sys` was not found.
  The file `alsa.pc` needs to be installed and the PKG_CONFIG_PATH environment variable must contain its parent directory.
  The PKG_CONFIG_PATH environment variable is not set.

  HINT: if you have installed the library, try setting PKG_CONFIG_PATH to the directory containing `alsa.pc`.

```

So I use `PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig/ cargo run` which
is a directory containing alsa.pc on my machine to build
