mod application_row;
use crate::application_row::ApplicationRow;
use gtk::gio;
use gtk::prelude::*;

fn main() {
  let application = gtk::Application::new(Some("com.fml.music_player"), Default::default());
  application.connect_activate(build_ui);
  application.run();
}

fn build_ui(app: &gtk::Application) {
  let window = gtk::ApplicationWindow::builder()
    .default_width(600)
    .default_height(600)
    .application(app)
    .title("fml")
    .build();

  let model = gio::ListStore::new(gio::AppInfo::static_type());
  gio::AppInfo::all().iter().for_each(|app_info| {
    model.append(app_info);
  });

  let factory = gtk::SignalListItemFactory::new();

  // the "setup" stage is used for creating the widgets
  factory.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let row = ApplicationRow::new();
    item.set_child(Some(&row));
  });

  // the bind stage is used for "binding" the data to the created widgets on the "setup" stage
  factory.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let app_info = item.item().unwrap().downcast::<gio::AppInfo>().unwrap();
    let child = item.child().unwrap().downcast::<ApplicationRow>().unwrap();
    child.set_app_info(&app_info);
  });

  // A sorter used to sort AppInfo in the model by their name
  let sorter = gtk::CustomSorter::new(move |obj1, obj2| {
    let app_info1 = obj1.downcast_ref::<gio::AppInfo>().unwrap();
    let app_info2 = obj2.downcast_ref::<gio::AppInfo>().unwrap();

    app_info1
      .name()
      .to_lowercase()
      .cmp(&app_info2.name().to_lowercase())
      .into()
  });
  let sorted_model = gtk::SortListModel::new(Some(&model), Some(&sorter));
  let selection_model = gtk::SingleSelection::new(Some(&sorted_model));

  let f1 = gtk::SignalListItemFactory::new();

  let c1 = gtk::ColumnViewColumn::new(Some("K1"), Some(&f1));
  let c2 = gtk::ColumnViewColumn::new(Some("K2"), Some(&f1));
  let column_view = gtk::ColumnView::new(Some(&selection_model));
  column_view.append_column(&c1);
  column_view.append_column(&c2);

  // Launch the application when an item of the list is activated
  column_view.connect_activate(move |column_view, position| {
    let model = column_view.model().unwrap();
    let app_info = model
      .item(position)
      .unwrap()
      .downcast::<gio::AppInfo>()
      .unwrap();
  });

  let scrolled_window = gtk::ScrolledWindow::builder()
    .hscrollbar_policy(gtk::PolicyType::Never) // Disable horizontal scrolling
    .min_content_width(360)
    .child(&column_view)
    .build();

  window.set_child(Some(&scrolled_window));
  window.show();
}
