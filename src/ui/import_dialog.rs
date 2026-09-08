use crate::i18n::I18n;

/// Action emitted back to the app when the user confirms an import.
#[derive(Debug)]
pub enum ImportCsvAction {
    Run { schema: String, table: String, path: String, columns: Vec<String> },
}

/// Confirmation dialog shown before a CSV import runs. Column names are taken
/// directly from the CSV file's own header row — PostgreSQL's `COPY ... FROM
/// STDIN WITH (FORMAT csv, HEADER true)` skips that line itself, so no manual
/// per-column mapping widget is needed; a mismatched name simply surfaces as a
/// clear Postgres error ("column X of relation Y does not exist") after Import
/// is clicked, same as any other DDL/DML error in this app.
#[derive(Debug, Default)]
pub struct ImportCsvDialog {
    pub open: bool,
    schema: String,
    table: String,
    path: String,
    headers: Vec<String>,
    error: Option<String>,
    pub running: bool,
}

impl ImportCsvDialog {
    /// Open the dialog for `schema.table`, reading just the first line of `path`
    /// to show the user which columns will be imported before they commit.
    pub fn open_for(&mut self, schema: String, table: String, path: String) {
        self.schema = schema;
        self.table = table;
        self.path = path.clone();
        self.running = false;
        match read_csv_header(&path) {
            Ok(headers) => {
                self.headers = headers;
                self.error = None;
            }
            Err(e) => {
                self.headers = Vec::new();
                self.error = Some(e);
            }
        }
        self.open = true;
    }

    pub fn show(&mut self, ctx: &egui::Context, i18n: &I18n) -> Option<ImportCsvAction> {
        if !self.open {
            return None;
        }

        let mut action = None;
        let mut open = self.open;
        let mut cancel_clicked = false;

        egui::Window::new(i18n.import_csv_title())
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!("{} → \"{}\".\"{}\"", self.path, self.schema, self.table));
                ui.separator();

                if let Some(err) = &self.error {
                    ui.colored_label(egui::Color32::from_rgb(200, 60, 60), err);
                } else if self.running {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(i18n.import_csv_running());
                    });
                } else {
                    ui.label(i18n.import_csv_detected_cols());
                    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        for h in &self.headers {
                            ui.monospace(h);
                        }
                    });
                }

                ui.separator();
                ui.horizontal(|ui| {
                    let can_import = self.error.is_none() && !self.headers.is_empty() && !self.running;
                    if ui.add_enabled(can_import, egui::Button::new(i18n.btn_import())).clicked() {
                        action = Some(ImportCsvAction::Run {
                            schema: self.schema.clone(),
                            table: self.table.clone(),
                            path: self.path.clone(),
                            columns: self.headers.clone(),
                        });
                        self.running = true;
                    }
                    if ui.button(i18n.btn_cancel()).clicked() {
                        cancel_clicked = true;
                    }
                });
            });

        self.open = open && !cancel_clicked;
        action
    }
}

/// Read just the first line of a CSV file and split it into column names.
/// Hand-rolled (no `csv` crate, matching this project's existing zero-dependency
/// export writer) — handles quoted fields with embedded commas/escaped quotes,
/// the same escaping rules `export_csv` produces.
fn read_csv_header(path: &str) -> Result<Vec<String>, String> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() {
        return Err("Empty file".to_owned());
    }
    Ok(split_csv_line(line))
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == ',' {
            fields.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    fields.push(cur);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_csv_line_handles_plain_fields() {
        assert_eq!(split_csv_line("id,name,active"), vec!["id", "name", "active"]);
    }

    #[test]
    fn split_csv_line_handles_quoted_field_with_comma() {
        assert_eq!(
            split_csv_line(r#"id,"last, first",active"#),
            vec!["id", "last, first", "active"]
        );
    }

    #[test]
    fn split_csv_line_handles_escaped_quotes() {
        assert_eq!(
            split_csv_line(r#""say ""hi""",plain"#),
            vec![r#"say "hi""#, "plain"]
        );
    }

    #[test]
    fn read_csv_header_reads_first_line_only() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ferox_test_csv_{}.csv", std::process::id()));
        std::fs::write(&path, "id,name,payload\r\n1,alpha,{}\r\n2,beta,{}\r\n").unwrap();
        let headers = read_csv_header(path.to_str().unwrap()).unwrap();
        assert_eq!(headers, vec!["id", "name", "payload"]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_csv_header_errors_on_empty_file() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ferox_test_csv_empty_{}.csv", std::process::id()));
        std::fs::write(&path, "").unwrap();
        assert!(read_csv_header(path.to_str().unwrap()).is_err());
        let _ = std::fs::remove_file(&path);
    }
}
