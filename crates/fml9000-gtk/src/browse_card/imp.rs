use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

#[derive(Debug, Default, CompositeTemplate)]
#[template(string = r#"
<?xml version="1.0" encoding="UTF-8"?>
<interface>
  <template class="BrowseCard" parent="GtkBox">
    <property name="orientation">vertical</property>
    <property name="spacing">4</property>
    <property name="margin-start">4</property>
    <property name="margin-end">4</property>
    <property name="margin-top">4</property>
    <property name="margin-bottom">4</property>
    <child>
      <object class="GtkOverlay" id="thumbnail_overlay">
        <property name="width-request">200</property>
        <property name="height-request">150</property>
        <property name="overflow">hidden</property>
        <child>
          <object class="GtkPicture" id="thumbnail">
            <property name="content-fit">cover</property>
            <property name="can-shrink">true</property>
          </object>
        </child>
        <child type="overlay">
          <object class="GtkImage" id="playing_icon">
            <property name="icon-name">media-playback-pause-symbolic</property>
            <property name="pixel-size">32</property>
            <property name="halign">center</property>
            <property name="valign">center</property>
            <property name="visible">false</property>
            <style>
              <class name="browse-playing-icon"/>
            </style>
          </object>
        </child>
        <child type="overlay">
          <object class="GtkSpinner" id="loading_spinner">
            <property name="halign">center</property>
            <property name="valign">center</property>
            <property name="width-request">32</property>
            <property name="height-request">32</property>
            <property name="visible">false</property>
            <style>
              <class name="browse-loading-spinner"/>
            </style>
          </object>
        </child>
      </object>
    </child>
    <child>
      <object class="GtkLabel" id="title_label">
        <property name="xalign">0</property>
        <property name="ellipsize">end</property>
        <property name="max-width-chars">30</property>
        <property name="lines">2</property>
        <property name="wrap">true</property>
        <property name="wrap-mode">word-char</property>
      </object>
    </child>
    <child>
      <object class="GtkLabel" id="subtitle_label">
        <property name="xalign">0</property>
        <property name="ellipsize">end</property>
        <property name="max-width-chars">30</property>
        <style>
          <class name="dim-label"/>
        </style>
      </object>
    </child>
  </template>
</interface>
"#)]
pub struct BrowseCardImp {
  #[template_child]
  pub thumbnail: TemplateChild<gtk::Picture>,
  #[template_child]
  pub playing_icon: TemplateChild<gtk::Image>,
  #[template_child]
  pub loading_spinner: TemplateChild<gtk::Spinner>,
  #[template_child]
  pub title_label: TemplateChild<gtk::Label>,
  #[template_child]
  pub subtitle_label: TemplateChild<gtk::Label>,
}

#[glib::object_subclass]
impl ObjectSubclass for BrowseCardImp {
  const NAME: &'static str = "BrowseCard";
  type Type = super::BrowseCard;
  type ParentType = gtk::Box;

  fn class_init(klass: &mut Self::Class) {
    klass.bind_template();
  }

  fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
    obj.init_template();
  }
}

impl ObjectImpl for BrowseCardImp {}
impl WidgetImpl for BrowseCardImp {}
impl BoxImpl for BrowseCardImp {}
