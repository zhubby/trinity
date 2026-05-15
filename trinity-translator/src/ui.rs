use egui::{self, epaint::Color32};
use std::sync::{Arc, Mutex, mpsc};
use trinity_util::{cfg::get_theme, font};

#[cfg(target_os = "windows")]
use crate::hotkey::HotkeySetting;
#[cfg(target_os = "windows")]
use std::sync::mpsc::Receiver;

pub const LINK_COLOR_DOING: Color32 = Color32::GREEN;
pub const LINK_COLOR_COMMON: Color32 = Color32::GRAY;

pub struct State {
    pub text: String,
    pub source_lang: deepl::Lang,
    pub target_lang: deepl::Lang,
    pub link_color: Color32,
}

pub struct MyApp {
    state: Arc<Mutex<State>>,

    lang_list_with_auto: Vec<deepl::Lang>,
    lang_list: Vec<deepl::Lang>,
    task_chan: mpsc::SyncSender<()>,
    show_box: bool,

    #[cfg(target_os = "windows")]
    hk_setting: HotkeySetting,
    #[cfg(target_os = "windows")]
    hotkey_rx: Receiver<()>,
}

impl MyApp {
    pub fn new(
        state: Arc<Mutex<State>>,
        task_chan: mpsc::SyncSender<()>,
        cc: &eframe::CreationContext<'_>,
    ) -> Self {
        font::install_fonts(&cc.egui_ctx);

        match get_theme().as_str() {
            "light" => cc.egui_ctx.set_visuals(egui::Visuals::light()),
            _ => cc.egui_ctx.set_visuals(egui::Visuals::dark()),
        }

        #[cfg(target_os = "windows")]
        let (hotkey_tx, hotkey_rx) = mpsc::sync_channel(1);
        #[cfg(target_os = "windows")]
        let mut hk_setting = HotkeySetting::default();
        #[cfg(target_os = "windows")]
        hk_setting.register_hotkey(hotkey_tx);

        Self {
            state,

            lang_list_with_auto: deepl::Lang::lang_list_with_auto(),
            lang_list: deepl::Lang::lang_list(),
            task_chan,
            show_box: false,

            #[cfg(target_os = "windows")]
            hk_setting,
            #[cfg(target_os = "windows")]
            hotkey_rx,
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let Self {
            state,

            lang_list_with_auto,
            lang_list,
            task_chan,
            show_box,

            #[cfg(target_os = "windows")]
            hk_setting,
            #[cfg(target_os = "windows")]
            hotkey_rx,
        } = self;
        let mut state = state.lock().unwrap();

        let old_source_lang = state.source_lang;
        let old_target_lang = state.target_lang;

        let ctx = ui.ctx().clone();

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            #[cfg(target_os = "windows")]
            hk_setting.unregister_all();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        #[cfg(target_os = "windows")]
        if hotkey_rx.try_recv().is_ok() {
            _ = task_chan.send(());
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal_top(|ui| {
                    let combobox_width = 145.0;
                    egui::ComboBox::from_id_salt(egui::Id::new("source_lang_ComboBox"))
                        .selected_text(state.source_lang.description())
                        .width(combobox_width)
                        .show_ui(ui, |ui| {
                            for i in lang_list_with_auto {
                                let i = i.to_owned();
                                ui.selectable_value(&mut state.source_lang, i, i.description());
                            }
                        });

                    if ui.add(egui::Button::new(" ⇌ ").frame(false)).clicked() {
                        let tmp_target_lang = state.target_lang;
                        let tmp_source_lang = state.source_lang;
                        state.target_lang = if tmp_source_lang == deepl::Lang::Auto {
                            deepl::Lang::EN
                        } else {
                            tmp_source_lang
                        };
                        state.source_lang = tmp_target_lang;
                    };

                    egui::ComboBox::from_id_salt(egui::Id::new("target_lang_ComboBox"))
                        .selected_text(state.target_lang.description())
                        .width(combobox_width)
                        .show_ui(ui, |ui| {
                            for i in lang_list {
                                let i = i.to_owned();
                                ui.selectable_value(&mut state.target_lang, i, i.description());
                            }
                        });
                    if ui.add(egui::Button::new("翻译")).clicked() {
                        _ = task_chan.send(());
                    };

                    ui.horizontal_wrapped(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.visuals_mut().hyperlink_color = state.link_color;
                            ui.hyperlink_to("⌘", "https://github.com/zu1k/translator");

                            if ui.add(egui::Button::new("□").frame(false)).clicked() {
                                *show_box = !*show_box;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(
                                    *show_box,
                                ));
                            };
                            if ui
                                .add(egui::Button::new("○").frame(false))
                                .is_pointer_button_down_on()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                            };
                        });
                    });
                });

                ui.separator();

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut state.text)
                                .desired_width(2000.0)
                                .desired_rows(7)
                                .frame(egui::Frame::NONE)
                                .lock_focus(true),
                        );
                    });
            });
        });

        if state.source_lang != old_source_lang || state.target_lang != old_target_lang {
            _ = task_chan.send(());
        };

        ctx.request_repaint();
    }
}
