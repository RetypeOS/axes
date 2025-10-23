use crate::{
    cli::handlers::commons,
    core::index_manager::{self, GLOBAL_PROJECT_UUID},
    models::{GlobalIndex, IndexEntry, ProjectRef},
    state::AppStateGuard,
};
use anyhow;
use colored::*;
use dialoguer::{Confirm, Input, theme::ColorfulTheme};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;

// --- Error Handling ---

/// Represents errors that can occur during the project registration (`onboarding`) process.
#[derive(Error, Debug)]
pub enum OnboardingError {
    /// A filesystem I/O error occurred.
    #[error("Filesystem Error: {0}")]
    Io(#[from] std::io::Error),
    /// An error occurred while interacting with the global index.
    #[error("Index Error: {0}")]
    Index(#[from] crate::core::index_manager::IndexError),
    /// An error occurred during an interactive prompt.
    #[error("User Interface Error: {0}")]
    Dialoguer(#[from] dialoguer::Error),
    /// A generic, non-specific error.
    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),
    /// The target directory for registration does not contain an `.axes/axes.toml` file.
    #[error(
        "The directory '{0}' does not appear to be an axes project (missing '.axes/axes.toml')."
    )]
    NotAnAxesProject(String),
    /// The user cancelled the registration process.
    #[error("Operation cancelled by user.")]
    Cancelled,
    /// In non-interactive mode (`--autosolve`), a project without a local `project_ref.bin` was found and skipped.
    #[error(
        "Cannot register a project without identity in --autosolve mode. Project at '{0}' was skipped."
    )]
    IdentitylessInAutosolve(String),
    /// A project with the resolved name already exists under the chosen parent.
    #[error("A project with the same name '{0}' already exists under the chosen parent.")]
    NameCollision(String),
    /// A project with the same UUID is already registered, but at a different filesystem path.
    #[error("A project with UUID '{0}' is already registered at a different path: '{1}'.")]
    UuidCollision(Uuid, String),
}

type OnboardingResult<T> = Result<T, OnboardingError>;

// --- Core Data Structures ---

/// Describes the source of a project's identity during the discovery phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentitySource {
    /// The project has a valid `.axes/project_ref.bin` file.
    ProjectRef(ProjectRef),
    /// The project has an `axes.toml` file but no `project_ref.bin`.
    TomlOnly,
    /// The directory is not a valid axes project (missing `axes.toml`).
    NotFound,
}

/// Represents a potential project found on the filesystem that could be registered.
#[derive(Debug, Clone)]
pub struct OnboardingCandidate {
    /// The absolute, canonical path to the project's root directory.
    pub path: PathBuf,
    /// How the project's identity was determined.
    pub identity_source: IdentitySource,
    /// The final, resolved simple name for the project after handling conflicts or user input.
    pub resolved_name: Option<String>,
    /// The final, resolved UUID of the project's parent.
    pub resolved_parent_uuid: Option<Uuid>,
    /// The final, resolved UUID for the project itself.
    pub resolved_uuid: Option<Uuid>,
    /// A flag indicating if this candidate should be registered. Can be set to `false` to skip it.
    pub should_register: bool,
}

impl OnboardingCandidate {
    /// Creates a new candidate by inspecting a given path for `axes.toml` and `project_ref.bin`.
    pub fn new(path: &Path) -> OnboardingResult<Self> {
        let canonical_path = dunce::canonicalize(path)?;
        if !canonical_path.join(".axes/axes.toml").exists() {
            return Ok(Self::invalid(canonical_path, IdentitySource::NotFound));
        }
        let identity_source = match index_manager::read_project_ref(&canonical_path) {
            Ok(pref) => IdentitySource::ProjectRef(pref),
            Err(_) => IdentitySource::TomlOnly,
        };
        Ok(Self {
            path: canonical_path,
            identity_source,
            resolved_name: None,
            resolved_parent_uuid: None,
            resolved_uuid: None,
            should_register: true,
        })
    }
    fn invalid(path: PathBuf, source: IdentitySource) -> Self {
        Self {
            path,
            identity_source: source,
            resolved_name: None,
            resolved_parent_uuid: None,
            resolved_uuid: None,
            should_register: false,
        }
    }
}

/// Configuration options for the `register_project` process.
#[derive(Debug)]
pub struct OnboardingOptions {
    /// If true, the process will be non-interactive and fail on any ambiguity or conflict.
    pub autosolve: bool,
    /// An optional parent UUID suggested via CLI arguments (e.g., `--parent`).
    pub suggested_parent_uuid: Option<Uuid>,
}

// --- Main Entry Point ---

/// The main entry point of the project registration process.
///
/// This function orchestrates a multi-phase process:
/// 1.  **Discovery**: Recursively scans the filesystem from a starting path to find all
///     unregistered axes projects.
/// 2.  **Resolution**: For each found project (`OnboardingCandidate`), it resolves its final name,
///     UUID, and parent, handling conflicts, missing identities, and user interaction.
/// 3.  **Confirmation**: If in interactive mode, it presents a plan of all projects to be
///     registered and asks for user confirmation.
/// 4.  **Action**: If confirmed, it modifies the `GlobalIndex` to register the new projects
///     and creates/updates their local `project_ref.bin` files.
///
/// # Arguments
/// * `path` - The starting path for the discovery scan.
/// * `state_guard` - A mutable guard to the application's global state.
/// * `options` - Configuration for the registration process (e.g., interactive or not).
pub fn register_project(
    path: &Path,
    state_guard: &mut AppStateGuard<'_>,
    options: &OnboardingOptions,
) -> OnboardingResult<()> {
    println!(
        "\n{}",
        format!(t!("register.info.starting_scan"), path = path.display()).bold()
    );

    // 1. DISCOVERY: Find all potential projects recursively.
    let mut candidates = discover_candidates(path, state_guard.index())?;
    if candidates.is_empty() {
        return Err(OnboardingError::NotAnAxesProject(
            path.display().to_string(),
        ));
    }

    // 2. Determine final names, parents, and UUIDs for each candidate.
    resolve_candidates(&mut candidates, state_guard, options)?;

    // Filter out candidates that were invalidated during resolution.
    candidates.retain(|c| c.should_register);

    if candidates.is_empty() {
        println!(
            "{}",
            t!("register.info.no_new_projects_to_register").yellow()
        );
        return Ok(());
    }

    // 3. CONFIRMATION (if interactive): Show the plan and get user approval.
    if !options.autosolve {
        present_plan_and_confirm(&candidates, state_guard.index())?;
    }

    // 4. ACTION: Perform the actual registration in the index.
    let mut registered_count = 0;
    for candidate in candidates {
        let (uuid, name, parent_uuid) = (
            candidate
                .resolved_uuid
                .expect("Candidate should have a resolved UUID at this stage"),
            candidate
                .resolved_name
                .expect("Candidate should have a resolved name at this stage"),
            candidate
                .resolved_parent_uuid
                .expect("Candidate should have a resolved parent UUID at this stage"),
        );

        let new_entry = IndexEntry {
            name: name.clone(),
            path: candidate.path.clone(),
            parent: Some(parent_uuid),
            ..Default::default()
        };
        state_guard.index_mut().projects.insert(uuid, new_entry);

        let new_ref = ProjectRef {
            self_uuid: uuid,
            parent_uuid: Some(parent_uuid),
            name,
        };
        index_manager::write_project_ref(&candidate.path, &new_ref)?;

        registered_count += 1;
    }

    println!(
        "{}",
        format!(t!("register.success.header"), count = registered_count).green()
    );
    Ok(())
}

// --- Phase 1: Discovery ---

/// Recursively scans for `axes.toml` files and creates candidates.
fn discover_candidates(
    start_path: &Path,
    index: &GlobalIndex,
) -> OnboardingResult<Vec<OnboardingCandidate>> {
    let mut candidates = Vec::new();
    let main_candidate = OnboardingCandidate::new(start_path)?;
    if !main_candidate.should_register {
        return Ok(vec![]);
    }

    // Pre-calculate a HashSet of existing paths for O(1) lookups.
    let existing_paths: HashSet<_> = index.projects.values().map(|p| &p.path).collect();

    let mut to_scan = vec![main_candidate.path.clone()];
    let mut seen_paths = HashSet::new();
    seen_paths.insert(main_candidate.path.clone());
    candidates.push(main_candidate);

    while let Some(path) = to_scan.pop() {
        // Use `read_dir` in a way that gracefully skips directories it can't access.
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.filter_map(Result::ok) {
                let child_path = entry.path();
                // Check if it's a directory, not already seen, and not already in the index.
                if child_path.is_dir()
                    && !existing_paths.contains(&child_path)
                    && seen_paths.insert(child_path.clone())
                {
                    match OnboardingCandidate::new(&child_path) {
                        Ok(candidate) if candidate.should_register => {
                            to_scan.push(child_path);
                            candidates.push(candidate);
                        }
                        Ok(_) => {} // Not a valid candidate, just ignore.
                        Err(e) => log::warn!(
                            "Could not process potential project at '{}': {}",
                            child_path.display(),
                            e
                        ),
                    }
                }
            }
        }
    }
    Ok(candidates)
}

// --- Phase 2: Resolution ---

/// Iterates through candidates to resolve names, parents, and handle conflicts.
fn resolve_candidates(
    candidates: &mut [OnboardingCandidate],
    state_guard: &mut AppStateGuard<'_>,
    options: &OnboardingOptions,
) -> OnboardingResult<()> {
    // This set tracks (parent_uuid, name) tuples for projects being added in this batch
    // to prevent internal collisions.
    let mut pending_names: HashSet<(Uuid, String)> = HashSet::new();

    // --- Phase 1: Resolve all identities first ---
    // We do this in a separate loop to ensure all `resolved_uuid` fields are populated
    // before we try to validate parent links in the next phase.
    for candidate in candidates.iter_mut() {
        if candidate.should_register {
            resolve_candidate_identity(candidate, state_guard.index(), options)?;
        }
    }

    // --- Pre-computation Step ---
    // Collect all UUIDs of candidates that are still valid for registration.
    // This avoids the mutable/immutable borrow conflict in the main loop.
    let valid_candidate_uuids: HashSet<Uuid> = candidates
        .iter()
        .filter_map(|c| {
            if c.should_register {
                c.resolved_uuid
            } else {
                None
            }
        })
        .collect();

    // --- Phase 2: Resolve parents and check for collisions ---
    for candidate in candidates.iter_mut() {
        if !candidate.should_register {
            continue;
        }

        // Resolve the parent, now passing the pre-computed set of valid UUIDs.
        let parent_uuid =
            resolve_candidate_parent(candidate, &valid_candidate_uuids, state_guard, options)?;
        candidate.resolved_parent_uuid = Some(parent_uuid);

        // --- Final Collision Check ---
        let name = candidate
            .resolved_name
            .as_ref()
            .expect("Candidate name should be resolved at this stage");

        // Check for collision with projects already in the global index.
        if index_manager::is_sibling_name_taken(state_guard.index(), parent_uuid, name, None) {
            return Err(OnboardingError::NameCollision(name.clone()));
        }

        // Check for collision with other projects being onboarded in this same batch.
        if !pending_names.insert((parent_uuid, name.clone())) {
            return Err(OnboardingError::NameCollision(name.clone()));
        }
    }

    Ok(())
}

/// Helper function to resolve the identity (UUID and name) of a single candidate.
/// Modifies the candidate in place.
fn resolve_candidate_identity(
    candidate: &mut OnboardingCandidate,
    index: &GlobalIndex,
    options: &OnboardingOptions,
) -> OnboardingResult<()> {
    match &candidate.identity_source {
        IdentitySource::ProjectRef(pref) => {
            if let Some(existing) = index.projects.get(&pref.self_uuid) {
                if existing.path != candidate.path {
                    return Err(OnboardingError::UuidCollision(
                        pref.self_uuid,
                        existing.path.display().to_string(),
                    ));
                }
                // Project is already registered at the correct path. Mark to skip.
                candidate.should_register = false;
                return Ok(());
            }
            candidate.resolved_uuid = Some(pref.self_uuid);
            candidate.resolved_name = Some(pref.name.clone());
        }
        IdentitySource::TomlOnly => {
            if options.autosolve {
                candidate.should_register = false;
                println!("{}", format!("Warning: Project at '{}' has no identity and was skipped in --autosolve mode.", candidate.path.display()).yellow());
                return Ok(());
            }

            println!(
                "{}",
                format!(
                    "\nProject at '{}' has no identity.",
                    candidate.path.display()
                )
                .yellow()
            );
            let name_prompt = t!("register.prompt.name_for_identityless");
            let default_name = candidate
                .path
                .file_name()
                .expect("Path should have a filename")
                .to_string_lossy()
                .to_string();

            // Loop to validate the name interactively
            loop {
                let input_name = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt(name_prompt)
                    .default(default_name.clone())
                    .interact_text()?;

                match commons::validate_project_name(&input_name) {
                    Ok(name) => {
                        candidate.resolved_name = Some(name);
                        break;
                    }
                    Err(e) => println!("{}", format!("  Error: {}", e).red()),
                }
            }
            candidate.resolved_uuid = Some(Uuid::new_v4());
        }
        IdentitySource::NotFound => {
            candidate.should_register = false;
        }
    }
    Ok(())
}

/// Helper function to determine the parent UUID for a single candidate.
/// This function is now read-only with respect to the `candidate` itself.
fn resolve_candidate_parent(
    candidate: &OnboardingCandidate, // <-- Ahora es una referencia inmutable
    valid_candidate_uuids: &HashSet<Uuid>, // <-- Recibe el set pre-calculado
    state_guard: &mut AppStateGuard<'_>,
    options: &OnboardingOptions,
) -> OnboardingResult<Uuid> {
    // Check for a parent suggested via command-line flag first.
    if let Some(suggested_parent) = options.suggested_parent_uuid {
        return Ok(suggested_parent);
    }

    // Check if the parent from its `project_ref.bin` is valid.
    if let Some(pref_parent_uuid) = candidate
        .identity_source
        .as_project_ref()
        .and_then(|p| p.parent_uuid)
    {
        // A parent is valid if it's already in the index OR it's in our pre-computed set.
        let is_valid = state_guard.index().projects.contains_key(&pref_parent_uuid)
            || valid_candidate_uuids.contains(&pref_parent_uuid);

        if is_valid {
            return Ok(pref_parent_uuid);
        }

        // If the parent from the ref file is invalid, warn the user if interactive.
        if !options.autosolve {
            println!(
                "{}",
                format!(
                    t!("register.warning.invalid_parent"),
                    path = candidate.path.display()
                )
                .yellow()
            );
        }
    }

    // Fallback: Use autosolve default or launch interactive selection.
    if options.autosolve {
        Ok(GLOBAL_PROJECT_UUID)
    } else {
        Ok(commons::choose_parent_interactive(state_guard)?)
    }
}

// We can add a small helper on IdentitySource to make the code cleaner.
impl IdentitySource {
    fn as_project_ref(&self) -> Option<&ProjectRef> {
        if let Self::ProjectRef(pref) = self {
            Some(pref)
        } else {
            None
        }
    }
}

// --- Phase 3: Confirmation ---

/// Shows a summary of changes and asks for final user confirmation.
fn present_plan_and_confirm(
    candidates: &[OnboardingCandidate],
    index: &GlobalIndex,
) -> OnboardingResult<()> {
    println!("\n{}", t!("register.info.plan_header").bold());
    for candidate in candidates {
        let name = candidate
            .resolved_name
            .as_ref()
            .expect("Candidate to be registered must have a name");
        let parent_uuid = candidate
            .resolved_parent_uuid
            .expect("Candidate to be registered must have a parent");
        let parent_name =
            index_manager::build_qualified_name(parent_uuid, index).unwrap_or_default();
        println!(
            "  - {} '{}' {} '{}'",
            t!("register.info.plan_line_project"),
            name.cyan(),
            t!("register.info.plan_line_as_child_of"),
            parent_name.yellow()
        );
    }

    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(t!("common.prompt.continue"))
        .default(true)
        .interact()?
    {
        return Err(OnboardingError::Cancelled);
    }
    Ok(())
}
