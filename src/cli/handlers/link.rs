// EN: src/cli/handlers/link.rs

use anyhow::{Context, Result, anyhow};

use super::commons;
use crate::{
    CancellationToken,
    core::{context_resolver, index_manager},
};

use clap::Parser;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct LinkArgs {
    /// The project context to link.
    context: String,
    /// The new parent project's context.
    new_parent: String,
}

pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    // 1. Resolve the project to be moved. This requires a context.
    let link_args = LinkArgs::try_parse_from(&args)?;
    let config = commons::resolve_config_from_context_or_session(
        Some(link_args.context),
        cancellation_token,
    )?;

    // 2. Get the new parent's context from the arguments.
    let new_parent_context = link_args.new_parent.trim();

    if new_parent_context.is_empty() {
        return Err(anyhow!(t!("link.error.empty_parent_context")));
    }

    println!(
        t!("link.info.attempting"),
        name = config.qualified_name,
        new_parent = new_parent_context
    );

    // 3. Load the index and resolve the new parent's UUID.
    let mut index = index_manager::load_and_ensure_global_project()?;
    let (new_parent_uuid, new_parent_qualified_name) =
        context_resolver::resolve_context(new_parent_context, &index, cancellation_token)
            .with_context(|| {
                anyhow!(
                    t!("link.error.cannot_resolve_parent"),
                    parent = new_parent_context
                )
            })?;

    // 4. Perform the link operation, which includes all critical validations.
    index_manager::link_project(&mut index, config.uuid, new_parent_uuid)
        .with_context(|| anyhow!(t!("link.error.link_failed"), name = config.qualified_name))?;

    // 5. Save the updated global index.
    index_manager::save_global_index(&index).with_context(|| t!("error.saving_global_index"))?;

    // 6. Update the local project reference file (`project_ref.bin`).
    // This is a critical step for self-healing and consistency.
    let mut project_ref =
        index_manager::get_or_create_project_ref(&config.project_root, config.uuid, &index)
            .with_context(|| t!("error.local_ref_failed"))?;

    project_ref.parent_uuid = Some(new_parent_uuid);
    if let Err(e) = index_manager::write_project_ref(&config.project_root, &project_ref) {
        // This is not a fatal error, but the user should be warned.
        eprintln!(
            "\n{}",
            anyhow!(t!("link.warning.local_ref_update_failed"), error = e)
        );
    }

    // 7. Provide clear feedback to the user.
    println!("\n{}", t!("common.success"));
    println!(
        "  {}",
        // NOTE: The qualified name of the *moved project* will change.
        // We can't easily show the new name without rebuilding it, but we can
        // show what happened in a clear way.
        format_args!(
            t!("link.success.header"),
            old_name = config.qualified_name,
            new_parent = new_parent_qualified_name
        )
    );
    println!("  {}", t!("common.info.caches_will_regenerate"));

    Ok(())
}
