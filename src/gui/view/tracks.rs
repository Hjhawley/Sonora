//! Track view (table list).

use iced::widget::{Column, column, container, mouse_area, row, scrollable, text};
use iced::{Alignment, Length};

use super::super::state::{Message, Sonora};
use super::super::util::filename_stem;
use super::constants::{
    HEADER_TEXT, ROW_TEXT, TRACK_LIST_SPACING, TRACK_ROW_H, TRACK_ROW_HPAD, TRACK_ROW_VPAD,
};
use super::widgets::fmt_duration;

pub(crate) fn build_tracks_center(state: &Sonora) -> Column<'_, Message> {
    column![
        text("Tracks").size(18),
        build_tracks_table(state).height(Length::Fill),
    ]
    .spacing(12)
}

fn build_tracks_table(state: &Sonora) -> iced::widget::Scrollable<'_, Message> {
    let header = row![
        text("").size(HEADER_TEXT).width(Length::Fixed(24.0)),
        text("#").size(HEADER_TEXT).width(Length::Fixed(44.0)),
        text("Title").size(HEADER_TEXT).width(Length::Fixed(240.0)),
        text("Artist").size(HEADER_TEXT).width(Length::Fixed(190.0)),
        text("Album").size(HEADER_TEXT).width(Length::Fixed(240.0)),
        text("Album Artist")
            .size(HEADER_TEXT)
            .width(Length::Fixed(170.0)),
        text("Year").size(HEADER_TEXT).width(Length::Fixed(70.0)),
        text("Genre").size(HEADER_TEXT).width(Length::Fixed(140.0)),
        text("Len").size(HEADER_TEXT).width(Length::Fixed(70.0)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let mut col = column![header].spacing(TRACK_LIST_SPACING);

    for (i, t) in state.tracks.iter().enumerate() {
        let is_primary = state.selected_track == Some(i);
        let is_selected = state.selected_tracks.contains(&i);

        // Primary selection gets ▶. Other selected rows get ●.
        let marker = if is_primary {
            "▶"
        } else if is_selected {
            "●"
        } else {
            ""
        };

        let track_no = t.track_no.map(|n| n.to_string()).unwrap_or_default();
        let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
        let artist = t.artist.clone().unwrap_or_else(|| "Unknown".into());
        let album = t.album.clone().unwrap_or_else(|| "Unknown".into());
        let album_artist = t
            .album_artist
            .clone()
            .or_else(|| t.artist.clone())
            .unwrap_or_else(|| "Unknown".into());
        let year = t.year.map(|y| y.to_string()).unwrap_or_default();
        let genre = t.genre.clone().unwrap_or_default();
        let len = fmt_duration(t.duration_ms);

        let row_cells = row![
            text(marker).size(ROW_TEXT).width(Length::Fixed(24.0)),
            text(track_no).size(ROW_TEXT).width(Length::Fixed(44.0)),
            text(title).size(ROW_TEXT).width(Length::Fixed(240.0)),
            text(artist).size(ROW_TEXT).width(Length::Fixed(190.0)),
            text(album).size(ROW_TEXT).width(Length::Fixed(240.0)),
            text(album_artist)
                .size(ROW_TEXT)
                .width(Length::Fixed(170.0)),
            text(year).size(ROW_TEXT).width(Length::Fixed(70.0)),
            text(genre).size(ROW_TEXT).width(Length::Fixed(140.0)),
            text(len).size(ROW_TEXT).width(Length::Fixed(70.0)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let row_widget = mouse_area(
            container(row_cells)
                .padding([TRACK_ROW_VPAD, TRACK_ROW_HPAD])
                .height(Length::Fixed(TRACK_ROW_H))
                .width(Length::Fill),
        )
        .on_press(Message::SelectTrack(i));

        col = col.push(row_widget);
    }

    scrollable(col).height(Length::Fill)
}
