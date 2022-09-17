use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use gtk::BinLayout;
use gtk::CompositeTemplate;

#[derive(Debug, Default, CompositeTemplate)]
#[template(file = "grid_cell.ui")]
pub struct GridCell {
  #[template_child]
  // can switch to inscription if we have gtk4.8
  pub name: TemplateChild<gtk::Label>,
}

#[glib::object_subclass]
impl ObjectSubclass for GridCell {
  const NAME: &'static str = "GridCell";
  type Type = super::GridCell;
  type ParentType = gtk::Widget;

  fn class_init(klass: &mut Self::Class) {
    // When inheriting from GtkWidget directly, you have to either override the size_allocate/measure
    // functions of WidgetImpl trait or use a layout manager which provides those functions for your widgets like below.
    klass.set_layout_manager_type::<BinLayout>();
    klass.bind_template();
  }

  fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
    obj.init_template();
  }
}

impl ObjectImpl for GridCell {
  fn dispose(&self, _widget: &Self::Type) {
    self.name.unparent();
  }
}
impl WidgetImpl for GridCell {}
