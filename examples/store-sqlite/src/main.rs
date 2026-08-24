mod transaction;

use fission::prelude::*;
use fission::sql::{SqlMigration, SqlMigrations, SqlStatement};

#[derive(Debug, Default)]
struct AppState {
    projects: Vec<String>,
    message: String,
}

impl GlobalState for AppState {}

#[fission_reducer(PrepareDatabase)]
fn prepare_database(
    _state: &mut AppState,
    _action: PrepareDatabase,
    ctx: &mut ReducerContext<AppState>,
) {
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
fn add_project(_state: &mut AppState, _action: AddProject, ctx: &mut ReducerContext<AppState>) {
    let mut transaction = transaction::new_project("A Fission project");
    transaction::append_audit(&mut transaction, "Project created");
    ctx.effects
        .sql()
        .transaction(transaction)
        .on_ok(ActionEnvelope::from(LoadProjects))
        .on_err(ActionEnvelope::from(DatabaseFailed));
}

#[fission_reducer(LoadProjects)]
fn load_projects(_state: &mut AppState, _action: LoadProjects, ctx: &mut ReducerContext<AppState>) {
    ctx.effects
        .sql()
        .query(SqlStatement::new(
            "SELECT name FROM projects ORDER BY id DESC LIMIT 20",
        ))
        .on_ok(ActionEnvelope::from(ProjectsLoaded))
        .on_err(ActionEnvelope::from(DatabaseFailed));
}

#[fission_reducer(ProjectsLoaded)]
fn projects_loaded(
    state: &mut AppState,
    _action: ProjectsLoaded,
    ctx: &mut ReducerContext<AppState>,
) {
    let Some(rows) = ctx.input.sql_rows() else {
        return;
    };
    state.projects = rows
        .rows
        .iter()
        .filter_map(|row| row.get::<String>("name").ok())
        .collect();
    state.message = format!("Loaded {} projects", state.projects.len());
}

#[fission_reducer(DatabaseFailed)]
fn database_failed(
    state: &mut AppState,
    _action: DatabaseFailed,
    ctx: &mut ReducerContext<AppState>,
) {
    state.message = ctx
        .input
        .sql_error()
        .map(|error| error.message)
        .unwrap_or_else(|| "Database operation failed".to_string());
}

#[derive(Clone)]
struct StoreExample;

impl From<StoreExample> for Widget {
    fn from(_: StoreExample) -> Widget {
        let (ctx, view) = fission::build::current::<AppState>();
        let tokens = &view.env().theme.tokens;
        let add = ctx.bind(AddProject, reduce!(add_project));
        ctx.register::<PrepareDatabase, _>(reduce!(prepare_database));
        ctx.register::<LoadProjects, _>(reduce!(load_projects));
        ctx.register::<ProjectsLoaded, _>(reduce!(projects_loaded));
        ctx.register::<DatabaseFailed, _>(reduce!(database_failed));

        let mut children = widgets![
            Text::new("SQLite store")
                .size(tokens.typography.heading1_size)
                .color(tokens.colors.text_primary),
            Text::new(view.state().message.clone()).color(tokens.colors.text_secondary),
            Button {
                on_press: Some(add),
                child: Some(Text::new("Add project atomically").into()),
                ..Default::default()
            },
        ];
        children.extend(
            view.state()
                .projects
                .iter()
                .cloned()
                .map(|project| Text::new(project).into()),
        );

        Container::new(Column {
            gap: Some(tokens.spacing.m),
            children,
            ..Default::default()
        })
        .padding_all(tokens.spacing.xl)
        .bg(tokens.colors.background)
        .into()
    }
}

fn main() -> anyhow::Result<()> {
    DesktopApp::<AppState, _>::new(StoreExample)
        .with_title("Fission SQLite store")
        .with_startup_action(PrepareDatabase)
        .run()
}
