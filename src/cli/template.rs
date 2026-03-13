use anyhow::Result;
use clap::Subcommand;
use comfy_table::{Cell, Table};
use dialoguer::{Input, MultiSelect};

use crate::{db::repo, db::template, open_db};

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// Create a new workspace template
    Create { name: String },
    /// List workspace templates
    List,
    /// Delete a workspace template
    Delete { name: String },
}

pub fn handle_template(cmd: TemplateCommands) -> Result<()> {
    match cmd {
        TemplateCommands::Create { name } => create(&name),
        TemplateCommands::List => list(),
        TemplateCommands::Delete { name } => delete(&name),
    }
}

fn create(name: &str) -> Result<()> {
    let db = open_db()?;

    let branch_template: String = Input::new()
        .with_prompt("Branch name template (e.g. {initials}-{ticket}-{desc})")
        .interact_text()?;

    let known = repo::list(&db)?;
    let repo_paths = if known.is_empty() {
        let path: String = Input::new()
            .with_prompt("Enter a repo path to include (blank to skip)")
            .allow_empty(true)
            .interact_text()?;
        if path.is_empty() {
            vec![]
        } else {
            vec![path]
        }
    } else {
        let selections = MultiSelect::new()
            .with_prompt("Select repos to include in this template")
            .items(&known)
            .interact()?;
        selections.into_iter().map(|i| known[i].clone()).collect()
    };

    template::create(&db, name, &branch_template, &repo_paths)?;
    println!("Template '{}' created.", name);
    Ok(())
}

fn list() -> Result<()> {
    let db = open_db()?;
    let templates = template::list(&db)?;

    if templates.is_empty() {
        println!("No templates. Use `cece template create <name>` to create one.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(["Name", "Branch Template", "Repos"]);
    for t in &templates {
        table.add_row([
            Cell::new(&t.name),
            Cell::new(&t.branch_template),
            Cell::new(t.repo_paths.join(", ")),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn delete(name: &str) -> Result<()> {
    let db = open_db()?;
    template::delete(&db, name)?;
    println!("Template '{}' deleted.", name);
    Ok(())
}
