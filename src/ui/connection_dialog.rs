use crate::config::{ConnectionProfile, SshAuthMethod, SshTunnelConfig, SslMode};
use crate::i18n::I18n;

#[derive(Debug, Default)]
pub struct ConnectionDialog {
    pub profile: ConnectionProfile,
    pub should_save: bool,
    pub cancelled: bool,
    pub error: Option<String>,
    pub test_clicked: bool,
    pub testing: bool,
    pub test_result: Option<Result<(), String>>,
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
        self.test_clicked = false;
        self.testing = false;
        self.test_result = None;
        self.password_buf = String::new();
        self.ssh_expanded = false;
        self.ssh_pw_buf = String::new();
        self.ssh_key_buf = String::new();
        self.ssh_auth_is_key = false;
    }

    /// Returns a ConnectionProfile when the user clicks Connect.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n) -> Option<ConnectionProfile> {
        let mut connect = false;

        egui::Grid::new("conn_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(i18n.label_name());
                ui.text_edit_singleline(&mut self.profile.name);
                ui.end_row();

                ui.label(i18n.label_group());
                let mut group_buf = self.profile.group.clone().unwrap_or_default();
                let changed = ui.add(
                    egui::TextEdit::singleline(&mut group_buf)
                        .hint_text(i18n.hint_group()),
                ).changed();
                if changed {
                    self.profile.group = if group_buf.is_empty() { None } else { Some(group_buf) };
                }
                ui.end_row();

                ui.label(i18n.label_host());
                ui.text_edit_singleline(&mut self.profile.host);
                ui.end_row();

                ui.label(i18n.label_port());
                let mut port_str = self.profile.port.to_string();
                if ui.text_edit_singleline(&mut port_str).changed() {
                    if let Ok(p) = port_str.parse::<u16>() {
                        self.profile.port = p;
                    }
                }
                ui.end_row();

                ui.label(i18n.label_database());
                ui.text_edit_singleline(&mut self.profile.database);
                ui.end_row();

                ui.label(i18n.label_user());
                ui.text_edit_singleline(&mut self.profile.user);
                ui.end_row();

                ui.label(i18n.label_password());
                let pw_edit = egui::TextEdit::singleline(&mut self.password_buf)
                    .password(true)
                    .hint_text(i18n.hint_password());
                ui.add(pw_edit);
                ui.end_row();

                ui.label(i18n.label_ssl_mode());
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
        let ssh_label = i18n.ssh_tunnel();
        let ssh_header = if self.ssh_expanded {
            format!("▾ {ssh_label}")
        } else {
            format!("▸ {ssh_label}")
        };
        if ui.selectable_label(false, ssh_header.as_str()).clicked() {
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
                        ui.label(i18n.label_enabled());
                        ui.checkbox(&mut ssh.enabled, "");
                        ui.end_row();

                        ui.label(i18n.label_ssh_host());
                        ui.text_edit_singleline(&mut ssh.host);
                        ui.end_row();

                        ui.label(i18n.label_ssh_port());
                        let mut port_str = ssh.port.to_string();
                        if ui.text_edit_singleline(&mut port_str).changed() {
                            if let Ok(p) = port_str.parse::<u16>() {
                                ssh.port = p;
                            }
                        }
                        ui.end_row();

                        ui.label(i18n.label_ssh_user());
                        ui.text_edit_singleline(&mut ssh.user);
                        ui.end_row();

                        ui.label(i18n.label_auth());
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.ssh_auth_is_key, false, i18n.label_password_radio());
                            ui.radio_value(&mut self.ssh_auth_is_key, true, i18n.label_private_key());
                        });
                        ui.end_row();

                        if !self.ssh_auth_is_key {
                            ui.label(i18n.label_ssh_password());
                            let pw = egui::TextEdit::singleline(&mut self.ssh_pw_buf).password(true);
                            ui.add(pw);
                            ui.end_row();
                        } else {
                            ui.label(i18n.label_key_path());
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.ssh_key_buf);
                                if ui.small_button(i18n.btn_browse()).clicked() {
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

        ui.checkbox(&mut self.should_save, i18n.save_profile());

        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, err);
        }

        // Test connection result
        match &self.test_result {
            Some(Ok(())) => {
                ui.colored_label(egui::Color32::from_rgb(80, 200, 80), i18n.test_conn_ok());
            }
            Some(Err(msg)) => {
                let full = format!("{}{}", i18n.test_conn_fail(), msg);
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), full);
            }
            None => {}
        }

        ui.horizontal(|ui| {
            if ui.button(i18n.btn_connect()).clicked() {
                connect = true;
            }
            let test_label = if self.testing { i18n.btn_testing() } else { i18n.btn_test_connection() };
            if ui.add_enabled(!self.testing, egui::Button::new(test_label)).clicked() {
                self.test_clicked = true;
                self.test_result = None;
                self.testing = true;
            }
            if ui.button(i18n.btn_cancel()).clicked() {
                self.cancelled = true;
            }
        });

        if self.test_clicked {
            self.profile.password = self.password_buf.clone();
        }

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
                self.error = Some(i18n.err_host_required().to_owned());
                return None;
            }
            Some(self.profile.clone())
        } else {
            None
        }
    }
}
