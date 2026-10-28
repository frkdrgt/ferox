use crate::config::ConnectionProfile;
use crate::dbeaver::{self, DbeaverScan};
use crate::i18n::I18n;

struct Candidate {
    profile: ConnectionProfile,
    selected: bool,
    /// Same host/port/database/user as an existing ferox profile.
    duplicate: bool,
}

/// Preview + selection dialog for importing connections from DBeaver.
/// Duplicates of existing profiles start unchecked so a second import is a no-op.
#[derive(Default)]
pub struct DbeaverImportDialog {
    pub open: bool,
    candidates: Vec<Candidate>,
    skipped_other: usize,
    sources: Vec<String>,
    error: Option<String>,
}

impl DbeaverImportDialog {
    /// Open the dialog with the auto-detected DBeaver workspace(s).
    pub fn open_auto(&mut self, existing: &[ConnectionProfile]) {
        self.load(&dbeaver::find_data_source_files(), existing);
    }

    fn load(&mut self, files: &[std::path::PathBuf], existing: &[ConnectionProfile]) {
        self.open = true;
        self.error = None;
        self.candidates.clear();
        self.skipped_other = 0;
        self.sources.clear();
        if files.is_empty() {
            return;
        }
        match dbeaver::scan(files) {
            Ok(DbeaverScan { profiles, skipped_other, sources }) => {
                self.skipped_other = skipped_other;
                self.sources = sources.iter().map(|p| p.display().to_string()).collect();
                self.candidates = profiles
                    .into_iter()
                    .map(|profile| {
                        let duplicate = dbeaver::is_duplicate(&profile, existing);
                        Candidate { profile, selected: !duplicate, duplicate }
                    })
                    .collect();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Returns the profiles to add when the user clicks Import.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        i18n: &I18n,
        existing: &[ConnectionProfile],
    ) -> Option<Vec<ConnectionProfile>> {
        if !self.open {
            return None;
        }
        let mut result = None;
        let mut open = self.open;
        let mut close = false;
        let mut pick_file = false;

        egui::Window::new(i18n.dbeaver_title())
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .open(&mut open)
            .show(ctx, |ui| {
                if let Some(err) = &self.error {
                    ui.colored_label(crate::ui::theme::DANGER, err);
                } else if self.sources.is_empty() {
                    ui.label(i18n.dbeaver_not_found());
                } else {
                    for s in &self.sources {
                        ui.label(egui::RichText::new(s).small().color(egui::Color32::from_gray(130)));
                    }
                    if self.candidates.is_empty() {
                        ui.label(i18n.dbeaver_none_pg());
                    }
                }

                if !self.candidates.is_empty() {
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        for c in &mut self.candidates {
                            let p = &c.profile;
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut c.selected, "");
                                if let Some([r, g, b]) = p.resolved_color() {
                                    ui.colored_label(egui::Color32::from_rgb(r, g, b), "●");
                                }
                                ui.label(egui::RichText::new(&p.name).strong());
                                ui.label(
                                    egui::RichText::new(format!("{}@{}:{}/{}", p.user, p.host, p.port, p.database))
                                        .small()
                                        .color(egui::Color32::from_gray(150)),
                                );
                                if p.ssh_tunnel.is_some() {
                                    ui.label(egui::RichText::new("SSH").small());
                                }
                                if c.duplicate {
                                    ui.label(
                                        egui::RichText::new(i18n.dbeaver_exists())
                                            .small()
                                            .color(egui::Color32::from_rgb(220, 180, 50)),
                                    );
                                } else if p.password.is_empty() {
                                    ui.label(
                                        egui::RichText::new(i18n.dbeaver_no_password())
                                            .small()
                                            .color(egui::Color32::from_gray(130)),
                                    );
                                }
                            });
                        }
                    });
                }
                if self.skipped_other > 0 {
                    ui.label(
                        egui::RichText::new(i18n.dbeaver_skipped_other(self.skipped_other))
                            .small()
                            .color(egui::Color32::from_gray(130)),
                    );
                }

                ui.separator();
                ui.horizontal(|ui| {
                    let count = self.candidates.iter().filter(|c| c.selected).count();
                    if ui
                        .add_enabled(count > 0, egui::Button::new(i18n.dbeaver_import_n(count)))
                        .clicked()
                    {
                        result = Some(
                            self.candidates
                                .iter()
                                .filter(|c| c.selected)
                                .map(|c| c.profile.clone())
                                .collect(),
                        );
                        close = true;
                    }
                    if ui.button(i18n.dbeaver_choose_file()).clicked() {
                        pick_file = true;
                    }
                    if ui.button(i18n.btn_cancel()).clicked() {
                        close = true;
                    }
                });
            });

        if pick_file {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DBeaver data-sources", &["json"])
                .pick_file()
            {
                self.load(&[path], existing);
            }
        }
        self.open = open && !close;
        result
    }
}
