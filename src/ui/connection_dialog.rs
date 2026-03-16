use crate::config::{ConnectionProfile, SslMode};

#[derive(Debug, Default)]
pub struct ConnectionDialog {
    pub profile: ConnectionProfile,
    pub should_save: bool,
    pub error: Option<String>,
    // Password is stored separately for security display
    password_buf: String,
}

impl ConnectionDialog {
    pub fn reset(&mut self) {
        self.profile = ConnectionProfile::default();
        self.should_save = false;
        self.error = None;
        self.password_buf = String::new();
    }

    /// Returns a ConnectionProfile when the user clicks Connect.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<ConnectionProfile> {
        let mut connect = false;

        egui::Grid::new("conn_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.profile.name);
                ui.end_row();

                ui.label("Host:");
                ui.text_edit_singleline(&mut self.profile.host);
                ui.end_row();

                ui.label("Port:");
                let mut port_str = self.profile.port.to_string();
                if ui.text_edit_singleline(&mut port_str).changed() {
                    if let Ok(p) = port_str.parse::<u16>() {
                        self.profile.port = p;
                    }
                }
                ui.end_row();

                ui.label("Database:");
                ui.text_edit_singleline(&mut self.profile.database);
                ui.end_row();

                ui.label("User:");
                ui.text_edit_singleline(&mut self.profile.user);
                ui.end_row();

                ui.label("Password:");
                let pw_edit = egui::TextEdit::singleline(&mut self.password_buf)
                    .password(true)
                    .hint_text("(leave blank for no password)");
                ui.add(pw_edit);
                ui.end_row();

                ui.label("SSL mode:");
                egui::ComboBox::from_id_source("ssl_mode")
                    .selected_text(self.profile.ssl.to_string())
                    .show_ui(ui, |ui: &mut egui::Ui| {
                        ui.selectable_value(
                            &mut self.profile.ssl,
                            SslMode::Disable,
                            "disable",
                        );
                        ui.selectable_value(
                            &mut self.profile.ssl,
                            SslMode::Prefer,
                            "prefer",
                        );
                        ui.selectable_value(
                            &mut self.profile.ssl,
                            SslMode::Require,
                            "require",
                        );
                    });
                ui.end_row();
            });

        ui.separator();

        ui.checkbox(&mut self.should_save, "Save connection profile");

        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, err);
        }

        ui.horizontal(|ui| {
            if ui.button("Connect").clicked() {
                connect = true;
            }
            if ui.button("Cancel").clicked() {
                // Signal cancel by returning None (caller closes the window)
            }
        });

        if connect {
            self.profile.password = self.password_buf.clone();
            if self.profile.host.is_empty() {
                self.error = Some("Host is required".into());
                return None;
            }
            Some(self.profile.clone())
        } else {
            None
        }
    }
}
