use crate::config::{ConnectionProfile, SshAuthMethod, SshTunnelConfig, SslMode};

#[derive(Debug, Default)]
pub struct ConnectionDialog {
    pub profile: ConnectionProfile,
    pub should_save: bool,
    pub cancelled: bool,
    pub error: Option<String>,
    password_buf: String,
    // SSH UI state
    ssh_expanded: bool,
    ssh_pw_buf: String,
    ssh_key_buf: String,
    ssh_auth_is_key: bool,
}

impl ConnectionDialog {
    pub fn reset(&mut self) {
        self.profile = ConnectionProfile::default();
        self.should_save = false;
        self.error = None;
        self.password_buf = String::new();
        self.ssh_expanded = false;
        self.ssh_pw_buf = String::new();
        self.ssh_key_buf = String::new();
        self.ssh_auth_is_key = false;
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

                ui.label("Group:");
                let mut group_buf = self.profile.group.clone().unwrap_or_default();
                let changed = ui.add(
                    egui::TextEdit::singleline(&mut group_buf)
                        .hint_text("e.g. Production, Staging, Dev (optional)"),
                ).changed();
                if changed {
                    self.profile.group = if group_buf.is_empty() { None } else { Some(group_buf) };
                }
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

        // ── SSH Tunnel section ─────────────────────────────────────────────
        let ssh_header = if self.ssh_expanded { "▾ SSH Tunnel" } else { "▸ SSH Tunnel" };
        if ui.selectable_label(false, ssh_header).clicked() {
            self.ssh_expanded = !self.ssh_expanded;
        }

        if self.ssh_expanded {
            // Ensure there's a SshTunnelConfig to edit
            if self.profile.ssh_tunnel.is_none() {
                self.profile.ssh_tunnel = Some(SshTunnelConfig {
                    port: 22,
                    ..Default::default()
                });
            }
            if let Some(ssh) = &mut self.profile.ssh_tunnel {
                egui::Grid::new("ssh_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Enabled:");
                        ui.checkbox(&mut ssh.enabled, "");
                        ui.end_row();

                        ui.label("SSH Host:");
                        ui.text_edit_singleline(&mut ssh.host);
                        ui.end_row();

                        ui.label("SSH Port:");
                        let mut port_str = ssh.port.to_string();
                        if ui.text_edit_singleline(&mut port_str).changed() {
                            if let Ok(p) = port_str.parse::<u16>() {
                                ssh.port = p;
                            }
                        }
                        ui.end_row();

                        ui.label("SSH User:");
                        ui.text_edit_singleline(&mut ssh.user);
                        ui.end_row();

                        ui.label("Auth:");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.ssh_auth_is_key, false, "Password");
                            ui.radio_value(&mut self.ssh_auth_is_key, true, "Private Key");
                        });
                        ui.end_row();

                        if !self.ssh_auth_is_key {
                            ui.label("SSH Password:");
                            let pw = egui::TextEdit::singleline(&mut self.ssh_pw_buf).password(true);
                            ui.add(pw);
                            ui.end_row();
                        } else {
                            ui.label("Key Path:");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.ssh_key_buf);
                                if ui.small_button("Browse…").clicked() {
                                    if let Some(path) =
                                        rfd::FileDialog::new().pick_file()
                                    {
                                        self.ssh_key_buf =
                                            path.to_string_lossy().into_owned();
                                    }
                                }
                            });
                            ui.end_row();
                        }
                    });
            }
        }

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
                self.cancelled = true;
            }
        });

        if connect {
            self.profile.password = self.password_buf.clone();
            // Finalize SSH auth
            if let Some(ssh) = &mut self.profile.ssh_tunnel {
                if ssh.enabled {
                    ssh.auth = if self.ssh_auth_is_key {
                        SshAuthMethod::PrivateKey { path: self.ssh_key_buf.clone() }
                    } else {
                        SshAuthMethod::Password(self.ssh_pw_buf.clone())
                    };
                }
            }
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
