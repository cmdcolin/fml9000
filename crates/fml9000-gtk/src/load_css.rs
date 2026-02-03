use gdk::Display;
use gtk::{gdk, CssProvider};

pub fn load_css() {
  let Some(display) = Display::default() else {
    eprintln!("Warning: Could not connect to a display, skipping CSS loading");
    return;
  };

  let provider = CssProvider::new();
  provider.load_from_string(&String::from_utf8_lossy(include_bytes!("style.css")));

  gtk::style_context_add_provider_for_display(
    &display,
    &provider,
    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
  );
}
