mod database;
mod grid_cell;
mod load_css;

use crate::grid_cell::{Entry, GridCell};
use database::{Facet, Track};
use gtk::gdk;
use gtk::gio::{self, ListStore};
use gtk::glib::{self, BoxedAnyObject};
use gtk::prelude::*;
use gtk::{
  Application, ApplicationWindow, Button, ColumnView, ColumnViewColumn, FileChooserAction,
  FileChooserDialog, GestureClick, Image, ListItem, MultiSelection, Orientation, Paned,
  PopoverMenu, ResponseType, Scale, ScrolledWindow, SearchEntry, SelectionModel,
  SignalListItemFactory, SingleSelection, VolumeButton,
};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::cell::Ref;
use std::cell::RefCell;
use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;

struct Playlist {
  name: String,
}

fn str_or_unknown(str: &Option<String>) -> String {
  str.as_ref().unwrap_or(&"(Unknown)".to_string()).to_string()
}

fn setup_col(item: &ListItem) {
  item
    .downcast_ref::<ListItem>()
    .unwrap()
    .set_child(Some(&GridCell::new()));
}

fn get_cell(item: &ListItem) -> (GridCell, BoxedAnyObject) {
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

fn app_main(application: &gtk::Application, stream_handle: &Rc<OutputStreamHandle>) {
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

  // database::run_scan();
  load_css::load_css();

  let facet_store = ListStore::new(BoxedAnyObject::static_type());
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

  let facet_sel = MultiSelection::new(Some(&facet_store));
  let facet_columnview = ColumnView::builder().model(&facet_sel).build();

  let playlist_mgr_sel = SingleSelection::builder()
    .model(&playlist_mgr_store)
    .build();

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

  let playlist_col1 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(400)
    .title("Artist / Album")
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
    .title("X")
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

  let action1 = gio::SimpleAction::new("add_to_playlist", None);
  action1.connect_activate(|_, _| {
    // println!("hello2 {:?} {:?}", a1, args);
  });
  wnd_rc.add_action(&action1);
  let action2 = gio::SimpleAction::new("properties", None);
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
    let f = r.filename.clone();

    println!("{}", f);
    let file = BufReader::new(File::open(f).unwrap());
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

    wnd_rc1.set_title(Some(&format!(
      "fml9000 // {} - {} - {}",
      str_or_unknown(&r.artist),
      str_or_unknown(&r.album),
      str_or_unknown(&r.title),
    )));
  });

  let rows = Rc::new(database::load_all().unwrap());
  let r = rows.clone();

  database::load_playlist_store(rows.iter(), &playlist_store_rc);
  database::load_facet_store(&r, &facet_store);
  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently added".to_string(),
  }));
  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently played".to_string(),
  }));

  facet_sel_rc.connect_selection_changed(move |_, _, _| {
    let selection = facet_sel_rc1.selection();
    let (iter, first_pos) = gtk::BitsetIter::init_first(&selection).unwrap();
    playlist_store_rc1.remove_all();
    let item = get_selection(&facet_sel_rc1, first_pos);
    let r: Ref<Facet> = item.borrow();
    let con = rows
      .iter()
      .filter(|x| x.album_artist == r.album_artist && x.album == r.album);

    database::load_playlist_store(con, &playlist_store_rc);

    for pos in iter {
      let item = get_selection(&facet_sel_rc1, pos);
      let r: Ref<Facet> = item.borrow();
      let con = rows
        .iter()
        .filter(|x| x.album_artist == r.album_artist && x.album == r.album);

      database::load_playlist_store(con, &playlist_store_rc);
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
          "{} / {}",
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
        "{} / {}",
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
  facet_box.append(&search_bar);
  facet_box.append(&facet_wnd);

  let playlist_wnd = ScrolledWindow::builder()
    .child(&playlist_columnview)
    .build();

  let playlist_mgr_wnd = ScrolledWindow::builder()
    .child(&playlist_mgr_columnview)
    .build();

  let album_art = Image::builder()
    .file("/home/cdiesh/src/fml9000/cover.jpg")
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
    .end_child(&album_art)
    .build();

  let lrpane = Paned::builder()
    .hexpand(true)
    .orientation(Orientation::Horizontal)
    .start_child(&ltopbottom)
    .end_child(&rtopbottom)
    .build();

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/play.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let play_img = Image::new();
  play_img.set_from_pixbuf(Some(&pixbuf));

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/pause.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let pause_img = Image::new();
  pause_img.set_from_pixbuf(Some(&pixbuf));

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/next.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let next_img = Image::new();
  next_img.set_from_pixbuf(Some(&pixbuf));

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/prev.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let prev_img = Image::new();
  prev_img.set_from_pixbuf(Some(&pixbuf));

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/stop.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let stop_img = Image::new();
  stop_img.set_from_pixbuf(Some(&pixbuf));

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/settings.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let settings_img = Image::new();
  settings_img.set_from_pixbuf(Some(&pixbuf));

  let play_btn = Button::builder().child(&play_img).build();
  let pause_btn = Button::builder().child(&pause_img).build();
  let next_btn = Button::builder().child(&next_img).build();
  let prev_btn = Button::builder().child(&prev_img).build();
  let stop_btn = Button::builder().child(&stop_img).build();
  let settings_btn = Button::builder().child(&settings_img).build();

  let button_box = gtk::Box::new(Orientation::Horizontal, 0);
  let seek_slider = Scale::new(
    Orientation::Horizontal,
    Some(&gtk::Adjustment::new(0.0, 0.0, 1.0, 0.01, 0.0, 0.0)),
  );

  let volume_button = VolumeButton::new();
  volume_button.connect_adjustment_notify(|val| {
    println!("{}", val);
  });
  volume_button.connect_value_changed(move |_, volume| {
    let sink = sink_refcell_rc1.borrow();
    sink.set_volume(volume as f32);
  });
  volume_button.set_value(1.0);
  seek_slider.set_hexpand(true);

  button_box.append(&settings_btn);
  button_box.append(&seek_slider);
  button_box.append(&play_btn);
  button_box.append(&pause_btn);
  button_box.append(&prev_btn);
  button_box.append(&next_btn);
  button_box.append(&stop_btn);
  button_box.append(&volume_button);

  pause_btn.connect_closure(
    "clicked",
    false,
    glib::closure_local!(move |_: Button| {
      // player_rc1.set_playing(false);
    }),
  );

  settings_btn.connect_clicked(move |_| {
    gtk::glib::MainContext::default().spawn_local(dialog(Rc::clone(&wnd_rc2)));
  });

  let main_ui = gtk::Box::new(Orientation::Vertical, 0);
  main_ui.append(&button_box);
  main_ui.append(&lrpane);
  main_ui.add_controller(&gesture);
  popover_menu_rc.set_parent(&main_ui);
  wnd_rc.set_child(Some(&main_ui));
  wnd_rc.show();
}

async fn dialog<W: IsA<gtk::Window>>(wnd: Rc<W>) {
  let preferences_dialog = gtk::Dialog::builder()
    .transient_for(&*wnd)
    .modal(true)
    .title("Preferences")
    .build();
  let content_area = preferences_dialog.content_area();
  let open_button = Button::builder().label("Open folder...").build();
  content_area.append(&open_button);
  let wnd_rc2 = wnd.clone();
  open_button.connect_clicked(move |_| {
    let file_chooser = FileChooserDialog::new(
      Some("Open Folder"),
      Some(&*wnd_rc2),
      FileChooserAction::SelectFolder,
      &[("Open", ResponseType::Ok), ("Cancel", ResponseType::Cancel)],
    );

    file_chooser.connect_response(move |d: &FileChooserDialog, response: ResponseType| {
      if response == ResponseType::Ok {
        let file = d.file().expect("Couldn't get file");
        println!("{}", file);
      }
      d.close();
    });

    file_chooser.show();
  });

  let answer = preferences_dialog.run_future().await;
  preferences_dialog.close();
}
