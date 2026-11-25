//! Everything that draws: theme, icons, shared widgets, the window panes, the work
//! tabs and dialogs. UI code reads app state and pushes `Act`s; it never talks to a
//! database or writes files itself.

pub mod dialogs;
pub mod icons;
pub mod panes;
pub mod style;
pub mod theme;
pub mod views;
pub mod widgets;
