//! Window pieces: menu, ribbon, connection tree, object list, info pane, tab bar and
//! status bar. Each takes only the state it shows and pushes `Act`s.

mod info;
mod menu_bar;
mod nav;
mod object_menus;
mod objects;
mod objects_toolbar;
mod ribbon;
mod search;
mod status_bar;
mod tab_bar;

pub use info::info;
pub use menu_bar::menu_bar;
pub use nav::nav;
pub use objects::objects;
pub use ribbon::ribbon;
pub use search::{search_shortcut, SearchBox};
pub use status_bar::status_bar;
pub use tab_bar::tab_bar;
