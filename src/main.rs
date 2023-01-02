mod facet_box;
mod grid_cell;
mod gtk_helpers;
mod header_bar;
mod load_css;
mod preferences_dialog;
mod settings;

use facet_box::create_facet_box;
use fml9000::models::Track;
use fml9000::{
  add_track_to_recently_played, load_facet_store, load_playlist_store, load_tracks, run_scan,
};
use grid_cell::Entry;
use gtk::gdk;
use gtk::gio::{self, ListStore, SimpleAction};
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  Application, ApplicationWindow, ColumnView, ColumnViewColumn, CustomFilter, GestureClick, Image,
  KeyvalTrigger, MultiSelection, Notebook, Orientation, Paned, PopoverMenu, ScrolledWindow,
  Shortcut, ShortcutAction, SignalListItemFactory, SingleSelection,
};
use gtk_helpers::{
  create_widget, get_cell, get_playlist_activate_selection, setup_col, str_or_unknown,
};
use header_bar::create_header_bar;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::cell::{Ref, RefCell};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;

struct Playlist {
  name: String,
}

const APP_ID: &str = "com.github.fml9000";

fn main() {
  let app = Application::builder().application_id(APP_ID).build();
  let (_stream, stream_handle) = OutputStream::try_default().unwrap();

  let stream_handle_rc = Rc::new(stream_handle);
  app.connect_activate(move |application| {
    app_main(&application, &stream_handle_rc);
  });
  app.run();
}

fn app_main(application: &Application, stream_handle: &Rc<OutputStreamHandle>) {
  let wnd = ApplicationWindow::builder()
    .default_width(1200)
    .default_height(600)
    .application(application)
    .title("fml9000")
    .build();

  let wnd_rc = Rc::new(wnd);
  let wnd_rc1 = wnd_rc.clone();
  let stream_handle_clone = stream_handle.clone();
  let sink_refcell_rc = Rc::new(RefCell::new(Sink::try_new(&stream_handle).unwrap()));
  let sink_refcell_rc1 = sink_refcell_rc.clone();

  let settings_rc = Rc::new(RefCell::new(crate::settings::read_settings()));

  load_css::load_css();

  let filter = CustomFilter::new(|_| true);

  let playlist_store = ListStore::new(BoxedAnyObject::static_type());
  let playlist_mgr_store = ListStore::new(BoxedAnyObject::static_type());

  let playlist_sel = MultiSelection::new(Some(&playlist_store));
  let playlist_columnview = ColumnView::builder().model(&playlist_sel).build();

  let source = gtk::DragSource::new();
  source.connect_drag_begin(|_, _| {
    println!("k1");
  });

  source.connect_drag_end(|_, _, _| {
    println!("k2");
  });

  playlist_columnview.add_controller(&source);

  let playlist_mgr_sel = SingleSelection::builder()
    .model(&playlist_mgr_store)
    .build();

  let album_art = Image::builder().vexpand(true).build();

  let album_art_rc = Rc::new(album_art);
  let album_art_rc1 = album_art_rc.clone();

  let playlist_sel_rc = Rc::new(playlist_sel);
  let playlist_sel_rc1 = playlist_sel_rc.clone();

  let playlist_store_rc = Rc::new(playlist_store);
  let playlist_mgr_columnview = ColumnView::builder().model(&playlist_mgr_sel).build();

  let artistalbum = SignalListItemFactory::new();
  let title = SignalListItemFactory::new();
  let filename = SignalListItemFactory::new();
  let track = SignalListItemFactory::new();
  let playlist_mgr = SignalListItemFactory::new();

  let pauseplay_action = SimpleAction::new("pauseplay", None);
  pauseplay_action.connect_activate(|a, b| {
    println!("pauseplay {:?} {:?}", a, b);
  });
  wnd_rc.add_action(&pauseplay_action);

  let pauseplay_shortcut = ShortcutAction::parse_string("action(win.pauseplay)").unwrap();
  pauseplay_action.connect_activate(|_, _| {});
  let trigger = KeyvalTrigger::new(gtk::gdk::Key::space, gtk::gdk::ModifierType::empty());
  let shortcut = Shortcut::builder()
    .trigger(&trigger)
    .action(&pauseplay_shortcut)
    .build();
  let shortcut_controller = gtk::ShortcutController::new();
  shortcut_controller.add_shortcut(&shortcut);
  shortcut_controller.connect_scope_notify(|_| {
    println!("here");
  });

  shortcut_controller.connect_mnemonic_modifiers_notify(|_| {
    println!("here2");
  });
  wnd_rc.add_controller(&shortcut_controller);

  let playlist_col1 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(400)
    .title("Album / Artist")
    .factory(&artistalbum)
    .build();

  let playlist_col2 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("#")
    .fixed_width(20)
    .factory(&track)
    .build();

  let playlist_col3 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("Title")
    .fixed_width(300)
    .factory(&title)
    .build();

  let playlist_col4 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(2000)
    .title("Filename")
    .factory(&filename)
    .build();

  let playlist_mgr_col = ColumnViewColumn::builder()
    .title("Playlists")
    .factory(&playlist_mgr)
    .expand(true)
    .build();

  playlist_columnview.append_column(&playlist_col1);
  playlist_columnview.append_column(&playlist_col2);
  playlist_columnview.append_column(&playlist_col3);
  playlist_columnview.append_column(&playlist_col4);
  playlist_mgr_columnview.append_column(&playlist_mgr_col);

  let action1 = SimpleAction::new("add_to_playlist", None);
  action1.connect_activate(|_, _| {
    // println!("hello2 {:?} {:?}", a1, args);
  });
  wnd_rc.add_action(&action1);
  let action2 = SimpleAction::new("properties", None);
  action2.connect_activate(|_, _| {
    // println!("hello {:?} {:?}", a1, args);
  });
  wnd_rc.add_action(&action2);

  let menu = gio::Menu::new();
  menu.append(Some("Add to new playlist"), Some("win.add_to_playlist"));
  menu.append(Some("Properties"), Some("win.properties"));
  let popover_menu = PopoverMenu::builder().build();
  popover_menu.set_menu_model(Some(&menu));
  popover_menu.set_has_arrow(false);
  let popover_menu_rc = Rc::new(popover_menu);
  let popover_menu_rc1 = popover_menu_rc.clone();
  let gesture = GestureClick::new();
  gesture.set_button(gdk::ffi::GDK_BUTTON_SECONDARY as u32);
  gesture.connect_released(move |gesture, _, x, y| {
    gesture.set_state(gtk::EventSequenceState::Claimed);
    let _selection = playlist_sel_rc1.selection();

    popover_menu_rc1.popup();
    popover_menu_rc1.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 0, 0)));
  });

  playlist_columnview.connect_activate(move |columnview, pos| {
    let selection = columnview.model().unwrap();
    let item = get_playlist_activate_selection(&selection, pos);
    let r: Ref<Rc<Track>> = item.borrow();
    let f1 = r.filename.clone();
    let f2 = r.filename.clone();
    let f3 = r.filename.clone();

    let file = BufReader::new(File::open(f1).unwrap());
    let source = Decoder::new(file).unwrap();

    let mut sink = sink_refcell_rc.borrow_mut();
    if !sink.empty() {
      sink.stop();
    }

    // kill and recreate sink, xref
    // https://github.com/betta-cyber/netease-music-tui/pull/27/
    // https://github.com/RustAudio/rodio/issues/315
    *sink = rodio::Sink::try_new(&stream_handle_clone).unwrap();
    sink.append(source);
    sink.play();

    add_track_to_recently_played(&f3);

    let mut p = PathBuf::from(f2);
    p.pop();
    p.push("cover.jpg");
    album_art_rc1.set_from_file(Some(p));

    wnd_rc1.set_title(Some(&format!(
      "fml9000 // {} - {} - {}",
      str_or_unknown(&r.artist),
      str_or_unknown(&r.album),
      str_or_unknown(&r.title),
    )));
  });

  let rows_rc = Rc::new(load_tracks());
  let rows_rc1 = rows_rc.clone();
  let rows_rc2 = rows_rc.clone();

  use std::time::Instant;
  let now = Instant::now();

  {
    let s = settings_rc.borrow();
    match &s.folder {
      Some(folder) => {
        run_scan(&folder, &rows_rc2);
      }
      None => {}
    }
  }

  let elapsed = now.elapsed();
  println!("Elapsed: {:.2?}", elapsed);

  let facet_store = ListStore::new(BoxedAnyObject::static_type());
  load_playlist_store(rows_rc.iter(), &playlist_store_rc);
  load_facet_store(&rows_rc1, &facet_store);

  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently added".to_string(),
  }));
  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently played".to_string(),
  }));

  artistalbum.connect_setup(move |_factory, item| setup_col(item));
  artistalbum.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let r: Ref<Rc<Track>> = obj.borrow();
    cell.set_entry(&Entry {
      name: format!(
        "{} // {}",
        str_or_unknown(&r.album),
        str_or_unknown(&r.artist),
      ),
    });
  });

  track.connect_setup(move |_factory, item| setup_col(item));
  track.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let r: Ref<Rc<Track>> = obj.borrow();
    cell.set_entry(&Entry {
      name: format!("{}", r.track.as_ref().unwrap_or(&"".to_string()),),
    });
  });

  title.connect_setup(move |_factory, item| setup_col(item));
  title.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let r: Ref<Rc<Track>> = obj.borrow();
    cell.set_entry(&Entry {
      name: format!("{}", r.title.as_ref().unwrap_or(&"".to_string())),
    });
  });

  filename.connect_setup(move |_factory, item| setup_col(item));
  filename.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let r: Ref<Rc<Track>> = obj.borrow();
    cell.set_entry(&Entry {
      name: r.filename.to_string(),
    });
  });

  playlist_mgr.connect_setup(move |_factory, item| setup_col(item));
  playlist_mgr.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let r: Ref<Playlist> = obj.borrow();
    cell.set_entry(&Entry {
      name: r.name.to_string(),
    });
  });

  let playlist_wnd = ScrolledWindow::builder()
    .child(&playlist_columnview)
    .build();

  let playlist_mgr_wnd = ScrolledWindow::builder()
    .child(&playlist_mgr_columnview)
    .build();

  let facet_box = create_facet_box(&playlist_store_rc, &facet_store, &filter, &rows_rc);

  let ltopbottom = Paned::builder()
    .vexpand(true)
    .orientation(Orientation::Vertical)
    .start_child(&facet_box)
    .end_child(&playlist_wnd)
    .build();

  let rtopbottom = Paned::builder()
    .vexpand(true)
    .orientation(Orientation::Vertical)
    .start_child(&playlist_mgr_wnd)
    .end_child(&*album_art_rc)
    .build();

  let lrpane = Paned::builder()
    .hexpand(true)
    .orientation(Orientation::Horizontal)
    .start_child(&ltopbottom)
    .end_child(&rtopbottom)
    .build();

  let main_ui = gtk::Box::new(Orientation::Vertical, 0);
  let rss_ui = gtk::Box::new(Orientation::Vertical, 0);

  let button_box = create_header_bar(settings_rc, sink_refcell_rc1, &wnd_rc);

  main_ui.append(&button_box);
  main_ui.append(&lrpane);
  main_ui.add_controller(&gesture);
  popover_menu_rc.set_parent(&main_ui);
  let notebook = Notebook::new();
  let lab1 = create_widget("Library");
  let lab2 = create_widget("RSS");
  notebook.append_page(&main_ui, Some(&lab1));
  notebook.append_page(&rss_ui, Some(&lab2));
  wnd_rc.set_child(Some(&notebook));
  wnd_rc.show();
}
