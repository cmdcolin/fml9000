mod database;
mod grid_cell;
mod load_css;
mod preferences_dialog;
mod settings;

use database::{Facet, Track};
use grid_cell::{Entry, GridCell};
use gtk::gdk;
use gtk::gio::{self, ListStore, SimpleAction};
use gtk::glib::{BoxedAnyObject, Object};
use gtk::prelude::*;
use gtk::{
  Adjustment, Application, ApplicationWindow, Button, ColumnView, ColumnViewColumn, CustomFilter,
  FilterListModel, GestureClick, Image, KeyvalTrigger, ListItem, MultiSelection, Orientation,
  Paned, PopoverMenu, Scale, ScrolledWindow, SearchEntry, SelectionModel, Shortcut, ShortcutAction,
  SignalListItemFactory, SingleSelection, VolumeButton,
};
use regex::Regex;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::cell::{Ref, RefCell};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;

struct Playlist {
  name: String,
}

fn str_or_unknown(str: &Option<String>) -> String {
  str.as_ref().unwrap_or(&"(Unknown)".to_string()).to_string()
}

fn setup_col(item: &Object) {
  item
    .downcast_ref::<ListItem>()
    .unwrap()
    .set_child(Some(&GridCell::new()));
}

fn get_cell(item: &Object) -> (GridCell, BoxedAnyObject) {
  let item = item.downcast_ref::<ListItem>().unwrap();
  let child = item.child().unwrap().downcast::<GridCell>().unwrap();
  let obj = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
  (child, obj)
}

fn get_selection(sel: &MultiSelection, pos: u32) -> BoxedAnyObject {
  sel.item(pos).unwrap().downcast::<BoxedAnyObject>().unwrap()
}

fn get_playlist_activate_selection(sel: &SelectionModel, pos: u32) -> BoxedAnyObject {
  sel.item(pos).unwrap().downcast::<BoxedAnyObject>().unwrap()
}

fn load_img(a: &[u8]) -> Image {
  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(a).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let img = Image::new();
  img.set_from_pixbuf(Some(&pixbuf));
  img
}

fn create_button(img: &Image) -> Button {
  Button::builder().child(img).build()
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
  let wnd_rc2 = wnd_rc.clone();
  let stream_handle_clone = stream_handle.clone();
  let sink_refcell_rc = Rc::new(RefCell::new(Sink::try_new(&stream_handle).unwrap()));
  let sink_refcell_rc1 = sink_refcell_rc.clone();
  let sink_refcell_rc2 = sink_refcell_rc.clone();
  let sink_refcell_rc3 = sink_refcell_rc.clone();
  let settings_rc = Rc::new(RefCell::new(crate::settings::read_settings()));

  load_css::load_css();

  let filter = CustomFilter::new(|_| true);
  let facet_store = ListStore::new(BoxedAnyObject::static_type());
  let facet_filter = FilterListModel::new(Some(&facet_store), Some(&filter));
  let facet_filter_rc = Rc::new(facet_filter);
  let facet_filter_rc2 = facet_filter_rc.clone();
  let facet_filter_rc3 = facet_filter_rc.clone();
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

  let facet_sel = MultiSelection::new(Some(&*facet_filter_rc3));
  let facet_columnview = ColumnView::builder().model(&facet_sel).build();

  let playlist_mgr_sel = SingleSelection::builder()
    .model(&playlist_mgr_store)
    .build();

  let album_art = Image::builder().vexpand(true).build();

  let album_art_rc = Rc::new(album_art);
  let album_art_rc1 = album_art_rc.clone();

  let playlist_sel_rc = Rc::new(playlist_sel);
  let playlist_sel_rc1 = playlist_sel_rc.clone();

  let facet_sel_rc = Rc::new(facet_sel);
  let facet_sel_rc1 = facet_sel_rc.clone();

  let playlist_store_rc = Rc::new(playlist_store);
  let playlist_store_rc1 = playlist_store_rc.clone();

  let playlist_mgr_columnview = ColumnView::builder().model(&playlist_mgr_sel).build();

  let artistalbum = SignalListItemFactory::new();
  let title = SignalListItemFactory::new();
  let filename = SignalListItemFactory::new();
  let track = SignalListItemFactory::new();
  let facet = SignalListItemFactory::new();
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

  let facet_col = ColumnViewColumn::builder()
    .title("Album Artist / Album")
    .factory(&facet)
    .expand(true)
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
  facet_columnview.append_column(&facet_col);
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

    crate::database::add_track_to_recently_played(&f3);

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

  let rows_rc = Rc::new(database::load_all().unwrap());
  let rows_rc1 = rows_rc.clone();
  let rows_rc2 = rows_rc.clone();

  {
    let s = settings_rc.borrow();
    match &s.folder {
      Some(folder) => {
        database::run_scan(&folder, &rows_rc2);
      }
      None => {}
    }
  }

  database::load_playlist_store(rows_rc.iter(), &playlist_store_rc);
  database::load_facet_store(&rows_rc1, &facet_store);

  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently added".to_string(),
  }));
  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently played".to_string(),
  }));

  facet_sel_rc.connect_selection_changed(move |_, _, _| {
    let selection = facet_sel_rc1.selection();
    match gtk::BitsetIter::init_first(&selection) {
      Some(result) => {
        let (iter, first_pos) = result;
        playlist_store_rc1.remove_all();
        let item = get_selection(&facet_sel_rc1, first_pos);
        let r: Ref<Facet> = item.borrow();
        let con = rows_rc
          .iter()
          .filter(|x| x.album_artist == r.album_artist && x.album == r.album);

        database::load_playlist_store(con, &playlist_store_rc);

        for pos in iter {
          let item = get_selection(&facet_sel_rc1, pos);
          let r: Ref<Facet> = item.borrow();
          let con = rows_rc
            .iter()
            .filter(|x| x.album_artist == r.album_artist && x.album == r.album);

          database::load_playlist_store(con, &playlist_store_rc);
        }
      }
      None => { /* empty selection */ }
    }
  });

  facet.connect_setup(|_factory, item| setup_col(item));
  facet.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let r: Ref<Facet> = obj.borrow();
    cell.set_entry(&Entry {
      name: if r.all {
        "(All)".to_string()
      } else {
        format!(
          "{} // {}",
          str_or_unknown(&r.album_artist),
          str_or_unknown(&r.album),
        )
      },
    });
  });

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

  let facet_wnd = ScrolledWindow::builder()
    .child(&facet_columnview)
    .vexpand(true)
    .build();

  let facet_box = gtk::Box::new(Orientation::Vertical, 0);
  let search_bar = SearchEntry::builder().build();

  search_bar.connect_search_changed(move |s| {
    let text = s.text();
    let re = Regex::new(&format!("(?i){}", regex::escape(text.as_str()))).unwrap();
    let filter = CustomFilter::new(move |obj| {
      let r = obj.downcast_ref::<BoxedAnyObject>().unwrap();
      let k: Ref<Facet> = r.borrow();
      let k1 = match &k.album {
        Some(s) => re.is_match(&s),
        None => false,
      };
      let k2 = match &k.album_artist {
        Some(s) => re.is_match(&s),
        None => false,
      };
      k1 || k2
    });
    facet_filter_rc2.set_filter(Some(&filter))
  });
  facet_box.append(&search_bar);
  facet_box.append(&facet_wnd);

  let playlist_wnd = ScrolledWindow::builder()
    .child(&playlist_columnview)
    .build();

  let playlist_mgr_wnd = ScrolledWindow::builder()
    .child(&playlist_mgr_columnview)
    .build();

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

  let prev_btn = create_button(&load_img(include_bytes!("img/prev.svg")));
  let stop_btn = create_button(&load_img(include_bytes!("img/stop.svg")));
  let next_btn = create_button(&load_img(include_bytes!("img/next.svg")));
  let pause_btn = create_button(&load_img(include_bytes!("img/pause.svg")));
  let play_btn = create_button(&load_img(include_bytes!("img/play.svg")));
  let settings_btn = create_button(&load_img(include_bytes!("img/settings.svg")));

  let button_box = gtk::Box::new(Orientation::Horizontal, 0);
  let seek_slider = Scale::builder()
    .hexpand(true)
    .orientation(Orientation::Horizontal)
    .adjustment(&Adjustment::new(0.0, 0.0, 1.0, 0.01, 0.0, 0.0))
    .build();

  let volume_button = VolumeButton::builder()
    .value({
      let s = settings_rc.borrow();
      s.volume
    })
    .build();
  let settings_rc1 = settings_rc.clone();
  volume_button.connect_value_changed(move |_, volume| {
    let sink = sink_refcell_rc1.borrow();
    let mut s = settings_rc1.borrow_mut();
    s.volume = volume;
    crate::settings::write_settings(&s).expect("Failed to write");
    sink.set_volume(volume as f32);
  });

  button_box.append(&settings_btn);
  button_box.append(&seek_slider);
  button_box.append(&play_btn);
  button_box.append(&pause_btn);
  button_box.append(&prev_btn);
  button_box.append(&next_btn);
  button_box.append(&stop_btn);
  button_box.append(&volume_button);

  pause_btn.connect_clicked(move |_| {
    let sink = sink_refcell_rc2.borrow();
    sink.pause();
  });

  play_btn.connect_clicked(move |_| {
    let sink = sink_refcell_rc3.borrow();
    sink.play();
  });

  settings_btn.connect_clicked(move |_| {
    gtk::glib::MainContext::default().spawn_local(crate::preferences_dialog::dialog(
      Rc::clone(&wnd_rc2),
      Rc::clone(&settings_rc),
    ));
  });

  let main_ui = gtk::Box::new(Orientation::Vertical, 0);
  main_ui.append(&button_box);
  main_ui.append(&lrpane);
  main_ui.add_controller(&gesture);
  popover_menu_rc.set_parent(&main_ui);
  wnd_rc.set_child(Some(&main_ui));
  wnd_rc.show();
}
