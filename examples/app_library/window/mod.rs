mod imp;
use gtk4 as gtk;

use crate::app_item::AppItem;
use glib::Object;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use gtk::{Application, SignalListItemFactory};

use libcosmic::x;

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        //quit shortcut
        app.set_accels_for_action("win.quit", &["<primary>W", "Escape"]);
        //launch shortcuts
        for i in 1..10 {
            app.set_accels_for_action(&format!("win.launch{}", i), &[&format!("<primary>{}", i)]);
        }
        Object::new(&[("application", app)]).expect("Failed to create `Window`.")
    }

    fn model(&self) -> &gio::ListStore {
        // Get state
        let imp = imp::Window::from_instance(self);
        imp.app_model.get().expect("Could not get model")
    }

    fn setup_model(&self) {
        // Create new model
        let model = gio::ListStore::new(gio::AppInfo::static_type());
        gio::AppInfo::all().iter().for_each(|app_info| {
            model.append(app_info);
        });

        // Get state and set model
        let imp = imp::Window::from_instance(self);
        imp.app_model
            .set(model.clone())
            .expect("Could not set model");

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
        let filter = gtk::CustomFilter::new(|_obj| true);
        let filter_model = gtk::FilterListModel::new(Some(&model), Some(filter).as_ref());
        let sorted_model = gtk::SortListModel::new(Some(&filter_model), Some(&sorter));
        let selection_model = gtk::SingleSelection::new(Some(&sorted_model));

        // Wrap model with selection and pass it to the list view
        imp.app_grid_view.set_model(Some(&selection_model));
    }

    fn setup_callbacks(&self) {
        // Get state
        let imp = imp::Window::from_instance(self);
        let window = self.clone().upcast::<gtk::Window>();
        let app_grid_view = &imp.app_grid_view;
        let sorted_model = app_grid_view
            .model()
            .expect("List view missing selection model")
            .downcast::<gtk::SingleSelection>()
            .expect("could not downcast listview model to single selection model")
            .model()
            .downcast::<gtk::SortListModel>()
            .expect("sorted list model could not be downcast");
        let filter_model = sorted_model
            .model()
            .expect("missing model for sort list model.")
            .downcast::<gtk::FilterListModel>()
            .expect("could not downcast sort list model to filter list model");

        let entry = &imp.entry;

        // Launch the application when an item of the list is activated
        app_grid_view.connect_activate(move |grid_view, position| {
            let model = grid_view.model().unwrap();
            let app_info = model
                .item(position)
                .unwrap()
                .downcast::<gio::AppInfo>()
                .unwrap();

            let context = grid_view.display().app_launch_context();
            if let Err(err) = app_info.launch(&[], Some(&context)) {
                let parent_window = grid_view.root().unwrap().downcast::<gtk::Window>().unwrap();

                gtk::MessageDialog::builder()
                    .text(&format!("Failed to start {}", app_info.name()))
                    .secondary_text(&err.to_string())
                    .message_type(gtk::MessageType::Error)
                    .modal(true)
                    .transient_for(&parent_window)
                    .build()
                    .show();
            }
        });

        entry.connect_changed(
            glib::clone!(@weak filter_model, @weak sorted_model => move |search: &gtk::SearchEntry| {
                let search_text = search.text().to_string().to_lowercase();
                let new_filter: gtk::CustomFilter = gtk::CustomFilter::new(move |obj| {
                    let search_res = obj.downcast_ref::<gio::AppInfo>()
                        .expect("The Object needs to be of type AppInfo");
                    search_res.name().to_string().to_lowercase().contains(&search_text)
                });
                let search_text = search.text().to_string().to_lowercase();
                let new_sorter: gtk::CustomSorter = gtk::CustomSorter::new(move |obj1, obj2| {
                    let app_info1 = obj1.downcast_ref::<gio::AppInfo>().unwrap();
                    let app_info2 = obj2.downcast_ref::<gio::AppInfo>().unwrap();
                    if search_text == "" {
                        return app_info1
                            .name()
                            .to_lowercase()
                            .cmp(&app_info2.name().to_lowercase())
                            .into();
                    }

                    let i_1 = app_info1.name().to_lowercase().find(&search_text);
                    let i_2 = app_info2.name().to_lowercase().find(&search_text);
                    match (i_1, i_2) {
                        (Some(i_1), Some(i_2)) => i_1.cmp(&i_2).into(),
                        (Some(_), None) => std::cmp::Ordering::Less.into(),
                        (None, Some(_)) => std::cmp::Ordering::Greater.into(),
                        _ => app_info1
                            .name()
                            .to_lowercase()
                            .cmp(&app_info2.name().to_lowercase())
                            .into()
                    }
                });

                filter_model.set_filter(Some(new_filter).as_ref());
                sorted_model.set_sorter(Some(new_sorter).as_ref());
            }),
        );

        window.connect_realize(move |window| {
            if let Some((display, surface)) = x::get_window_x11(window) {
                unsafe {
                    x::change_property(
                        &display,
                        &surface,
                        "_NET_WM_WINDOW_TYPE",
                        x::PropMode::Replace,
                        &[x::Atom::new(&display, "_NET_WM_WINDOW_TYPE_DIALOG").unwrap()],
                    );
                }
            } else {
                println!("failed to get X11 window");
            }
        });

        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate(glib::clone!(@weak window => move |_, _| {
            window.close();
        }));
        self.add_action(&action_quit);

        window.connect_is_active_notify(|win| {
            if !win.is_active() {
                win.close();
            }
        });
    }

    fn setup_factory(&self) {
        let factory = SignalListItemFactory::new();
        factory.connect_setup(move |_factory, item| {
            let row = AppItem::new();
            item.set_child(Some(&row));
        });

        // the bind stage is used for "binding" the data to the created widgets on the "setup" stage
        factory.connect_bind(move |_factory, grid_item| {
            let app_info = grid_item
                .item()
                .unwrap()
                .downcast::<gio::AppInfo>()
                .unwrap();

            let child = grid_item.child().unwrap().downcast::<AppItem>().unwrap();
            child.set_app_info(&app_info);
        });
        // Set the factory of the list view
        let imp = imp::Window::from_instance(self);
        imp.app_grid_view.set_factory(Some(&factory));
    }
}