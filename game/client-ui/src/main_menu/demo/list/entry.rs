use egui::Response;
use egui_extras::TableRow;
use ui_base::utils::icon_font_text_for_text;

use crate::main_menu::demo_list::DemoListEntry;

/// single server list entry
pub fn render(mut row: TableRow<'_, '_>, demo: &DemoListEntry) -> Response {
    let mut clicked: Response;

    let mut inner_clicked = None;
    let (_, res) = row.col(|ui| {
        inner_clicked = Some(
            ui.centered_and_justified(|ui| {
                ui.label(icon_font_text_for_text(
                    ui,
                    match demo {
                        DemoListEntry::File { .. } => "\u{f15b}",
                        DemoListEntry::Directory { .. } => "\u{f07b}",
                    },
                ))
            })
            .inner,
        );
    });
    clicked = if let Some(inner) = inner_clicked {
        res.union(inner)
    } else {
        res
    };
    let res = row
        .col(|ui| {
            clicked = clicked.union(ui.label(match demo {
                DemoListEntry::File { name, .. } => name.trim_end_matches(".twdemo"),
                DemoListEntry::Directory { name } => name,
            }));
        })
        .1;
    clicked = clicked.union(res);
    let res = row
        .col(|ui| {
            clicked = clicked.union(ui.label(match demo {
                DemoListEntry::File { date, .. } => date,
                DemoListEntry::Directory { .. } => "",
            }));
        })
        .1;
    clicked = clicked.union(res);
    clicked
}
