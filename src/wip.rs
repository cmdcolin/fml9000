// non working pause/play with spacebar
//   let pauseplay_action = SimpleAction::new("pauseplay", None);
//   pauseplay_action.connect_activate(|a, b| {
//     println!("pauseplay {:?} {:?}", a, b);
//   });
//   wnd_rc.add_action(&pauseplay_action);

//   let pauseplay_shortcut = ShortcutAction::parse_string("action(win.pauseplay)").unwrap();
//   pauseplay_action.connect_activate(|_, _| {});
//   let trigger = KeyvalTrigger::new(gdk::Key::space, gdk::ModifierType::empty());
//   let shortcut = Shortcut::builder()
//     .trigger(&trigger)
//     .action(&pauseplay_shortcut)
//     .build();
//   let shortcut_controller = gtk::ShortcutController::new();
//   shortcut_controller.add_shortcut(&shortcut);
//   shortcut_controller.connect_scope_notify(|_| {
//     println!("here");
//   });

//   shortcut_controller.connect_mnemonic_modifiers_notify(|_| {
//     println!("here2");
//   });
//   wnd_rc.add_controller(&shortcut_controller);
//
//
// non-working drag code
//
// let source = gtk::DragSource::new();
// source.connect_drag_begin(|_, _| {
//   println!("k1");
// });

// source.connect_drag_end(|_, _, _| {
//   println!("k2");
// });
// playlist_columnview.add_controller(&source);
//
//

// not great working right click code
//
// let menu = gio::Menu::new();
// menu.append(Some("Add to new playlist"), Some("win.add_to_playlist"));
// menu.append(Some("Properties"), Some("win.properties"));
// let popover_menu = PopoverMenu::builder().build();
// popover_menu.set_menu_model(Some(&menu));
// popover_menu.set_has_arrow(false);
// let popover_menu_rc = Rc::new(popover_menu);
// let popover_menu_rc1 = popover_menu_rc.clone();
// let gesture = GestureClick::new();
// gesture.set_button(gdk::ffi::GDK_BUTTON_SECONDARY as u32);
// gesture.connect_released(move |gesture, _, x, y| {
//   gesture.set_state(gtk::EventSequenceState::Claimed);
//   let _selection = playlist_sel_rc1.selection();

//   popover_menu_rc1.popup();
//   popover_menu_rc1.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 0, 0)));
// });
// popover_menu_rc.set_parent(&main_ui);
// main_ui.add_controller(&gesture);
//
// let action1 = SimpleAction::new("add_to_playlist", None);
// action1.connect_activate(|_, _| {
//   // println!("hello2 {:?} {:?}", a1, args);
// });
// wnd_rc.add_action(&action1);
// let action2 = SimpleAction::new("properties", None);
// action2.connect_activate(|_, _| {
//   // println!("hello {:?} {:?}", a1, args);
// });
// wnd_rc.add_action(&action2);
// main_ui.add_controller(&gesture);
