use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use iced::{
    Length, Padding, Theme,
    widget::{self, button, container, horizontal_rule, hover, text, text_input, vertical_space},
};
use serde::{Deserialize, Serialize};
use strum::EnumString;
use twox_hash::XxHash64;

use crate::icons::{self, ButtonVariant, icon_button};

// This does not need to be cryptographically sound in the slightest. It is just used for
// fingerprinting files to detect updates.
const HASH_SEED: u64 = 1337;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub page: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BookmarkSet {
    pub marks: Vec<Bookmark>,
    file_hash: u64,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum BookmarkMessage {
    CreateBookmark {
        path: PathBuf,
        name: String,
        page: i32,
    },
    DeleteBookmark {
        path: PathBuf,
        name: String,
    },
    GoTo {
        path: PathBuf,
        page: i32,
    },
    PendingName(String),
    RequestNewBookmark {
        name: String,
    },
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookmarkStore {
    sets: Vec<BookmarkSet>,
    #[serde(skip)]
    pending_name: String,
}

impl BookmarkStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn system_store() -> Result<Self> {
        serde_json::from_str(&fs::read_to_string(Self::system_store_path()?)?)
            .map_err(|e| anyhow!("{}", e))
    }

    fn system_store_path() -> Result<PathBuf> {
        Ok(home::home_dir()
            .ok_or(anyhow!("No home directory could be determined"))?
            .join("./.config/miro-pdf/bookmarks.json"))
    }

    pub fn save(&self) -> Result<()> {
        fs::write(
            Self::system_store_path()?,
            serde_json::to_string(self).map_err(|e| anyhow!("{}", e))?,
        )
        .map_err(|e| anyhow!("{}", e))
    }

    pub fn update(&mut self, message: BookmarkMessage) -> iced::Task<BookmarkMessage> {
        match message {
            BookmarkMessage::CreateBookmark { path, name, page } => {
                self.create_bookmark(path, name, page);
                iced::Task::none()
            }
            BookmarkMessage::DeleteBookmark { path, name } => {
                self.delete_bookmark(path, name);
                iced::Task::none()
            }
            BookmarkMessage::PendingName(s) => {
                self.pending_name = s;
                iced::Task::none()
            }
            BookmarkMessage::GoTo { path: _, page: _ }
            | BookmarkMessage::RequestNewBookmark { name: _ } => panic!("Should be handled by app"),
            BookmarkMessage::None => iced::Task::none(),
        }
    }

    pub fn view(&self) -> iced::Element<'_, BookmarkMessage> {
        let mut col = widget::column![
            text("Bookmarks").size(18.0),
            vertical_space().height(8.0),
            text_input("New bookmark", &self.pending_name)
                .on_input(BookmarkMessage::PendingName)
                .on_submit(BookmarkMessage::RequestNewBookmark {
                    name: self.pending_name.clone()
                }),
            vertical_space().height(8.0),
            horizontal_rule(2.0),
            vertical_space().height(8.0),
        ];
        for set in &self.sets {
            col = col.push(self.view_bookmark_set(set));
        }

        container(col).height(Length::Fill).into()
    }

    fn view_bookmark_set<'a>(&self, set: &'a BookmarkSet) -> iced::Element<'a, BookmarkMessage> {
        let mut marks = widget::column![
            text(set.path.file_name().unwrap().to_string_lossy()).shaping(text::Shaping::Advanced),
            vertical_space().height(4.0)
        ];
        for mark in &set.marks {
            marks = marks.push(
                button(widget::row![
                    hover(
                        text(&mark.name)
                            .shaping(text::Shaping::Advanced)
                            .style(|_: &Theme| widget::text::Style {
                                color: Some(iced::Color::from_rgb(0.5, 0.5, 0.5)),
                            })
                            .width(Length::Fill),
                        text(&mark.name)
                            .shaping(text::Shaping::Advanced)
                            .style(|theme: &Theme| {
                                let palette = theme.extended_palette();
                                widget::text::Style {
                                    color: Some(palette.primary.base.color),
                                }
                            })
                            .width(Length::Fill),
                    ),
                    icon_button(icons::delete(), ButtonVariant::Danger).on_press(
                        BookmarkMessage::DeleteBookmark {
                            path: set.path.clone(),
                            name: mark.name.clone()
                        }
                    )
                ])
                .style(|_: &Theme, _| widget::button::Style {
                    background: None,
                    ..Default::default()
                })
                .width(Length::Fill)
                .padding(Padding::default().left(8.0).right(8.0))
                .on_press(BookmarkMessage::GoTo {
                    path: set.path.clone(),
                    page: mark.page,
                }),
            );
        }
        widget::scrollable(marks).into()
    }

    /// Requires canonical path
    fn create_bookmark(&mut self, path: PathBuf, name: String, page: i32) {
        match self.sets.iter_mut().find(|s| s.path == path) {
            Some(set) => {
                set.marks.push(Bookmark { page, name });
            }
            None => {
                self.sets.push(BookmarkSet {
                    marks: vec![Bookmark { page, name }],
                    file_hash: hash_file(&path).unwrap_or(0),
                    path,
                });
            }
        }
    }

    /// Requires canonical path
    fn delete_bookmark(&mut self, path: PathBuf, name: String) {
        if let Some((i, set)) = self
            .sets
            .iter_mut()
            .enumerate()
            .find(|(_, set)| set.path == path)
        {
            set.marks.retain(|m| m.name != name);
            if set.marks.is_empty() {
                self.sets.remove(i);
            }
        }
    }
}

fn hash_file(path: &Path) -> Result<u64> {
    let bytes = fs::read(path)?;
    Ok(XxHash64::oneshot(HASH_SEED, &bytes))
}
