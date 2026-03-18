use crate::grid_cell::Entry;
use crate::new_playlist_dialog;
use crate::playback_controller::PlaybackController;
use crate::settings::FmlSettings;
use crate::source_model::{
  build_section_children, load_media_items_to_store, populate_section_headers, try_get_source_from_row,
  SourceKind, TreeEntry,
};
use crate::youtube_add_dialog;
use adw::prelude::*;
use fml9000_core::{
  add_to_playlist, delete_playlist, load_track_by_filename, load_video_by_id,
  rename_playlist, MediaItem,
};
use gtk::gdk;
use gtk::gio::ListStore;
use gtk::glib;
use gtk::glib::BoxedAnyObject;
use gtk::{ColumnView, ColumnViewColumn, DropTarget, GestureClick, PopoverMenu, ScrolledWindow, SignalListItemFactory, SingleSelection, TreeExpander, TreeListModel, TreeListRow};
use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;

pub fn create_playlist_manager(
  playlist_mgr_store: &ListStore,
  main_playlist_store: ListStore,
  playback_controller: Rc<PlaybackController>,
  settings: Rc<RefCell<FmlSettings>>,
  current_playlist_id: Rc<RefCell<Option<i32>>>,
  is_viewing_playback_queue: Rc<Cell<bool>>,
) -> (gtk::Box, SingleSelection) {
  populate_playlist_store(playlist_mgr_store);

  let tree_model = TreeListModel::new(playlist_mgr_store.clone(), false, true, |item| {
    let obj = item.downcast_ref::<BoxedAnyObject>()?;
    let entry: Ref<TreeEntry> = obj.borrow();
    match &*entry {
      TreeEntry::SectionHeader(_, kind) => Some(build_section_children(kind).into()),
      TreeEntry::Source(_) => None,
    }
  });

  let selection = SingleSelection::builder()
    .model(&tree_model)
    .autoselect(false)
    .build();
  let columnview = ColumnView::builder().model(&selection).build();
  let factory = SignalListItemFactory::new();

  let playback_controller_for_setup = playback_controller.clone();
  let playlist_mgr_store_for_setup = playlist_mgr_store.clone();
  let selection_for_setup = selection.clone();
  let main_store_for_setup = main_playlist_store.clone();
  let current_playlist_id_for_setup = current_playlist_id.clone();
  let tree_model_for_setup = tree_model.clone();
  factory.connect_setup(move |_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let cell = crate::grid_cell::GridCell::new();
    let expander = TreeExpander::new();
    expander.set_child(Some(&cell));
    list_item.set_child(Some(&expander));

    let drop_target = DropTarget::new(glib::Type::STRING, gdk::DragAction::COPY);

    let current_playlist_id: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));

    let pc = playback_controller_for_setup.clone();
    let store = playlist_mgr_store_for_setup.clone();
    let pid = current_playlist_id.clone();
    let pid_for_enter = current_playlist_id.clone();
    let expander_for_enter = expander.clone();
    drop_target.connect_enter(move |_target, _x, _y| {
      if pid_for_enter.borrow().is_some() {
        if let Some(child) = expander_for_enter.child() {
          child.add_css_class("drop-target-hover");
        }
      }
      gdk::DragAction::COPY
    });

    let expander_for_leave = expander.clone();
    drop_target.connect_leave(move |_target| {
      if let Some(child) = expander_for_leave.child() {
        child.remove_css_class("drop-target-hover");
      }
    });

    let expander_for_drop = expander.clone();
    let sel = selection_for_setup.clone();
    let main_store_for_drop = main_store_for_setup.clone();
    let current_pid_for_drop = current_playlist_id_for_setup.clone();
    let tree_model_for_drop = tree_model_for_setup.clone();
    drop_target.connect_drop(move |_target, value, _x, _y| {
      if let Some(child) = expander_for_drop.child() {
        child.remove_css_class("drop-target-hover");
      }

      let Ok(data) = value.get::<String>() else {
        return false;
      };

      if let Some(playlist_id) = *pid.borrow() {
        let result = handle_drop_on_playlist(playlist_id, &data);
        if result {
          for i in 0..tree_model_for_drop.n_items() {
            if let Some(item) = tree_model_for_drop.item(i) {
              if let Some(row) = item.downcast_ref::<TreeListRow>() {
                if let Some(source) = try_get_source_from_row(row) {
                  if source.playlist_id() == Some(playlist_id) {
                    sel.set_selected(i);
                    *current_pid_for_drop.borrow_mut() = Some(playlist_id);
                    main_store_for_drop.remove_all();
                    let items = source.load_items();
                    load_media_items_to_store(&items, &main_store_for_drop);
                    break;
                  }
                }
              }
            }
          }
        }
        return result;
      }

      let store_clone = store.clone();
      let data_clone = data.clone();
      new_playlist_dialog::show_dialog(
        pc.clone(),
        data.clone(),
        move |playlist_id| {
          let _ = handle_drop_on_playlist(playlist_id, &data_clone);
          store_clone.remove_all();
          populate_playlist_store(&store_clone);
        },
      );
      true
    });

    expander.add_controller(drop_target);

    if let Some(cell) = expander.child().and_then(|c| c.downcast::<crate::grid_cell::GridCell>().ok()) {
      cell.set_playlist_id(current_playlist_id);
    }
  });

  let settings_for_bind = settings.clone();
  factory.connect_bind(move |_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let expander = list_item.child().unwrap().downcast::<TreeExpander>().unwrap();
    let row = list_item.item().unwrap().downcast::<TreeListRow>().unwrap();
    expander.set_list_row(Some(&row));

    let cell = expander.child().unwrap().downcast::<crate::grid_cell::GridCell>().unwrap();
    let row_height = settings_for_bind.borrow().row_height;
    cell.set_row_height(row_height.height_pixels(), row_height.is_compact());

    let obj = row.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let entry: Ref<TreeEntry> = obj.borrow();
    match &*entry {
      TreeEntry::SectionHeader(name, _) => {
        cell.set_entry(&Entry { name: name.clone() });
        cell.add_css_class("section-header");
        cell.remove_css_class("user-playlist");
        cell.set_playlist_id_value(None);
      }
      TreeEntry::Source(kind) => {
        cell.remove_css_class("section-header");
        cell.set_entry(&Entry { name: kind.label() });
        if let SourceKind::UserPlaylist(id, _) = kind {
          cell.add_css_class("user-playlist");
          cell.set_playlist_id_value(Some(*id));
        } else {
          cell.remove_css_class("user-playlist");
          cell.set_playlist_id_value(None);
        }
      }
    }
  });

  factory.connect_unbind(move |_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let expander = list_item.child().unwrap().downcast::<TreeExpander>().unwrap();
    expander.set_list_row(None::<&TreeListRow>);
  });

  let column = ColumnViewColumn::builder()
    .title("Playlists")
    .factory(&factory)
    .expand(true)
    .build();

  columnview.append_column(&column);

  let main_playlist_store_clone = main_playlist_store.clone();
  let current_playlist_id_clone = current_playlist_id.clone();
  let playback_controller_clone = playback_controller.clone();
  let is_viewing_playback_queue_clone = is_viewing_playback_queue.clone();
  let main_playlist_store_for_callback = main_playlist_store.clone();
  selection.connect_selection_changed(move |sel, _, _| {
    if let Some(item) = sel.selected_item() {
      let row = item.downcast::<TreeListRow>().unwrap();
      let Some(source) = try_get_source_from_row(&row) else {
        return;
      };

      main_playlist_store_clone.remove_all();

      let is_playback_queue = source == SourceKind::PlaybackQueue;
      is_viewing_playback_queue_clone.set(is_playback_queue);

      if is_playback_queue {
        let store = main_playlist_store_for_callback.clone();
        playback_controller_clone.set_on_queue_changed(Some(Rc::new(move || {
          store.remove_all();
          let items = SourceKind::PlaybackQueue.load_items();
          load_media_items_to_store(&items, &store);
        })));
      } else {
        playback_controller_clone.set_on_queue_changed(None);
      }

      *current_playlist_id_clone.borrow_mut() = source.playlist_id();
      let items = source.load_items();
      load_media_items_to_store(&items, &main_playlist_store_clone);
    }
  });

  let playlist_menu = gtk::gio::Menu::new();
  playlist_menu.append(Some("Rename"), Some("playlist-mgr.rename"));
  playlist_menu.append(Some("Delete"), Some("playlist-mgr.delete"));

  let playlist_popover = PopoverMenu::from_model(Some(&playlist_menu));
  playlist_popover.set_parent(&columnview);
  playlist_popover.set_has_arrow(false);

  let current_playlist: Rc<RefCell<Option<(i32, String)>>> = Rc::new(RefCell::new(None));

  let action_group = gtk::gio::SimpleActionGroup::new();

  let cp = current_playlist.clone();
  let store_for_rename = playlist_mgr_store.clone();
  let pc_for_rename = playback_controller.clone();
  let rename_action = gtk::gio::SimpleAction::new("rename", None);
  rename_action.connect_activate(move |_, _| {
    if let Some((id, name)) = cp.borrow().clone() {
      show_rename_dialog(pc_for_rename.clone(), id, &name, {
        let store = store_for_rename.clone();
        move || {
          store.remove_all();
          populate_playlist_store(&store);
        }
      });
    }
  });
  action_group.add_action(&rename_action);

  let cp = current_playlist.clone();
  let store_for_delete = playlist_mgr_store.clone();
  let delete_action = gtk::gio::SimpleAction::new("delete", None);
  delete_action.connect_activate(move |_, _| {
    if let Some((id, _)) = cp.borrow().clone() {
      if delete_playlist(id).is_ok() {
        store_for_delete.remove_all();
        populate_playlist_store(&store_for_delete);
      }
    }
  });
  action_group.add_action(&delete_action);

  columnview.insert_action_group("playlist-mgr", Some(&action_group));

  let gesture = GestureClick::builder().button(3).build();
  let cp = current_playlist.clone();
  let popover = playlist_popover.clone();
  let sel_for_gesture = selection.clone();
  gesture.connect_pressed(move |gesture, _n_press, x, y| {
    if let Some(item) = sel_for_gesture.selected_item() {
      if let Ok(row) = item.downcast::<TreeListRow>() {
        if let Some(source) = try_get_source_from_row(&row) {
          if let SourceKind::UserPlaylist(id, name) = source {
            *cp.borrow_mut() = Some((id, name));
            let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            popover.set_pointing_to(Some(&rect));
            popover.popup();
            gesture.set_state(gtk::EventSequenceState::Claimed);
          }
        }
      }
    }
  });

  columnview.add_controller(gesture);

  let add_yt_btn = gtk::Button::builder()
    .icon_name("list-add-symbolic")
    .tooltip_text("Add YouTube Channel")
    .css_classes(["flat"])
    .build();

  let playlist_mgr_store_clone = playlist_mgr_store.clone();
  let playback_controller_clone = playback_controller.clone();
  add_yt_btn.connect_clicked(move |_| {
    let store = playlist_mgr_store_clone.clone();
    youtube_add_dialog::show_dialog(playback_controller_clone.clone(), move || {
      store.remove_all();
      populate_playlist_store(&store);
    });
  });

  let header_box = gtk::Box::builder()
    .orientation(gtk::Orientation::Horizontal)
    .build();
  header_box.append(&gtk::Label::builder().label("Playlists").hexpand(true).xalign(0.0).build());
  header_box.append(&add_yt_btn);

  let scrolled = ScrolledWindow::builder()
    .child(&columnview)
    .vexpand(true)
    .build();

  let container = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
    .spacing(4)
    .build();
  container.append(&header_box);
  container.append(&scrolled);

  (container, selection)
}

pub fn populate_playlist_store(store: &ListStore) {
  populate_section_headers(store);
}

fn handle_drop_on_playlist(playlist_id: i32, data: &str) -> bool {
  let mut success = false;
  for line in data.lines() {
    if let Some(filename) = line.strip_prefix("track:") {
      if let Some(track) = load_track_by_filename(filename) {
        if add_to_playlist(playlist_id, &MediaItem::Track(track)).is_ok() {
          success = true;
        }
      }
    }
    if let Some(video_id_str) = line.strip_prefix("video:") {
      if let Ok(video_id) = video_id_str.parse::<i32>() {
        if let Some(video) = load_video_by_id(video_id) {
          if add_to_playlist(playlist_id, &MediaItem::Video(video)).is_ok() {
            success = true;
          }
        }
      }
    }
  }
  success
}

fn show_rename_dialog(
  playback_controller: Rc<PlaybackController>,
  playlist_id: i32,
  current_name: &str,
  on_renamed: impl Fn() + 'static,
) {
  let wnd = playback_controller.window();

  let dialog = gtk::Window::builder()
    .title("Rename Playlist")
    .default_width(350)
    .default_height(150)
    .modal(true)
    .transient_for(&**wnd)
    .build();

  let content = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
    .spacing(12)
    .margin_top(24)
    .margin_bottom(24)
    .margin_start(24)
    .margin_end(24)
    .build();

  let name_entry = gtk::Entry::builder()
    .text(current_name)
    .hexpand(true)
    .build();

  let button_box = gtk::Box::builder()
    .orientation(gtk::Orientation::Horizontal)
    .spacing(12)
    .halign(gtk::Align::End)
    .build();

  let cancel_btn = gtk::Button::builder().label("Cancel").build();
  let rename_btn = gtk::Button::builder()
    .label("Rename")
    .css_classes(["suggested-action"])
    .build();

  button_box.append(&cancel_btn);
  button_box.append(&rename_btn);

  content.append(&name_entry);
  content.append(&button_box);

  dialog.set_child(Some(&content));

  let dialog_weak = dialog.downgrade();
  cancel_btn.connect_clicked(move |_| {
    if let Some(d) = dialog_weak.upgrade() {
      d.close();
    }
  });

  let dialog_weak = dialog.downgrade();
  let name_entry_clone = name_entry.clone();
  rename_btn.connect_clicked(move |_| {
    let new_name = name_entry_clone.text().to_string();
    if !new_name.is_empty()
      && rename_playlist(playlist_id, &new_name).is_ok() {
        on_renamed();
        if let Some(d) = dialog_weak.upgrade() {
          d.close();
        }
      }
  });

  dialog.present();
}
