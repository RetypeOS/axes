use crate::{
    cli::handlers::commons,
    core::index_manager::{self, GLOBAL_PROJECT_UUID},
    models::{GlobalIndex, IndexEntry, ProjectRef},
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

#[derive(Error, Debug)]
pub enum OnboardingError {
    #[error("Filesystem Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Index Error: {0}")]
    Index(#[from] crate::core::index_manager::IndexError),
    #[error("User Interface Error: {0}")]
    Dialoguer(#[from] dialoguer::Error),
    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),
    #[error(
        "The directory '{0}' does not appear to be an axes project (missing '.axes/axes.toml')."
    )]
    NotAnAxesProject(String),
    #[error("Operation cancelled by user.")]
    Cancelled,
    #[error(
        "Cannot register a project without identity in --autosolve mode. Project at '{0}' was skipped."
    )]
    IdentitylessInAutosolve(String),
    #[error("A project with the same name '{0}' already exists under the chosen parent.")]
    NameCollision(String),
    #[error("A project with UUID '{0}' is already registered at a different path: '{1}'.")]
    UuidCollision(Uuid, String),
}
type OnboardingResult<T> = Result<T, OnboardingError>;

// --- Core Data Structures ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentitySource {
    ProjectRef(ProjectRef),
    TomlOnly,
    NotFound,
}

#[derive(Debug, Clone)]
pub struct OnboardingCandidate {
    pub path: PathBuf,
    pub identity_source: IdentitySource,
    pub resolved_name: Option<String>,
    pub resolved_parent_uuid: Option<Uuid>,
    pub resolved_uuid: Option<Uuid>,
    pub should_register: bool,
}

impl OnboardingCandidate {
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

pub struct OnboardingOptions {
    pub autosolve: bool,
    pub suggested_parent_uuid: Option<Uuid>,
}

// --- Main Entry Point ---

/// The main entry point of the onboarding state machine.
pub fn register_project(
    path: &Path,
    index: &mut GlobalIndex,
    options: &OnboardingOptions,
) -> OnboardingResult<()> {
    println!(
        "\n{}",
        format!(t!("register.info.starting_scan"), path = path.display()).bold()
    );

    // 1. DISCOVERY: Find all potential projects recursively.
    let mut candidates = discover_candidates(path, index)?;
    if candidates.is_empty() {
        return Err(OnboardingError::NotAnAxesProject(
            path.display().to_string(),
        ));
    }

    // 2. Determine final names, parents, and UUIDs for each candidate.
    resolve_candidates(&mut candidates, index, options)?;

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
        present_plan_and_confirm(&candidates, index)?;
    }

    // 4. ACTION: Perform the actual registration in the index.
    let mut registered_count = 0;
    for candidate in candidates {
        let (uuid, name, parent_uuid) = (
            candidate.resolved_uuid.unwrap(),
            candidate.resolved_name.unwrap(),
            candidate.resolved_parent_uuid.unwrap(),
        );

        let new_entry = IndexEntry {
            name: name.clone(),
            path: candidate.path.clone(),
            parent: Some(parent_uuid),
            ..Default::default()
        };
        index.projects.insert(uuid, new_entry);

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
    index: &mut GlobalIndex,
    options: &OnboardingOptions,
) -> OnboardingResult<()> {
    // This prevents two new projects in the same batch from colliding with each other.
    let mut pending_names: HashSet<(Uuid, String)> = HashSet::new();

    for i in 0..candidates.len() {
        if !candidates[i].should_register {
            continue;
        }

        // --- Phase 2.1: Resolve UUID and Name ---
        {
            let candidate = &mut candidates[i];
            match &candidate.identity_source {
                IdentitySource::ProjectRef(pref) => {
                    if let Some(existing) = index.projects.get(&pref.self_uuid) {
                        if existing.path != candidate.path {
                            return Err(OnboardingError::UuidCollision(
                                pref.self_uuid,
                                existing.path.display().to_string(),
                            ));
                        } else {
                            // Project is already registered at the correct path. Mark to skip.
                            candidate.should_register = false;
                        }
                    }
                    if candidate.should_register {
                        candidate.resolved_uuid = Some(pref.self_uuid);
                        candidate.resolved_name = Some(pref.name.clone());
                    }
                }
                IdentitySource::TomlOnly => {
                    if options.autosolve {
                        candidate.should_register = false;
                        println!("{}", format!("Warning: Project at '{}' has no identity and was skipped in --autosolve mode.", candidate.path.display()).yellow());
                    } else {
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
                            .unwrap_or_default()
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
                }
                _ => {
                    candidate.should_register = false;
                }
            }
        }

        if !candidates[i].should_register {
            continue;
        }

        // --- Phase 2.2: Resolve Parent UUID ---
        let pref_parent_uuid = if let IdentitySource::ProjectRef(p) = &candidates[i].identity_source
        {
            p.parent_uuid
        } else {
            None
        };

        let is_pref_parent_valid = if let Some(p_uuid) = pref_parent_uuid {
            index.projects.contains_key(&p_uuid)
                || candidates
                    .iter()
                    .any(|c| c.should_register && c.resolved_uuid == Some(p_uuid))
        } else {
            false
        };

        let valid_pref_parent = if is_pref_parent_valid {
            pref_parent_uuid
        } else {
            None
        };

        let final_parent_uuid = match (options.suggested_parent_uuid, valid_pref_parent) {
            (Some(p), _) => Some(p),
            (None, Some(p)) => Some(p),
            _ => {
                if options.autosolve {
                    Some(GLOBAL_PROJECT_UUID)
                } else {
                    if pref_parent_uuid.is_some() {
                        println!(
                            "{}",
                            format!(
                                t!("register.warning.invalid_parent"),
                                path = candidates[i].path.display()
                            )
                            .yellow()
                        );
                    }
                    Some(commons::choose_parent_interactive(index)?)
                }
            }
        };

        // --- Phase 2.3: Final Collision Check & Write ---
        {
            let candidate = &mut candidates[i];
            candidate.resolved_parent_uuid = final_parent_uuid;

            let name = candidate.resolved_name.as_ref().unwrap();
            let parent = candidate.resolved_parent_uuid.unwrap();

            // Check for collision with projects already in the index.
            if index
                .projects
                .values()
                .any(|p| p.parent == Some(parent) && &p.name == name)
            {
                return Err(OnboardingError::NameCollision(name.clone()));
            }

            // ROBUSTNESS: Check for collision with projects in the current onboarding batch.
            if !pending_names.insert((parent, name.clone())) {
                return Err(OnboardingError::NameCollision(name.clone()));
            }
        }
    }
    Ok(())
}

// --- Phase 3: Confirmation ---

/// Shows a summary of changes and asks for final user confirmation.
fn present_plan_and_confirm(
    candidates: &[OnboardingCandidate],
    index: &GlobalIndex,
) -> OnboardingResult<()> {
    println!("\n{}", t!("register.info.plan_header").bold());
    for candidate in candidates {
        let name = candidate.resolved_name.as_ref().unwrap();
        let parent_uuid = candidate.resolved_parent_uuid.unwrap();
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
