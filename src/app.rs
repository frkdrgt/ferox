use std::sync::mpsc::{self, Receiver, Sender};

use crate::{
    config::AppConfig,
    db::{DbCommand, DbEvent, DbHandle},
    history::QueryHistory,
    ui::{
        connection_dialog::ConnectionDialog,
        join_builder::{JoinAction, JoinBuilder},
        sidebar::{
            pk_from_indexes, script_delete, script_insert, script_select, script_update,
            Sidebar, SidebarAction, ScriptKind,
        },
        tab_manager::TabManager,
        table_dialog::{TableDialog, TableDialogAction},
    },
};

// ── Per-connection state ───────────────────────────────────────────────────────

pub struct ConnState {
    pub id: usize,
    pub name: String,
    pub db_tx: Sender<DbCommand>,
    pub db_rx: Receiver<DbEvent>,
    pub sidebar: Sidebar,
    pub status: ConnectionStatus,
    pub pending_ddl_schema: Option<String>,
    pub pending_edit_table: Option<(String, String)>,
    /// True while a safe-mode BEGIN transaction is open on this connection.
    pub in_transaction: bool,
    /// Script kind pending for (schema, table) — set when GenerateScript fires
    /// before columns are loaded; fulfilled when TableDetails arrives.
    pub pending_script: Option<(String, String, ScriptKind)>,
}

// ── App ────────────────────────────────────────────────────────────────────────

pub struct PgClientApp {
    // Multiple connections
    pub connections: Vec<ConnState>,
    pub active_conn: usize,
    pub next_conn_id: usize,

    // UI panels
    pub tab_manager: TabManager,
    pub connection_dialog: ConnectionDialog,

    // Dialogs
    pub table_dialog: TableDialog,
    pub join_builder: JoinBuilder,

    // App state
    pub config: AppConfig,
    pub history: QueryHistory,
    pub show_connection_dialog: bool,
    /// When true, DML statements are automatically wrapped in BEGIN.
    pub safe_mode: bool,
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

        let config = AppConfig::load().unwrap_or_default();
        let history = QueryHistory::load().unwrap_or_default();

        Self {
            connections: Vec::new(),
            active_conn: 0,
            next_conn_id: 0,
            tab_manager: TabManager::default(),
            connection_dialog: ConnectionDialog::default(),
            table_dialog: TableDialog::default(),
            join_builder: JoinBuilder::default(),
            config,
            history,
            show_connection_dialog: true,
            safe_mode: false,
        }
    }

    /// Create a new ConnState and DB worker for a saved profile, then push it.
    pub fn connect_to_profile(&mut self, idx: usize) {
        if let Some(profile) = self.config.connections.get(idx).cloned() {
            let (cmd_tx, cmd_rx) = mpsc::channel::<DbCommand>();
            let (evt_tx, evt_rx) = mpsc::channel::<DbEvent>();
            DbHandle::spawn(cmd_rx, evt_tx);

            let conn_id = self.next_conn_id;
            self.next_conn_id += 1;

            let _ = cmd_tx.send(DbCommand::Connect(profile.clone()));

            let name = format!("{}@{}", profile.database, profile.host);
            self.connections.push(ConnState {
                id: conn_id,
                name,
                db_tx: cmd_tx,
                db_rx: evt_rx,
                sidebar: Sidebar::default(),
                status: ConnectionStatus::Connecting,
                pending_ddl_schema: None,
                pending_edit_table: None,
                in_transaction: false,
                pending_script: None,
            });
            self.active_conn = self.connections.len() - 1;

            // Update the active tab to use this connection, or create a new tab.
            if self.tab_manager.active_tab_conn_id() != conn_id {
                // If there's already content in the active tab, create a new tab.
                self.tab_manager.new_tab(conn_id);
            }
        }
    }

    /// Connect using a raw profile (from the connection dialog), not from saved config.
    fn connect_with_profile(&mut self, profile: crate::config::ConnectionProfile) {
        let (cmd_tx, cmd_rx) = mpsc::channel::<DbCommand>();
        let (evt_tx, evt_rx) = mpsc::channel::<DbEvent>();
        DbHandle::spawn(cmd_rx, evt_tx);

        let conn_id = self.next_conn_id;
        self.next_conn_id += 1;

        let _ = cmd_tx.send(DbCommand::Connect(profile.clone()));

        let name = format!("{}@{}", profile.database, profile.host);
        self.connections.push(ConnState {
            id: conn_id,
            name,
            db_tx: cmd_tx,
            db_rx: evt_rx,
            sidebar: Sidebar::default(),
            status: ConnectionStatus::Connecting,
            pending_ddl_schema: None,
            pending_edit_table: None,
            in_transaction: false,
            pending_script: None,
        });
        self.active_conn = self.connections.len() - 1;

        // Give the new connection a tab.
        self.tab_manager.new_tab(conn_id);
    }

    /// Process all pending DB events from all background threads.
    fn process_db_events(&mut self) {
        for i in 0..self.connections.len() {
            // Collect events without holding a mutable borrow on the whole Vec.
            let events: Vec<DbEvent> = {
                let conn = &self.connections[i];
                let mut evts = Vec::new();
                while let Ok(e) = conn.db_rx.try_recv() {
                    evts.push(e);
                }
                evts
            };

            let conn_id = self.connections[i].id;

            for event in events {
                match event {
                    DbEvent::Connected { host, database } => {
                        self.connections[i].status = ConnectionStatus::Connected {
                            host: host.clone(),
                            database: database.clone(),
                        };
                        self.connections[i].name = format!("{database}@{host}");
                        let _ = self.connections[i].db_tx.send(DbCommand::LoadSchemas);
                    }
                    DbEvent::ConnectionError(msg) => {
                        self.connections[i].status = ConnectionStatus::Error(msg);
                    }
                    DbEvent::Disconnected => {
                        self.connections[i].status = ConnectionStatus::Disconnected;
                        self.connections[i].sidebar.clear();
                    }
                    DbEvent::Schemas(schemas) => {
                        self.connections[i].sidebar.set_schemas(schemas);
                    }
                    DbEvent::Tables { schema, tables } => {
                        self.connections[i].sidebar.set_tables(&schema, tables);
                    }
                    DbEvent::TableDetails { schema, table, columns, indexes, foreign_keys } => {
                        self.connections[i].sidebar.set_table_details(
                            &schema,
                            &table,
                            columns.clone(),
                            indexes.clone(),
                            foreign_keys,
                        );

                        // Fulfil a pending script request if this is the right table.
                        if let Some((ps, pt, kind)) =
                            self.connections[i].pending_script.take()
                        {
                            if ps == schema && pt == table {
                                let pk = pk_from_indexes(&indexes);
                                let sql = match kind {
                                    ScriptKind::Select => script_select(&schema, &table, &columns),
                                    ScriptKind::Insert => script_insert(&schema, &table, &columns),
                                    ScriptKind::Update => script_update(&schema, &table, &columns, &pk),
                                    ScriptKind::Delete => script_delete(&schema, &table, &columns, &pk),
                                };
                                self.tab_manager.set_sql(sql);
                            } else {
                                // Different table — put it back.
                                self.connections[i].pending_script =
                                    Some((ps, pt, kind));
                            }
                        }

                        if self.connections[i].pending_edit_table
                            == Some((schema.clone(), table.clone()))
                        {
                            self.connections[i].pending_edit_table = None;
                            let schemas = self.connections[i].sidebar.schema_names();
                            self.table_dialog.open_edit(schema, table, columns, schemas);
                        }
                    }
                    DbEvent::PrimaryKey { schema, table, columns } => {
                        self.tab_manager.set_primary_key(&schema, &table, columns);
                    }
                    DbEvent::QueryResult(result) => {
                        self.tab_manager.set_result_for(conn_id, result);
                    }
                    DbEvent::QueryError(msg) => {
                        self.tab_manager.set_error_for(conn_id, msg);
                    }
                    DbEvent::ExportDone(path) => {
                        self.tab_manager.set_export_done(path);
                    }
                    DbEvent::KillDone(_pid) => {
                        // Auto-reload dashboard after kill
                        self.tab_manager.set_dashboard_loading();
                        if let Some(conn) = self.connections.iter().find(|c| c.id == conn_id) {
                            let _ = conn.db_tx.send(DbCommand::LoadDashboard);
                        }
                    }
                    DbEvent::DdlDone => {
                        if let Some(schema) = self.connections[i].pending_ddl_schema.take() {
                            let _ = self.connections[i]
                                .db_tx
                                .send(DbCommand::LoadTables { schema });
                        }
                    }
                    DbEvent::DashboardData { table_stats, connections, index_stats } => {
                        if self.tab_manager.dashboard_conn_id() == Some(conn_id) {
                            self.tab_manager.set_dashboard_data(
                                table_stats,
                                connections,
                                index_stats,
                            );
                        }
                    }
                    DbEvent::ErDiagramData { schema, tables } => {
                        self.tab_manager.set_er_diagram_data(&schema, tables);
                    }
                    DbEvent::TransactionOpen => {
                        self.connections[i].in_transaction = true;
                    }
                    DbEvent::TransactionClosed => {
                        self.connections[i].in_transaction = false;
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
                    let profiles = self.config.connections.clone();
                    let mut to_delete: Option<usize> = None;

                    // Collect ordered unique groups
                    let mut seen_groups: Vec<Option<String>> = vec![];
                    for p in &profiles {
                        if !seen_groups.contains(&p.group) {
                            seen_groups.push(p.group.clone());
                        }
                    }

                    for group in &seen_groups {
                        ui.separator();
                        if let Some(g) = group {
                            let dot = match g.to_lowercase() {
                                s if s.contains("prod") => "🔴",
                                s if s.contains("stag") || s.contains("test") => "🟡",
                                s if s.contains("dev") || s.contains("local") => "🟢",
                                _ => "🔵",
                            };
                            ui.label(
                                egui::RichText::new(format!("{dot} {g}"))
                                    .small()
                                    .strong()
                                    .color(egui::Color32::from_rgb(110, 123, 139)),
                            );
                        }
                        for (i, profile) in profiles.iter().enumerate() {
                            if profile.group.as_deref() == group.as_deref() {
                                ui.horizontal(|ui| {
                                    if ui.button(&profile.name).clicked() {
                                        self.connect_to_profile(i);
                                        ui.close_menu();
                                    }
                                    if ui.small_button("×").clicked() {
                                        to_delete = Some(i);
                                        ui.close_menu();
                                    }
                                });
                            }
                        }
                    }

                    if let Some(idx) = to_delete {
                        self.config.connections.remove(idx);
                        let _ = self.config.save();
                    }
                }
                ui.separator();
                if ui.button("Disconnect").clicked() {
                    if let Some(conn) = self.connections.get(self.active_conn) {
                        let _ = conn.db_tx.send(DbCommand::Disconnect);
                    }
                    ui.close_menu();
                }
            });

            ui.menu_button("Query", |ui| {
                let safe_label = if self.safe_mode {
                    "🛡 Safe Mode  ✓"
                } else {
                    "🛡 Safe Mode"
                };
                if ui.button(safe_label).clicked() {
                    self.safe_mode = !self.safe_mode;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Join Builder…").clicked() {
                    self.join_builder.open();
                    ui.close_menu();
                }
                if ui.button("📊 Dashboard").clicked() {
                    let active_conn_id = self
                        .connections
                        .get(self.active_conn)
                        .map(|c| c.id)
                        .unwrap_or(0);
                    self.tab_manager.open_or_focus_dashboard(active_conn_id);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Execute (F5 / Ctrl+Enter)").clicked() {
                    self.execute_query();
                    ui.close_menu();
                }
                if ui.button("Cancel (Ctrl+C)").clicked() {
                    if let Some(conn) = self.connections.get(self.active_conn) {
                        let _ = conn.db_tx.send(DbCommand::CancelQuery);
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Export as CSV…").clicked() {
                    let active_conn_id = self.tab_manager.active_tab_conn_id();
                    if let Some(conn) = self.connections.iter().find(|c| c.id == active_conn_id) {
                        self.tab_manager.trigger_export_csv(&conn.db_tx);
                    }
                    ui.close_menu();
                }
                if ui.button("Export as JSON…").clicked() {
                    let active_conn_id = self.tab_manager.active_tab_conn_id();
                    if let Some(conn) = self.connections.iter().find(|c| c.id == active_conn_id) {
                        self.tab_manager.trigger_export_json(&conn.db_tx);
                    }
                    ui.close_menu();
                }
            });
        });
    }

    fn render_status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let status = self
                    .connections
                    .get(self.active_conn)
                    .map(|c| &c.status)
                    .unwrap_or(&ConnectionStatus::Disconnected);

                match status {
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

    fn execute_query(&mut self) {
        let sql = self.tab_manager.current_sql().to_owned();
        if sql.trim().is_empty() {
            return;
        }
        self.history.push(sql.clone());
        let _ = self.history.save();
        let conn_id = self.tab_manager.active_tab_conn_id();
        self.tab_manager.set_running_for(conn_id);

        let active_conn_idx = self.connections.iter().position(|c| c.id == conn_id);
        if let Some(idx) = active_conn_idx {
            let in_tx = self.connections[idx].in_transaction;
            let cmd = if self.safe_mode && !in_tx && is_dml(&sql) {
                DbCommand::ExecuteSafe(sql)
            } else {
                DbCommand::Execute(sql)
            };
            let _ = self.connections[idx].db_tx.send(cmd);
        } else {
            self.tab_manager.set_error_for(conn_id, "Not connected".into());
        }
    }

    fn render_connection_switcher(&mut self, ui: &mut egui::Ui) {
        // Collect display data to avoid borrow issues
        let data: Vec<(usize, String, bool)> = self
            .connections
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let dot = match &c.status {
                    ConnectionStatus::Connected { .. } => "🟢",
                    ConnectionStatus::Connecting => "🟡",
                    ConnectionStatus::Error(_) => "🔴",
                    ConnectionStatus::Disconnected => "⚫",
                };
                (i, format!("{} {}", dot, c.name), i == self.active_conn)
            })
            .collect();

        let mut new_active = self.active_conn;
        let mut open_dialog = false;

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, label, is_active) in &data {
                if ui.selectable_label(*is_active, label.as_str()).clicked() {
                    new_active = *i;
                }
            }
            if ui.small_button("＋").on_hover_text("New connection").clicked() {
                open_dialog = true;
            }
        });

        if new_active != self.active_conn {
            self.active_conn = new_active;
        }
        if open_dialog {
            self.show_connection_dialog = true;
            self.connection_dialog.reset();
        }
    }
}

impl eframe::App for PgClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_db_events();

        // Update window title with active connection's status.
        let title = self
            .connections
            .get(self.active_conn)
            .map(|c| match &c.status {
                ConnectionStatus::Connected { database, host } => {
                    format!("pgclient — {database} @ {host}")
                }
                ConnectionStatus::Connecting => "pgclient — connecting…".to_owned(),
                _ => "pgclient".to_owned(),
            })
            .unwrap_or_else(|| "pgclient".to_owned());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        // Update autocomplete completion data for each connection.
        for conn in &self.connections {
            let (tables, columns) = conn.sidebar.completion_data();
            let conn_id = conn.id;
            self.tab_manager.update_completion_data_for(conn_id, tables, columns);
        }

        // Dashboard load / refresh logic
        if self.tab_manager.dashboard_is_active() && self.tab_manager.dashboard_needs_load() {
            if let Some(conn_id) = self.tab_manager.dashboard_conn_id() {
                if let Some(conn) = self.connections.iter().find(|c| c.id == conn_id) {
                    let _ = conn.db_tx.send(DbCommand::LoadDashboard);
                    self.tab_manager.set_dashboard_loading();
                }
            }
        }

        // ER diagram load poll
        if let Some((er_conn_id, schema)) = self.tab_manager.er_diagram_needs_load() {
            if let Some(conn) = self.connections.iter().find(|c| c.id == er_conn_id) {
                let _ = conn.db_tx.send(DbCommand::LoadErDiagram { schema });
                self.tab_manager.mark_er_load_requested();
            }
        }

        let should_execute = ctx.input(|i| {
            i.key_pressed(egui::Key::F5)
                || (i.modifiers.ctrl && i.key_pressed(egui::Key::Enter))
        });
        if should_execute {
            self.execute_query();
        }

        let should_cancel = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::C));
        if should_cancel {
            if let Some(conn) = self.connections.get(self.active_conn) {
                let _ = conn.db_tx.send(DbCommand::CancelQuery);
            }
        }

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.render_menu(ui);
        });

        // Safe mode transaction banner (shown when a BEGIN is open)
        let active_in_tx = self.connections.get(self.active_conn)
            .map(|c| c.in_transaction)
            .unwrap_or(false);
        if active_in_tx {
            egui::TopBottomPanel::top("safe_mode_banner").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 193, 7),
                        "⚠  Safe Mode — transaction open. Changes are not yet saved.",
                    );
                    ui.add_space(12.0);
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("✓  COMMIT")
                                    .color(egui::Color32::from_rgb(40, 40, 40)),
                            )
                            .fill(egui::Color32::from_rgb(80, 200, 120)),
                        )
                        .clicked()
                    {
                        if let Some(conn) = self.connections.get(self.active_conn) {
                            let _ = conn.db_tx.send(DbCommand::Commit);
                        }
                    }
                    ui.add_space(4.0);
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("✕  ROLLBACK")
                                    .color(egui::Color32::from_rgb(40, 40, 40)),
                            )
                            .fill(egui::Color32::from_rgb(220, 80, 80)),
                        )
                        .clicked()
                    {
                        if let Some(conn) = self.connections.get(self.active_conn) {
                            let _ = conn.db_tx.send(DbCommand::Rollback);
                        }
                    }
                });
            });
        } else if self.safe_mode {
            // No open transaction — show subtle safe mode indicator
            egui::TopBottomPanel::top("safe_mode_indicator").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(86, 156, 214),
                        "🛡 Safe Mode ON — DML statements will be wrapped in BEGIN automatically.",
                    );
                });
            });
        }

        // Status bar at bottom
        self.render_status_bar(ctx);

        // Left sidebar — connection switcher + schema browser
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .min_width(180.0)
            .default_width(220.0)
            .show(ctx, |ui| {
                // Connection switcher at top
                if !self.connections.is_empty() {
                    self.render_connection_switcher(ui);
                    ui.separator();
                }

                let active_conn_id = self.connections.get(self.active_conn).map(|c| c.id);

                let actions = if let Some(conn) = self.connections.get_mut(self.active_conn) {
                    conn.sidebar.show(ui)
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new("No connection").color(egui::Color32::GRAY));
                        if ui.button("Connect…").clicked() {
                            self.show_connection_dialog = true;
                            self.connection_dialog.reset();
                        }
                    });
                    vec![]
                };

                for action in actions {
                    let conn_id = active_conn_id.unwrap_or(0);
                    match action {
                        SidebarAction::LoadTables(schema) => {
                            if let Some(conn) = self.connections.get(self.active_conn) {
                                let _ = conn.db_tx.send(DbCommand::LoadTables { schema });
                            }
                        }
                        SidebarAction::LoadDetails { schema, table } => {
                            if let Some(conn) = self.connections.get(self.active_conn) {
                                let _ = conn.db_tx.send(DbCommand::LoadDetails { schema, table });
                            }
                        }
                        SidebarAction::BrowseTable { schema, table } => {
                            if let Some(conn) = self.connections.get(self.active_conn) {
                                let db_tx = conn.db_tx.clone();
                                self.tab_manager.start_browse(schema, table, conn_id, &db_tx);
                            }
                        }
                        SidebarAction::RunSql(sql) => {
                            self.tab_manager.set_sql(sql);
                            self.execute_query();
                        }
                        SidebarAction::NewTable { schema } => {
                            if let Some(conn) = self.connections.get(self.active_conn) {
                                let schemas = conn.sidebar.schema_names();
                                self.table_dialog.open_new(schema, schemas);
                            }
                        }
                        SidebarAction::SetSql(sql) => {
                            self.tab_manager.set_sql(sql);
                        }
                        SidebarAction::GenerateScript { schema, table, kind } => {
                            let details = self
                                .connections
                                .get(self.active_conn)
                                .and_then(|c| c.sidebar.get_table_details(&schema, &table))
                                .filter(|d| d.loaded)
                                .map(|d| (d.columns.clone(), d.indexes.clone()));

                            if let Some((cols, idxs)) = details {
                                let pk = pk_from_indexes(&idxs);
                                let sql = match kind {
                                    ScriptKind::Select => script_select(&schema, &table, &cols),
                                    ScriptKind::Insert => script_insert(&schema, &table, &cols),
                                    ScriptKind::Update => script_update(&schema, &table, &cols, &pk),
                                    ScriptKind::Delete => script_delete(&schema, &table, &cols, &pk),
                                };
                                self.tab_manager.set_sql(sql);
                            } else {
                                // Columns not loaded yet — request them and remember.
                                if let Some(conn) = self.connections.get_mut(self.active_conn) {
                                    conn.pending_script =
                                        Some((schema.clone(), table.clone(), kind));
                                    let _ = conn.db_tx.send(DbCommand::LoadDetails {
                                        schema,
                                        table,
                                    });
                                }
                            }
                        }
                        SidebarAction::ViewErDiagram { schema } => {
                            self.tab_manager.open_er_diagram(schema.clone(), conn_id);
                            if let Some(conn) = self.connections.get(self.active_conn) {
                                let _ = conn.db_tx.send(DbCommand::LoadErDiagram { schema });
                            }
                            self.tab_manager.mark_er_load_requested();
                        }
                        SidebarAction::EditTable { schema, table } => {
                            let cached = self
                                .connections
                                .get(self.active_conn)
                                .and_then(|conn| {
                                    conn.sidebar
                                        .get_table_details(&schema, &table)
                                        .filter(|d| d.loaded)
                                        .map(|d| d.columns.clone())
                                });
                            if let Some(columns) = cached {
                                if let Some(conn) = self.connections.get(self.active_conn) {
                                    let schemas = conn.sidebar.schema_names();
                                    self.table_dialog.open_edit(schema, table, columns, schemas);
                                }
                            } else {
                                if let Some(conn) = self.connections.get_mut(self.active_conn) {
                                    conn.pending_edit_table =
                                        Some((schema.clone(), table.clone()));
                                    let _ = conn
                                        .db_tx
                                        .send(DbCommand::LoadDetails { schema, table });
                                }
                            }
                        }
                    }
                }
            });

        // Central panel — query editor + results (with tab bar)
        egui::CentralPanel::default().show(ctx, |ui| {
            // Build the list of (conn_id, &db_tx) for tab_manager.show()
            let conn_refs: Vec<(usize, &Sender<DbCommand>)> = self
                .connections
                .iter()
                .map(|c| (c.id, &c.db_tx))
                .collect();
            self.tab_manager.show(ui, &conn_refs, &mut self.history);
        });

        // Dashboard refresh: if show_inline returned true (refresh button), reload
        // This is handled inside tab_manager.show() → dashboard.show_inline() which returns bool,
        // but we need to detect it here. We check again after show():
        if self.tab_manager.dashboard_is_active() && self.tab_manager.dashboard_needs_load() {
            if let Some(conn_id) = self.tab_manager.dashboard_conn_id() {
                if let Some(conn) = self.connections.iter().find(|c| c.id == conn_id) {
                    let _ = conn.db_tx.send(DbCommand::LoadDashboard);
                    self.tab_manager.set_dashboard_loading();
                }
            }
        }

        // Join Builder dialog
        let jb_tables = self
            .connections
            .get(self.active_conn)
            .map(|c| c.sidebar.all_tables_with_columns())
            .unwrap_or_default();
        self.join_builder.update_available(jb_tables);
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
                    if let Some(conn) = self.connections.get(self.active_conn) {
                        let _ = conn.db_tx.send(DbCommand::LoadDetails { schema, table });
                    }
                }
            }
        }

        // Table dialog (New / Edit)
        if let Some(action) = self.table_dialog.show(ctx) {
            match action {
                TableDialogAction::ExecuteDdl { sql, refresh_schema } => {
                    if let Some(conn) = self.connections.get_mut(self.active_conn) {
                        conn.pending_ddl_schema = Some(refresh_schema);
                        let _ = conn.db_tx.send(DbCommand::ExecuteDdl(sql));
                    }
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
                        self.connect_with_profile(profile);
                        self.show_connection_dialog = false;
                    }
                    if self.connection_dialog.cancelled {
                        self.connection_dialog.cancelled = false;
                        self.show_connection_dialog = false;
                    }
                });
            if !open {
                self.show_connection_dialog = false;
            }
        }

        // Request repaint while connecting or query running to show spinner
        let any_connecting = self.connections.iter().any(|c| {
            matches!(c.status, ConnectionStatus::Connecting)
        });
        if any_connecting || self.tab_manager.is_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

fn is_dml(sql: &str) -> bool {
    let t = sql.trim().to_lowercase();
    t.starts_with("insert")
        || t.starts_with("update")
        || t.starts_with("delete")
        || t.starts_with("truncate")
        || t.starts_with("create")
        || t.starts_with("drop")
        || t.starts_with("alter")
        || t.starts_with("grant")
        || t.starts_with("revoke")
}

fn configure_style(ctx: &egui::Context) {
    // ── JetBrains Darcula-inspired dark visuals ──────────────────────────────
    let mut vis = egui::Visuals::dark();

    vis.panel_fill     = egui::Color32::from_rgb(43, 43, 43);   // #2b2b2b
    vis.window_fill    = egui::Color32::from_rgb(60, 63, 65);   // #3c3f41
    vis.faint_bg_color = egui::Color32::from_rgb(49, 51, 53);   // #313335

    vis.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(49, 51, 53);
    vis.widgets.inactive.bg_fill       = egui::Color32::from_rgb(76, 80, 82);   // #4c5052
    vis.widgets.hovered.bg_fill        = egui::Color32::from_rgb(92, 97, 100);  // #5c6164
    vis.widgets.hovered.weak_bg_fill   = egui::Color32::from_rgb(92, 97, 100);
    vis.widgets.active.bg_fill         = egui::Color32::from_rgb(78, 159, 222); // #4e9fde

    vis.selection.bg_fill =
        egui::Color32::from_rgba_premultiplied(33, 66, 131, 180); // #214283
    vis.selection.stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(78, 159, 222)); // #4e9fde

    vis.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(74, 76, 78)); // #4a4c4e

    vis.window_rounding = egui::Rounding::same(6.0);
    vis.menu_rounding   = egui::Rounding::same(4.0);

    ctx.set_visuals(vis);

    // ── Spacing & fonts ─────────────────────────────────────────────────────
    let mut style = (*ctx.style()).clone();

    style.spacing.item_spacing   = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.menu_margin    = egui::Margin::same(6.0);
    style.spacing.window_margin  = egui::Margin::same(10.0);
    style.spacing.indent         = 12.0;

    use egui::{FontId, TextStyle};
    style.text_styles.insert(TextStyle::Heading,   FontId::proportional(15.0));
    style.text_styles.insert(TextStyle::Body,       FontId::proportional(13.0));
    style.text_styles.insert(TextStyle::Button,     FontId::proportional(13.0));
    style.text_styles.insert(TextStyle::Small,      FontId::proportional(11.0));
    style.text_styles.insert(TextStyle::Monospace,  FontId::monospace(13.0));

    ctx.set_style(style);

    // ── Windows symbol font fallback ─────────────────────────────────────────
    // egui's bundled Ubuntu/Hack fonts lack many Unicode symbol codepoints
    // (▶ ✕ ✎ ↺ ⟳ ＋ etc.).  On Windows there is no automatic OS-level fallback,
    // so we load Segoe UI Symbol + Segoe UI Emoji from the system fonts directory
    // and append them as fallbacks for every font family.
    #[cfg(target_os = "windows")]
    {
        let mut fonts = egui::FontDefinitions::default();
        for path in &[
            "C:/Windows/Fonts/seguisym.ttf",
            "C:/Windows/Fonts/seguiemj.ttf",
        ] {
            if let Ok(data) = std::fs::read(path) {
                let name = std::path::Path::new(path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                fonts.font_data.insert(name.clone(), egui::FontData::from_owned(data));
                for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                    fonts.families.entry(family).or_default().push(name.clone());
                }
            }
        }
        ctx.set_fonts(fonts);
    }
}
