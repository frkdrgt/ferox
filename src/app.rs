use std::sync::mpsc::{self, Receiver, Sender};

use crate::{
    config::AppConfig,
    db::{DbCommand, DbEvent, DbHandle},
    history::QueryHistory,
    ui::{
        connection_dialog::ConnectionDialog,
        join_builder::{JoinAction, JoinBuilder},
        sidebar::{Sidebar, SidebarAction},
        tab_manager::TabManager,
        table_dialog::{TableDialog, TableDialogAction},
    },
};

pub struct PgClientApp {
    // DB communication channels
    pub db_tx: Sender<DbCommand>,
    pub db_rx: Receiver<DbEvent>,

    // UI panels
    pub sidebar: Sidebar,
    pub tab_manager: TabManager,
    pub connection_dialog: ConnectionDialog,

    // Dialogs
    pub table_dialog: TableDialog,
    pub join_builder: JoinBuilder,

    // App state
    pub config: AppConfig,
    pub history: QueryHistory,
    pub status: ConnectionStatus,
    pub show_connection_dialog: bool,
    pub active_profile_idx: Option<usize>,
    /// Schema to reload after a DDL command succeeds.
    pub pending_ddl_schema: Option<String>,
    /// (schema, table) for which we're awaiting LoadDetails before opening Edit Table dialog.
    pub pending_edit_table: Option<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected { host: String, database: String },
    Error(String),
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        ConnectionStatus::Disconnected
    }
}

impl PgClientApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);

        let (cmd_tx, cmd_rx) = mpsc::channel::<DbCommand>();
        let (evt_tx, evt_rx) = mpsc::channel::<DbEvent>();

        // Spawn DB worker thread
        DbHandle::spawn(cmd_rx, evt_tx);

        let config = AppConfig::load().unwrap_or_default();
        let history = QueryHistory::load().unwrap_or_default();

        Self {
            db_tx: cmd_tx,
            db_rx: evt_rx,
            sidebar: Sidebar::default(),
            tab_manager: TabManager::default(),
            connection_dialog: ConnectionDialog::default(),
            table_dialog: TableDialog::default(),
            join_builder: JoinBuilder::default(),
            config,
            history,
            status: ConnectionStatus::Disconnected,
            show_connection_dialog: false,
            active_profile_idx: None,
            pending_ddl_schema: None,
            pending_edit_table: None,
        }
    }

    /// Process all pending DB events from the background thread.
    fn process_db_events(&mut self) {
        while let Ok(event) = self.db_rx.try_recv() {
            match event {
                DbEvent::Connected { host, database } => {
                    self.status = ConnectionStatus::Connected {
                        host: host.clone(),
                        database: database.clone(),
                    };
                    // Trigger schema load
                    let _ = self.db_tx.send(DbCommand::LoadSchemas);
                }
                DbEvent::ConnectionError(msg) => {
                    self.status = ConnectionStatus::Error(msg);
                }
                DbEvent::Disconnected => {
                    self.status = ConnectionStatus::Disconnected;
                    self.sidebar.clear();
                }
                DbEvent::Schemas(schemas) => {
                    self.sidebar.set_schemas(schemas);
                }
                DbEvent::Tables { schema, tables } => {
                    self.sidebar.set_tables(&schema, tables);
                }
                DbEvent::TableDetails { schema, table, columns, indexes, foreign_keys } => {
                    self.sidebar.set_table_details(
                        &schema,
                        &table,
                        columns.clone(),
                        indexes,
                        foreign_keys,
                    );
                    // If we were waiting for these details to open the Edit Table dialog, do it now.
                    if self.pending_edit_table == Some((schema.clone(), table.clone())) {
                        self.pending_edit_table = None;
                        let schemas = self.sidebar.schema_names();
                        self.table_dialog.open_edit(schema, table, columns, schemas);
                    }
                }
                DbEvent::PrimaryKey { schema, table, columns } => {
                    self.tab_manager.set_primary_key(&schema, &table, columns);
                }
                DbEvent::QueryResult(result) => {
                    self.tab_manager.set_result(result);
                }
                DbEvent::QueryError(msg) => {
                    self.tab_manager.set_error(msg);
                }
                DbEvent::ExportDone(path) => {
                    self.tab_manager.set_export_done(path);
                }
                DbEvent::DdlDone => {
                    if let Some(schema) = self.pending_ddl_schema.take() {
                        let _ = self.db_tx.send(DbCommand::LoadTables { schema });
                    }
                }
            }
        }
    }

    fn render_menu(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Connection", |ui| {
                if ui.button("New Connection…").clicked() {
                    self.show_connection_dialog = true;
                    self.connection_dialog.reset();
                    ui.close_menu();
                }
                if !self.config.connections.is_empty() {
                    ui.separator();
                    let profiles = self.config.connections.clone();
                    for (i, profile) in profiles.iter().enumerate() {
                        if ui.button(&profile.name).clicked() {
                            self.connect_to_profile(i);
                            ui.close_menu();
                        }
                    }
                }
                ui.separator();
                if ui.button("Disconnect").clicked() {
                    let _ = self.db_tx.send(DbCommand::Disconnect);
                    ui.close_menu();
                }
            });

            ui.menu_button("Query", |ui| {
                if ui.button("Join Builder…").clicked() {
                    self.join_builder.open();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Execute (F5 / Ctrl+Enter)").clicked() {
                    self.execute_query();
                    ui.close_menu();
                }
                if ui.button("Cancel (Ctrl+C)").clicked() {
                    let _ = self.db_tx.send(DbCommand::CancelQuery);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Export as CSV…").clicked() {
                    self.tab_manager.trigger_export_csv(&self.db_tx);
                    ui.close_menu();
                }
                if ui.button("Export as JSON…").clicked() {
                    self.tab_manager.trigger_export_json(&self.db_tx);
                    ui.close_menu();
                }
            });
        });
    }

    fn render_status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                match &self.status {
                    ConnectionStatus::Disconnected => {
                        ui.colored_label(egui::Color32::GRAY, "⬤  Disconnected");
                    }
                    ConnectionStatus::Connecting => {
                        ui.colored_label(egui::Color32::YELLOW, "⬤  Connecting…");
                    }
                    ConnectionStatus::Connected { host, database } => {
                        ui.colored_label(
                            egui::Color32::GREEN,
                            format!("⬤  {database}@{host}"),
                        );
                    }
                    ConnectionStatus::Error(msg) => {
                        ui.colored_label(egui::Color32::RED, format!("⬤  Error: {msg}"));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(elapsed) = self.tab_manager.last_query_duration() {
                        ui.label(format!("{elapsed:.2}ms"));
                        ui.label(" | ");
                    }
                    if let Some(row_count) = self.tab_manager.result_row_count() {
                        ui.label(format!("{row_count} rows"));
                    }
                });
            });
        });
    }

    fn connect_to_profile(&mut self, idx: usize) {
        if let Some(profile) = self.config.connections.get(idx).cloned() {
            self.active_profile_idx = Some(idx);
            self.status = ConnectionStatus::Connecting;
            let _ = self.db_tx.send(DbCommand::Connect(profile));
        }
    }

    fn execute_query(&mut self) {
        let sql = self.tab_manager.current_sql().to_owned();
        if sql.trim().is_empty() {
            return;
        }
        self.history.push(sql.clone());
        let _ = self.history.save();
        self.tab_manager.set_running();
        let _ = self.db_tx.send(DbCommand::Execute(sql));
    }
}

impl eframe::App for PgClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_db_events();

        // Update window title with active DB name.
        let title = match &self.status {
            ConnectionStatus::Connected { database, host } => {
                format!("pgclient — {database} @ {host}")
            }
            ConnectionStatus::Connecting => "pgclient — connecting…".to_owned(),
            _ => "pgclient".to_owned(),
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        let should_execute = ctx.input(|i| {
            i.key_pressed(egui::Key::F5)
                || (i.modifiers.ctrl && i.key_pressed(egui::Key::Enter))
        });
        if should_execute {
            self.execute_query();
        }

        let should_cancel = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::C));
        if should_cancel {
            let _ = self.db_tx.send(DbCommand::CancelQuery);
        }

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.render_menu(ui);
        });

        // Status bar at bottom
        self.render_status_bar(ctx);

        // Left sidebar — schema browser
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .min_width(180.0)
            .default_width(220.0)
            .show(ctx, |ui| {
                for action in self.sidebar.show(ui) {
                    match action {
                        SidebarAction::LoadTables(schema) => {
                            let _ = self.db_tx.send(DbCommand::LoadTables { schema });
                        }
                        SidebarAction::LoadDetails { schema, table } => {
                            let _ = self.db_tx.send(DbCommand::LoadDetails { schema, table });
                        }
                        SidebarAction::BrowseTable { schema, table } => {
                            self.tab_manager.start_browse(schema, table, &self.db_tx);
                        }
                        SidebarAction::RunSql(sql) => {
                            self.tab_manager.set_sql(sql);
                            self.execute_query();
                        }
                        SidebarAction::NewTable { schema } => {
                            let schemas = self.sidebar.schema_names();
                            self.table_dialog.open_new(schema, schemas);
                        }
                        SidebarAction::EditTable { schema, table } => {
                            // Try the details cache first; otherwise request a load.
                            let cached = self
                                .sidebar
                                .get_table_details(&schema, &table)
                                .filter(|d| d.loaded)
                                .map(|d| d.columns.clone());
                            if let Some(columns) = cached {
                                let schemas = self.sidebar.schema_names();
                                self.table_dialog.open_edit(schema, table, columns, schemas);
                            } else {
                                self.pending_edit_table = Some((schema.clone(), table.clone()));
                                let _ = self.db_tx.send(DbCommand::LoadDetails { schema, table });
                            }
                        }
                    }
                }
            });

        // Central panel — query editor + results (with tab bar)
        egui::CentralPanel::default().show(ctx, |ui| {
            self.tab_manager.show(ui, &self.db_tx, &mut self.history);
        });

        // Join Builder dialog
        self.join_builder.update_available(self.sidebar.all_tables_with_columns());
        for action in self.join_builder.show(ctx) {
            match action {
                JoinAction::Run(sql) => {
                    self.tab_manager.set_sql(sql);
                    self.execute_query();
                }
                JoinAction::SendToEditor(sql) => {
                    self.tab_manager.set_sql(sql);
                }
                JoinAction::LoadDetails { schema, table } => {
                    let _ = self.db_tx.send(DbCommand::LoadDetails { schema, table });
                }
            }
        }

        // Table dialog (New / Edit)
        if let Some(action) = self.table_dialog.show(ctx) {
            match action {
                TableDialogAction::ExecuteDdl { sql, refresh_schema } => {
                    self.pending_ddl_schema = Some(refresh_schema);
                    let _ = self.db_tx.send(DbCommand::ExecuteDdl(sql));
                }
            }
        }

        // Connection dialog modal
        if self.show_connection_dialog {
            let mut open = true;
            egui::Window::new("Connect to PostgreSQL")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    if let Some(profile) = self.connection_dialog.show(ui) {
                        // Save profile if requested
                        if self.connection_dialog.should_save {
                            self.config.connections.push(profile.clone());
                            let _ = self.config.save();
                        }
                        self.status = ConnectionStatus::Connecting;
                        let _ = self.db_tx.send(DbCommand::Connect(profile));
                        self.show_connection_dialog = false;
                    }
                });
            if !open {
                self.show_connection_dialog = false;
            }
        }

        // Request repaint while connecting or query running to show spinner
        if matches!(self.status, ConnectionStatus::Connecting)
            || self.tab_manager.is_running()
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

fn configure_style(ctx: &egui::Context) {
    // ── Dark visuals ────────────────────────────────────────────────────────
    let mut vis = egui::Visuals::dark();

    vis.panel_fill       = egui::Color32::from_rgb(18, 20, 24);
    vis.window_fill      = egui::Color32::from_rgb(24, 27, 32);
    vis.faint_bg_color   = egui::Color32::from_rgb(28, 32, 38);

    vis.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(30, 34, 40);
    vis.widgets.inactive.bg_fill       = egui::Color32::from_rgb(36, 40, 48);
    vis.widgets.hovered.bg_fill        = egui::Color32::from_rgb(38, 56, 90);
    vis.widgets.hovered.weak_bg_fill   = egui::Color32::from_rgb(38, 56, 90);
    vis.widgets.active.bg_fill         = egui::Color32::from_rgb(55, 62, 74);

    vis.selection.bg_fill =
        egui::Color32::from_rgba_premultiplied(86, 156, 214, 60);
    vis.selection.stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(86, 156, 214));

    vis.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 50, 58));

    vis.window_rounding = egui::Rounding::same(6.0);
    vis.menu_rounding   = egui::Rounding::same(4.0);

    ctx.set_visuals(vis);

    // ── Spacing & fonts ─────────────────────────────────────────────────────
    let mut style = (*ctx.style()).clone();

    style.spacing.item_spacing   = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.menu_margin    = egui::Margin::same(6.0);
    style.spacing.window_margin  = egui::Margin::same(10.0);

    use egui::{FontId, TextStyle};
    style.text_styles.insert(TextStyle::Heading,  FontId::proportional(15.0));
    style.text_styles.insert(TextStyle::Body,     FontId::proportional(13.0));
    style.text_styles.insert(TextStyle::Button,   FontId::proportional(13.0));
    style.text_styles.insert(TextStyle::Small,    FontId::proportional(11.0));
    style.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.0));

    ctx.set_style(style);
}
