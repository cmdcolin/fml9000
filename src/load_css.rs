use gdk::Display;
use gtk::{gdk, CssProvider};

pub fn load_css() {
  // Load the CSS file and add it to the provider
  let provider = CssProvider::new();
  provider.load_from_data(&String::from_utf8_lossy(include_bytes!("style.css")));

  // Add the provider to the default screen
  gtk::style_context_add_provider_for_display(
    &Display::default().expect("Could not connect to a display."),
    &provider,
    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
  );
}
