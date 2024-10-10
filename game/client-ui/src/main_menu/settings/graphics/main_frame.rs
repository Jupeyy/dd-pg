use egui::{Button, Color32, DragValue, Grid, Layout, ScrollArea, Stroke};
use egui_extras::{Size, StripBuilder};
use graphics_types::gpu::{Gpu, GpuType};
use ui_base::types::UiRenderPipe;

use crate::{events::UiEvent, main_menu::user_data::UserData};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        let config = &mut pipe.user_data.config.engine;
        let wnd = &mut config.wnd;
        let wnd_old = wnd.clone();

        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(300.0))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    Grid::new("gfx-settings").num_columns(2).show(ui, |ui| {
                        ui.label("Window mode");
                        egui::ComboBox::new("fullscreen_mode", "")
                            .selected_text(if wnd.fullscreen {
                                "fullscreen"
                            } else if !wnd.fullscreen && wnd.maximized && !wnd.decorated {
                                "borderless-fullscreen"
                            } else {
                                "windowed"
                            })
                            .show_ui(ui, |ui| {
                                ui.vertical(|ui| {
                                    if ui.add(egui::Button::new("fullscreen")).clicked() {
                                        wnd.fullscreen = true;
                                    }
                                    if ui.add(egui::Button::new("borderless-fullscreen")).clicked()
                                    {
                                        wnd.fullscreen = false;
                                        wnd.decorated = false;
                                        wnd.maximized = true;
                                    }
                                    if ui.add(egui::Button::new("windowed")).clicked() {
                                        wnd.fullscreen = false;
                                        wnd.decorated = true;
                                    }
                                })
                            });
                        ui.end_row();

                        ui.label("Monitor");
                        egui::ComboBox::new("monitor_select", "")
                            .selected_text(&wnd.monitor.name)
                            .show_ui(ui, |ui| {
                                ui.vertical(|ui| {
                                    for monitor in pipe.user_data.monitors.monitors().iter() {
                                        if ui.add(egui::Button::new(&monitor.name)).clicked() {
                                            wnd.monitor.name = monitor.name.clone();
                                            if let Some(mode) = monitor.video_modes.first() {
                                                wnd.monitor.width = mode.width;
                                                wnd.monitor.height = mode.height;
                                            }
                                        }
                                    }
                                })
                            });
                        ui.end_row();

                        ui.label("V-sync");
                        if ui.checkbox(&mut config.gl.vsync, "").changed() {
                            pipe.user_data.events.push(UiEvent::VsyncChanged);
                        }
                        ui.end_row();

                        let gpus = pipe.user_data.backend_handle.gpus();
                        ui.label("Msaa");
                        let mut msaa_step = (config.gl.msaa_samples as f64).log2() as u32;
                        let max_step = (gpus.cur.msaa_sampling_count as f64).log2() as u32;
                        if ui
                            .add(
                                DragValue::new(&mut msaa_step)
                                    .range(0..=max_step)
                                    .custom_formatter(|v, _| {
                                        let samples = 2_u32.pow(v as u32);
                                        if samples == 1 {
                                            "off".to_string()
                                        } else {
                                            format!("{}", samples)
                                        }
                                    }),
                            )
                            .changed()
                        {
                            config.gl.msaa_samples = 2_u32.pow(msaa_step);
                            pipe.user_data.events.push(UiEvent::MsaaChanged);
                        }
                        ui.end_row();

                        ui.label("Graphics card");
                        let auto_gpu_display_str = format!("auto({})", gpus.auto.name);
                        egui::ComboBox::new("gpu_select", "")
                            .selected_text(if config.gl.gpu == "auto" {
                                &auto_gpu_display_str
                            } else {
                                &config.gl.gpu
                            })
                            .show_ui(ui, |ui| {
                                ui.vertical(|ui| {
                                    let gpu_list = [
                                        vec![Gpu {
                                            name: "auto".to_string(),
                                            ty: GpuType::Invalid,
                                        }],
                                        gpus.gpus.clone(),
                                    ]
                                    .concat();
                                    for gpu in gpu_list {
                                        if ui
                                            .add(
                                                egui::Button::new(if gpu.name == "auto" {
                                                    &auto_gpu_display_str
                                                } else {
                                                    &gpu.name
                                                })
                                                .selected(gpu.name == config.gl.gpu)
                                                .stroke(if gpu.name == gpus.cur.name {
                                                    Stroke::new(2.0, Color32::LIGHT_GREEN)
                                                } else {
                                                    Stroke::NONE
                                                }),
                                            )
                                            .clicked()
                                        {
                                            config.gl.gpu = gpu.name;
                                        }
                                    }
                                })
                            });
                        ui.end_row();
                    });
                });
                strip.cell(|ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ui.available_height());
                    if let Some(monitors) = pipe
                        .user_data
                        .monitors
                        .monitors()
                        .iter()
                        .find(|monitor| monitor.name == wnd.monitor.name)
                    {
                        ui.with_layout(
                            Layout::top_down(egui::Align::Min).with_cross_justify(true),
                            |ui| {
                                fn fmt_res(w: u32, h: u32, refresh_rate_mhz: u32) -> String {
                                    let g = gcd::binary_u32(w, h);
                                    format!(
                                        "{}x{} @{:0.2} ({}:{})",
                                        w,
                                        h,
                                        refresh_rate_mhz as f64 / 1000.0,
                                        w / g,
                                        h / g
                                    )
                                }

                                ui.label(format!(
                                    "Monitor: {} - {}",
                                    monitors.name,
                                    fmt_res(wnd.width, wnd.height, wnd.refresh_rate_mhz)
                                ));
                                ui.style_mut().spacing.scroll.floating = false;
                                ScrollArea::vertical().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    let style = ui.style_mut();
                                    style.visuals.widgets.inactive.weak_bg_fill =
                                        Color32::from_black_alpha(50);
                                    style.visuals.widgets.active.weak_bg_fill =
                                        Color32::from_black_alpha(50);
                                    style.visuals.widgets.hovered.weak_bg_fill =
                                        Color32::from_black_alpha(50);
                                    for mode in &monitors.video_modes {
                                        if ui
                                            .add(
                                                Button::new(fmt_res(
                                                    mode.width,
                                                    mode.height,
                                                    mode.refresh_rate_mhz,
                                                ))
                                                .selected(
                                                    wnd.width == mode.width
                                                        && wnd.height == mode.height
                                                        && wnd.refresh_rate_mhz
                                                            == mode.refresh_rate_mhz,
                                                ),
                                            )
                                            .clicked()
                                        {
                                            wnd.width = mode.width;
                                            wnd.height = mode.height;
                                            wnd.refresh_rate_mhz = mode.refresh_rate_mhz;
                                        }
                                    }
                                });
                            },
                        );
                    }
                });
            });

        if wnd_old != *wnd {
            pipe.user_data.events.push(UiEvent::WindowChange);
        }
    });
}
