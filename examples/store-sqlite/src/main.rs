mod transaction;

use fission::prelude::*;
use fission::sql::{SqlMigration, SqlMigrations, SqlStatement};

#[derive(Debug)]
struct Project {
    id: i64,
    name: String,
}

#[derive(Debug, Default)]
struct AppState {
    projects: Vec<Project>,
    message: String,
}

impl GlobalState for AppState {}

#[fission_reducer(PrepareDatabase)]
fn prepare_database(_state: &mut AppState, ctx: &mut ReducerContext<AppState>) {
    let mut migrations = SqlMigrations::new();
    migrations
        .add(SqlMigration::new(
            1,
            "create projects and audit",
            "CREATE TABLE projects(id INTEGER PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE audit(id INTEGER PRIMARY KEY, message TEXT NOT NULL);",
        ))
        .expect("migration versions are unique");
    ctx.effects
        .sql()
        .migrate(migrations)
        .on_ok(ActionEnvelope::from(LoadProjects))
        .on_err(ActionEnvelope::from(DatabaseFailed));
}

#[fission_reducer(AddProject)]
fn add_project(_state: &mut AppState, ctx: &mut ReducerContext<AppState>) {
    let mut transaction = transaction::new_project("A Fission project");
    transaction::append_audit(&mut transaction, "Project created");
    ctx.effects
        .sql()
        .transaction(transaction)
        .on_ok(ActionEnvelope::from(LoadProjects))
        .on_err(ActionEnvelope::from(DatabaseFailed));
}

#[fission_reducer(LoadProjects)]
fn load_projects(_state: &mut AppState, ctx: &mut ReducerContext<AppState>) {
    ctx.effects
        .sql()
        .query(SqlStatement::new(
            "SELECT id, name FROM projects ORDER BY id DESC LIMIT 20",
        ))
        .on_ok(ActionEnvelope::from(ProjectsLoaded))
        .on_err(ActionEnvelope::from(DatabaseFailed));
}

#[fission_reducer(ProjectsLoaded)]
fn projects_loaded(state: &mut AppState, ctx: &mut ReducerContext<AppState>) {
    let Some(rows) = ctx.input.sql_rows() else {
        return;
    };
    state.projects = rows
        .rows
        .iter()
        .filter_map(|row| {
            Some(Project {
                id: row.get::<i64>("id").ok()?,
                name: row.get::<String>("name").ok()?,
            })
        })
        .collect();
    state.message = format!("Loaded {} projects", state.projects.len());
}

#[fission_reducer(DatabaseFailed)]
fn database_failed(state: &mut AppState, ctx: &mut ReducerContext<AppState>) {
    state.message = ctx
        .input
        .sql_error()
        .map(|error| error.message)
        .unwrap_or_else(|| "Database operation failed".to_string());
}

fission_component! {
    StoreExample<AppState>
    |_app, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let add = cx.build.bind(AddProject, reduce!(add_project));
        cx.build.register::<PrepareDatabase, _>(reduce!(prepare_database));
        cx.build.register::<LoadProjects, _>(reduce!(load_projects));
        cx.build.register::<ProjectsLoaded, _>(reduce!(projects_loaded));
        cx.build.register::<DatabaseFailed, _>(reduce!(database_failed));
    } =>
    Container(
        Column(
            gap: tokens.spacing.m,
            children: [
                Text("SQLite store")
                    .size(tokens.typography.heading1_size)
                    .color(tokens.colors.text_primary),
                Text(state.message.clone()).color(tokens.colors.text_secondary),
                Button(
                    child: Text("Add project atomically"),
                    on_press: add,
                ),
                for project in &state.projects {
                    Text(project.name.clone(), key: project.id)
                },
            ],
        ),
    )
    .padding_all(tokens.spacing.xl)
    .bg(tokens.colors.background)
}

fn main() -> anyhow::Result<()> {
    DesktopApp::<AppState, _>::new(StoreExample)
        .with_title("Fission SQLite store")
        .with_startup_action(PrepareDatabase)
        .run()
}
