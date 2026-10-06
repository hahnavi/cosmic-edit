// SPDX-License-Identifier: GPL-3.0-only

use cosmic::widget::menu::Item as MenuItem;
use cosmic::widget::menu::key_bind::KeyBind;
use cosmic::{
    Element,
    app::Core,
    iced::{
        Length, advanced::widget::text::Style as TextStyle,
    },
    theme,
    widget::{
        self, divider,
        menu::{ItemHeight, ItemWidth, Tree as MenuTree, menu_button},
        responsive_menu_bar, segmented_button, space,
    },
};
use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use crate::{Action, Config, ConfigState, Message, fl};

static MENU_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(|| cosmic::widget::Id::new("responsive-menu"));

// Menu rows are fixed-height in libcosmic; wrapped labels get visually clipped.
// Keep recent path labels short enough to stay on one line.
const RECENT_MENU_LABEL_MAX_CHARS: usize = 40;

fn char_count(value: &str) -> usize {
    value.chars().count()
}

fn take_prefix_chars(value: &str, count: usize) -> String {
    value.chars().take(count).collect()
}

fn take_suffix_chars(value: &str, count: usize) -> String {
    let total = char_count(value);
    if count >= total {
        value.to_string()
    } else {
        value.chars().skip(total - count).collect()
    }
}

fn truncate_middle(value: &str, max_chars: usize) -> String {
    const ELLIPSIS: &str = "...";
    if char_count(value) <= max_chars {
        return value.to_string();
    }

    if max_chars <= ELLIPSIS.len() + 2 {
        return take_prefix_chars(value, max_chars);
    }

    let prefix_len = (max_chars - ELLIPSIS.len()) / 2;
    let suffix_len = max_chars - ELLIPSIS.len() - prefix_len;
    format!(
        "{}{}{}",
        take_prefix_chars(value, prefix_len),
        ELLIPSIS,
        take_suffix_chars(value, suffix_len)
    )
}

fn format_recent_menu_path(path: &PathBuf, home_dir_opt: Option<&PathBuf>) -> String {
    const ELLIPSIS: &str = "...";

    let display = if let Some(home_dir) = home_dir_opt {
        if let Ok(part) = path.strip_prefix(home_dir) {
            format!("~/{}", part.display())
        } else {
            path.display().to_string()
        }
    } else {
        path.display().to_string()
    };

    if char_count(&display) <= RECENT_MENU_LABEL_MAX_CHARS {
        return display;
    }

    let file_name = path.file_name().and_then(|name| name.to_str());
    let parent_name = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str());

    let mut tails = Vec::new();
    if let Some(file_name) = file_name {
        tails.push(format!("/{}", file_name));
        if let Some(parent_name) = parent_name {
            tails.push(format!("/{}/{}", parent_name, file_name));
        }
    }

    for tail in tails.into_iter().rev() {
        let tail_len = char_count(&tail);
        if tail_len + ELLIPSIS.len() >= RECENT_MENU_LABEL_MAX_CHARS {
            continue;
        }

        let prefix_len = RECENT_MENU_LABEL_MAX_CHARS - ELLIPSIS.len() - tail_len;
        if prefix_len < 6 {
            continue;
        }

        return format!(
            "{}{}{}",
            take_prefix_chars(&display, prefix_len),
            ELLIPSIS,
            tail
        );
    }

    truncate_middle(&display, RECENT_MENU_LABEL_MAX_CHARS)
}

pub fn editor_context_menu_items(
    key_binds: &HashMap<KeyBind, Action>,
    entity: segmented_button::Entity,
    has_selection: bool,
) -> Vec<MenuTree<Message>> {
    fn key_style(theme: &cosmic::Theme) -> TextStyle {
        let mut color = theme.cosmic().background(false).component.on;
        color.alpha *= 0.75;
        TextStyle {
            color: Some(color.into()),
            ..Default::default()
        }
    }

    let key_for = |action: &Action| {
        for (key_bind, key_action) in key_binds.iter() {
            if key_action == action {
                return key_bind.to_string();
            }
        }
        String::new()
    };

    let menu_item = |menu_label: String, menu_action: Action, enabled: bool| {
        MenuTree::from(Element::from(
            menu_button(vec![
                widget::text(menu_label).into(),
                space::horizontal().into(),
                widget::text(key_for(&menu_action))
                    .class(theme::Text::Custom(key_style))
                    .into(),
            ])
            .on_press_maybe(
                enabled.then_some(Message::TabContextAction(entity, menu_action)),
            ),
        ))
    };

    let divider_item = || {
        MenuTree::from(Element::from(
            widget::container(divider::horizontal::light())
                .padding([4, 0])
                .width(Length::Fill),
        ))
    };

    vec![
        menu_item(fl!("undo"), Action::Undo, true),
        menu_item(fl!("redo"), Action::Redo, true),
        divider_item(),
        menu_item(fl!("cut"), Action::Cut, has_selection),
        menu_item(fl!("copy"), Action::Copy, has_selection),
        menu_item(fl!("paste"), Action::Paste, true),
        divider_item(),
        menu_item(fl!("select-all"), Action::SelectAll, has_selection),
    ]
}

pub fn tab_context_menu_items(
    key_binds: &HashMap<KeyBind, Action>,
    entity: segmented_button::Entity,
    is_editor: bool,
    has_path: bool,
) -> Vec<MenuTree<cosmic::Action<Message>>> {
    fn key_style(theme: &cosmic::Theme) -> TextStyle {
        let mut color = theme.cosmic().background(false).component.on;
        color.alpha *= 0.75;
        TextStyle {
            color: Some(color.into()),
            ..Default::default()
        }
    }

    let key_for = |action: &Action| {
        for (key_bind, key_action) in key_binds.iter() {
            if key_action == action {
                return key_bind.to_string();
            }
        }
        String::new()
    };

    let menu_item = |label: String, key: String, message: Message| {
        MenuTree::from(Element::from(
            menu_button(vec![
                widget::text(label).into(),
                space::horizontal().into(),
                widget::text(key)
                    .class(theme::Text::Custom(key_style))
                    .into(),
            ])
            .on_press(cosmic::Action::App(message)),
        ))
    };

    let divider_item = || {
        MenuTree::from(Element::from(
            widget::container(divider::horizontal::light())
                .padding([4, 0])
                .width(Length::Fill),
        ))
    };

    let mut items = Vec::new();
    items.push(menu_item(
        fl!("close"),
        key_for(&Action::CloseFile),
        Message::TabClose(entity),
    ));
    items.push(menu_item(
        fl!("close-others"),
        String::new(),
        Message::TabCloseOthers(entity),
    ));
    items.push(menu_item(
        fl!("close-all"),
        String::new(),
        Message::TabCloseAll,
    ));
    if is_editor {
        items.push(divider_item());
        items.push(menu_item(
            fl!("save"),
            key_for(&Action::Save),
            Message::Save(Some(entity)),
        ));
        items.push(menu_item(
            fl!("save-as"),
            key_for(&Action::SaveAsDialog),
            Message::SaveAsDialog(Some(entity)),
        ));
        if has_path {
            items.push(divider_item());
            items.push(menu_item(
                fl!("reload"),
                String::new(),
                Message::TabReload(entity),
            ));
            items.push(menu_item(
                fl!("copy-file-path"),
                String::new(),
                Message::TabCopyPath(entity),
            ));
        }
    }
    items
}

pub fn menu_bar<'a>(
    core: &Core,
    config: &Config,
    config_state: &ConfigState,
    key_binds: &HashMap<KeyBind, Action>,
    projects: &Vec<(String, PathBuf)>,
    has_selection: bool,
) -> Element<'a, Message> {
    //TODO: port to libcosmic
    let menu_tab_width = |tab_width: u16| {
        MenuItem::CheckBox(
            fl!("tab-width", tab_width = tab_width),
            None,
            config.tab_width == tab_width,
            Action::TabWidth(tab_width),
        )
    };

    let home_dir_opt = dirs::home_dir();

    let mut recent_files = Vec::with_capacity(config_state.recent_files.len());
    for (i, path) in config_state.recent_files.iter().enumerate() {
        recent_files.push(MenuItem::Button(
            format_recent_menu_path(path, home_dir_opt.as_ref()),
            None,
            Action::OpenRecentFile(i),
        ));
    }

    let mut recent_projects = Vec::with_capacity(config_state.recent_projects.len());
    for (i, path) in config_state.recent_projects.iter().enumerate() {
        recent_projects.push(MenuItem::Button(
            format_recent_menu_path(path, home_dir_opt.as_ref()),
            None,
            Action::OpenRecentProject(i),
        ));
    }

    let mut close_projects = Vec::with_capacity(projects.len());
    for (project_i, (name, _path)) in projects.iter().enumerate() {
        close_projects.push(MenuItem::Button(
            name.clone(),
            None,
            Action::CloseProject(project_i),
        ));
    }

    responsive_menu_bar()
        .item_height(ItemHeight::Dynamic(40))
        .item_width(ItemWidth::Uniform(320))
        .spacing(4.0)
        .into_element(
            core,
            key_binds,
            MENU_ID.clone(),
            Message::Surface,
            vec![
                (
                    (fl!("file")),
                    vec![
                        MenuItem::Button(fl!("new-file"), None, Action::NewFile),
                        MenuItem::Button(fl!("new-window"), None, Action::NewWindow),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("open-file"), None, Action::OpenFileDialog),
                        MenuItem::Folder(fl!("open-recent-file"), recent_files),
                        MenuItem::Button(fl!("close-file"), None, Action::CloseFile),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("menu-open-project"), None, Action::OpenProjectDialog),
                        MenuItem::Folder(fl!("open-recent-project"), recent_projects),
                        MenuItem::Folder(fl!("close-project"), close_projects),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("save"), None, Action::Save),
                        MenuItem::Button(fl!("save-as"), None, Action::SaveAsDialog),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("revert-all-changes"), None, Action::RevertAllChanges),
                        MenuItem::Divider,
                        MenuItem::Button(
                            fl!("menu-document-statistics"),
                            None,
                            Action::ToggleDocumentStatistics,
                        ),
                        //TODO MenuItem::Button(fl!("document-type"), Action::Todo),
                        //TODO MenuItem::Button(fl!("encoding"), Action::Todo),
                        MenuItem::Button(
                            fl!("menu-git-management"),
                            None,
                            Action::ToggleGitManagement,
                        ),
                        //TODO MenuItem::Button(fl!("print"), Action::Todo),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("quit"), None, Action::Quit),
                    ],
                ),
                (
                    (fl!("edit")),
                    vec![
                        MenuItem::Button(fl!("undo"), None, Action::Undo),
                        MenuItem::Button(fl!("redo"), None, Action::Redo),
                        MenuItem::Divider,
                        if has_selection {
                            MenuItem::Button(fl!("cut"), None, Action::Cut)
                        } else {
                            MenuItem::ButtonDisabled(fl!("cut"), None, Action::NoOp)
                        },
                        if has_selection {
                            MenuItem::Button(fl!("copy"), None, Action::Copy)
                        } else {
                            MenuItem::ButtonDisabled(fl!("copy"), None, Action::NoOp)
                        },
                        MenuItem::Button(fl!("paste"), None, Action::Paste),
                        MenuItem::Button(fl!("select-all"), None, Action::SelectAll),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("find"), None, Action::Find),
                        MenuItem::Button(fl!("replace"), None, Action::FindAndReplace),
                        MenuItem::Button(fl!("find-in-project"), None, Action::ToggleProjectSearch),
                        /*TODO: implement spell-check
                        MenuItem::Divider,
                        MenuItem::Button(fl!("spell-check"), None, Action::Todo),
                        */
                    ],
                ),
                (
                    (fl!("view")),
                    vec![
                        MenuItem::Folder(
                            fl!("indentation"),
                            vec![
                                MenuItem::CheckBox(
                                    fl!("automatic-indentation"),
                                    None,
                                    config.auto_indent,
                                    Action::ToggleAutoIndent,
                                ),
                                MenuItem::Divider,
                                menu_tab_width(1),
                                menu_tab_width(2),
                                menu_tab_width(3),
                                menu_tab_width(4),
                                menu_tab_width(5),
                                menu_tab_width(6),
                                menu_tab_width(7),
                                menu_tab_width(8),
                                //TODO MenuItem::Divider,
                                //TODO MenuItem::Button(fl!("convert-indentation-to-spaces"), Action::Todo),
                                //TODO MenuItem::Button(fl!("convert-indentation-to-tabs"), Action::Todo),
                            ],
                        ),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("zoom-in"), None, Action::ZoomIn),
                        MenuItem::Button(fl!("default-size"), None, Action::ZoomReset),
                        MenuItem::Button(fl!("zoom-out"), None, Action::ZoomOut),
                        MenuItem::Divider,
                        MenuItem::CheckBox(
                            fl!("word-wrap"),
                            None,
                            config.word_wrap,
                            Action::ToggleWordWrap,
                        ),
                        MenuItem::CheckBox(
                            fl!("show-line-numbers"),
                            None,
                            config.line_numbers,
                            Action::ToggleLineNumbers,
                        ),
                        MenuItem::CheckBox(
                            fl!("highlight-current-line"),
                            None,
                            config.highlight_current_line,
                            Action::ToggleHighlightCurrentLine,
                        ),
                        //TODO: MenuItem::CheckBox(fl!("syntax-highlighting"), Action::Todo),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("menu-settings"), None, Action::ToggleSettingsPage),
                        //TODO MenuItem::Divider,
                        //TODO MenuItem::Button(fl!("menu-keyboard-shortcuts"), Action::Todo),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("menu-about"), None, Action::About),
                    ],
                ),
            ],
        )
}
